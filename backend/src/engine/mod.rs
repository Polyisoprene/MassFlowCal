//! 物料平衡计算引擎
//!
//! 核心流程：
//! 1. 将 FlowGraph 构建为 DAG（有向无环图）
//! 2. 拓扑排序确定计算顺序
//! 3. 按序逐节点求值（收集上游输入 → 计算 → 缓存结果）
//! 4. 全局物料平衡检验

mod dag;
pub mod nodes;

use std::collections::HashMap;
use uuid::Uuid;

use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::{FlowBasis, FlowGraph, Node, NodeParams, NodeType};
use crate::models::stream::MaterialStream;

use nodes::NodeOutputs;

/// 计算引擎，持有数据库引用用于分子量查询
pub struct Engine {
    db: Database,
}

impl Engine {
    pub fn new(db: Database) -> Self { Self { db } }

    /// 获取数据库引用（用于外部组件查询）
    pub fn db(&self) -> &Database { &self.db }

    /// 执行全图计算的主要入口
    ///
    /// 返回 HashMap<node_id, Vec<(port_name, stream)>> — 每个节点的每个输出端口一个流
    /// NodeOutputs 使用 Vec 而非 HashMap，保证端口顺序与节点定义一致
    pub fn compute(
        &self,
        graph: &FlowGraph,
    ) -> Result<HashMap<Uuid, NodeOutputs>, EngineError> {
        // 1. 构建 DAG + 拓扑排序
        let (dag, node_map) = dag::build_dag(graph)?;
        let order = dag::compute_order(&dag, &node_map)?;

        // 2. 建立 node_id → Node 查找（后续计算需要端口定义等信息）
        let node_lookup: HashMap<Uuid, &Node> = graph.nodes.iter().map(|n| (n.id, n)).collect();

        // 3. 按拓扑序执行
        let mut results: HashMap<Uuid, NodeOutputs> = HashMap::new();
        for node_id in &order {
            let node = node_lookup.get(node_id).ok_or_else(|| {
                EngineError::Computation(format!("节点 {} 不存在", node_id))
            })?;
            let outputs = self.compute_node(node, graph, &results)?;
            results.insert(node.id, outputs);
        }

        // 4. 全局物料平衡校验
        self.check_overall_balance(graph, &results)?;
        Ok(results)
    }

    /// 计算单个节点
    ///
    /// 先按 node_type 处理无参数节点（Mixer, Product, Copy, Filter），
    /// 再按 params 匹配有参数节点（Feed, Reactor, Splitter, ...）
    fn compute_node(
        &self,
        node: &Node,
        graph: &FlowGraph,
        results: &HashMap<Uuid, NodeOutputs>,
    ) -> Result<NodeOutputs, EngineError> {
        let inputs = nodes::collect_inputs(node, &graph.connections, results)?;

        // 无参数节点（params 为 None）→ 按 node_type 分发
        let params = match &node.params {
            Some(p) => p,
            None => {
                return match node.node_type {
                    NodeType::Mixer => nodes::mixer::compute_mixer(&inputs, &self.db),
                    // Copy: 将上游流复制 N 份到各输出端口
                    NodeType::Copy => {
                        let stream = inputs.values().next().cloned().unwrap_or_default();
                        let outputs: NodeOutputs = node.outputs.iter()
                            .map(|p| (p.name.clone(), stream.clone()))
                            .collect();
                        Ok(outputs)
                    }
                    // Filter: 直接透传上游流
                    NodeType::Filter => {
                        let upstream = inputs.values().next().cloned().unwrap_or_default();
                        if upstream.is_empty() {
                            return Err(EngineError::Computation("Filter: 上游流为空".into()));
                        }
                        Ok(vec![("out".to_string(), upstream)])
                    }
                    // Product: 透传上游流，标记为系统出口
                    _ => {
                        let stream = inputs.values().next().cloned().unwrap_or_default();
                        Ok(vec![("product_out".to_string(), stream)])
                    }
                };
            }
        };

        // 有参数节点 → 按 params 变体分发
        match params {
            NodeParams::Feed(p) => {
                // Feed 节点有上游输入时：取上游摩尔流量数值，按自身组分和基准换算
                if let Some(upstream) = inputs.values().next() {
                    if !upstream.is_empty() && !p.cas_number.is_empty() {
                        let mw = self.db().get_mw(&p.cas_number)?;
                        // 两种基准都使用 upstream.mole_flow 作为源数值
                        let stream = match p.flow_basis {
                            // Mass 模式：质量 = 上游摩尔数 × MW
                            FlowBasis::Mass => MaterialStream::from_pure_mass(&p.cas_number, upstream.mole_flow, mw),
                            // Mole 模式：摩尔 = 上游摩尔数，质量 = 摩尔 × MW
                            FlowBasis::Mole => MaterialStream::from_pure_mole(&p.cas_number, upstream.mole_flow, mw),
                        };
                        return Ok(vec![("feed_out".to_string(), stream)]);
                    }
                }
                // 无上游或 CAS 为空 → 使用 Feed 自身参数
                let stream = nodes::feed::compute_feed(p, &self.db)?;
                Ok(vec![("feed_out".to_string(), stream)])
            }
            NodeParams::Reactor(p) => nodes::reactor::compute_reactor(p, &inputs, &self.db),
            NodeParams::Splitter(p) => nodes::splitter::compute_splitter(p, &inputs, &self.db),
            NodeParams::Separator(p) => nodes::separator::compute_separator(p, &inputs, &self.db),
            NodeParams::Decanter(p) => nodes::decanter::compute_decanter(p, &inputs, &self.db),
            NodeParams::Calculator(p) => nodes::calculator::compute_calculator(p, &inputs, &self.db),
            NodeParams::UnitConversion(p) => nodes::unit_conversion::compute_unit_conversion(p, &inputs, &self.db),
        }
    }

    /// 全局物料平衡检验
    ///
    /// 汇总所有 Feed 节点的输出质量流量作为总输入，
    /// 汇总所有 Product 节点的输入质量流量作为总输出，
    /// 容差 = max(总流量 × 0.001, 0.1) kg/h
    fn check_overall_balance(
        &self,
        graph: &FlowGraph,
        results: &HashMap<Uuid, NodeOutputs>,
    ) -> Result<(), EngineError> {
        let mut total_input = 0.0f64;
        let mut total_output = 0.0f64;

        for node in &graph.nodes {
            match node.node_type {
                NodeType::Feed => {
                    if let Some(outputs) = results.get(&node.id) {
                        for (_, stream) in outputs {
                            total_input += stream.mass_flow;
                        }
                    }
                }
                NodeType::Product => {
                    for conn in &graph.connections {
                        if conn.target_node == node.id {
                            if let Some(upstream) = results.get(&conn.origin_node) {
                                let upstream_vals: Vec<&MaterialStream> = upstream.iter().map(|(_, s)| s).collect();
                                if let Some(stream) = upstream_vals.get(conn.origin_slot).or_else(|| upstream_vals.first()) {
                                    total_output += stream.mass_flow;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let delta = (total_input - total_output).abs();
        let tolerance = (total_input.max(total_output) * 0.001).max(0.1);
        if delta > tolerance {
            return Err(EngineError::OverallBalanceError {
                input: total_input,
                output: total_output,
                delta,
            });
        }
        Ok(())
    }
}
