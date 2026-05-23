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

use mfc_core::error::EngineError;
use mfc_core::graph::{FlowBasis, FlowGraph, Node, NodeParams, NodeType};
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

use nodes::NodeOutputs;

/// 计算引擎（无状态）
pub struct Engine;

impl Engine {
    /// 执行全图计算的主要入口
    ///
    /// `provider` 提供组分分子量查询，可以是 SQLite 数据库、内存缓存等任意实现
    ///
    /// 返回 HashMap<node_id, Vec<(port_name, stream)>> — 每个节点的每个输出端口一个流
    pub fn compute(
        graph: &FlowGraph,
        provider: &dyn ComponentProvider,
    ) -> Result<HashMap<Uuid, NodeOutputs>, EngineError> {
        // 1. 构建 DAG + 拓扑排序
        let (dag, node_map) = dag::build_dag(graph)?;
        let order = dag::compute_order(&dag, &node_map)?;

        // 2. 建立 node_id → Node 查找
        let node_lookup: HashMap<Uuid, &Node> = graph.nodes.iter().map(|n| (n.id, n)).collect();

        // 3. 按拓扑序执行
        let mut results: HashMap<Uuid, NodeOutputs> = HashMap::new();
        for node_id in &order {
            let node = node_lookup.get(node_id).ok_or_else(|| {
                EngineError::Computation(format!("节点 {} 不存在", node_id))
            })?;
            let outputs = Self::compute_node(node, graph, &results, provider)?;
            results.insert(node.id, outputs);
        }

        // 4. 全局物料平衡校验
        Self::check_overall_balance(graph, &results)?;
        Ok(results)
    }

    /// 计算单个节点
    fn compute_node(
        node: &Node,
        graph: &FlowGraph,
        results: &HashMap<Uuid, NodeOutputs>,
        provider: &dyn ComponentProvider,
    ) -> Result<NodeOutputs, EngineError> {
        let inputs = nodes::collect_inputs(node, &graph.connections, results)?;

        let params = match &node.params {
            Some(p) => p,
            None => {
                return match node.node_type {
                    NodeType::Mixer => nodes::mixer::compute_mixer(&inputs, provider),
                    NodeType::Copy => {
                        let stream = inputs.values().next().cloned().unwrap_or_default();
                        let outputs: NodeOutputs = node.outputs.iter()
                            .map(|p| (p.name.clone(), stream.clone()))
                            .collect();
                        Ok(outputs)
                    }
                    NodeType::Filter => {
                        Err(EngineError::Computation("Filter: 缺少参数".into()))
                    }
                    _ => {
                        let stream = inputs.values().next().cloned().unwrap_or_default();
                        Ok(vec![("product_out".to_string(), stream)])
                    }
                };
            }
        };

        match params {
            NodeParams::Feed(p) => {
                if let Some(upstream) = inputs.values().next() {
                    if !upstream.is_empty() && !p.cas_number.is_empty() {
                        let mw = provider.get_mw(&p.cas_number)?;
                        let stream = match p.flow_basis {
                            FlowBasis::Mass => MaterialStream::from_pure_mass(&p.cas_number, upstream.mass_flow, mw),
                            FlowBasis::Mole => MaterialStream::from_pure_mole(&p.cas_number, upstream.mass_flow, mw),
                        };
                        return Ok(vec![("feed_out".to_string(), stream)]);
                    }
                }
                let stream = nodes::feed::compute_feed(p, provider)?;
                Ok(vec![("feed_out".to_string(), stream)])
            }
            NodeParams::Reactor(p) => nodes::reactor::compute_reactor(p, &inputs, provider),
            NodeParams::Splitter(p) => nodes::splitter::compute_splitter(p, &inputs, provider),
            NodeParams::Separator(p) => nodes::separator::compute_separator(p, &inputs, provider),
            NodeParams::Decanter(p) => nodes::decanter::compute_decanter(p, &inputs, provider),
            NodeParams::Calculator(p) => nodes::calculator::compute_calculator(p, &inputs, provider),
            NodeParams::Filter(p) => {
                let upstream = inputs.values().next().cloned().unwrap_or_default();
                if upstream.is_empty() || p.cas_number.is_empty() {
                    return Err(EngineError::Computation("Filter: 上游流为空或未选择组分".into()));
                }
                let comp_flow = upstream.components.get(&p.cas_number)
                    .ok_or_else(|| EngineError::Computation(format!("Filter: 上游中未找到组分 {}", p.cas_number)))?;
                let value = match p.flow_basis {
                    FlowBasis::Mass => comp_flow.mass_flow,
                    FlowBasis::Mole => comp_flow.mole_flow,
                };
                let stream = MaterialStream::from_pure_value(&p.cas_number, value);
                Ok(vec![("out".to_string(), stream)])
            },
        }
    }

    /// 全局物料平衡检验
    fn check_overall_balance(
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
