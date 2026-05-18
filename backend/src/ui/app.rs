//! 主应用状态与 UI 面板
//!
//! MassFlowApp 是桌面应用的核心，管理：
//! - Snarl 节点图数据（egui-snarl）
//! - graph_nodes 映射（snarl_id → 引擎 Node 结构体）
//! - 组分数据库 / 项目组分
//! - 计算引擎调用
//! - 各弹窗：组分数据库管理、项目组分管理、计算结果、日志

use eframe::egui;
use egui_snarl::Snarl;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::Database;
use crate::engine::Engine;
use crate::models::component::ComponentDef;
use crate::models::graph::{
    CalculatorOp, Connection, DecanterParams, FeedParams, FlowBasis,
    FlowGraph, Node, NodeParams, NodeType, Port, PortType, ReactorParams, ReactionDef,
    SeparatorKind, SeparatorParams, SplitMode, SplitterParams, UnitConversionParams,
};
use crate::models::stream::MaterialStream;

use super::canvas::MassFlowViewer;

// ==================== App State ====================

/// 主应用状态，实现 eframe::App trait
pub struct MassFlowApp {
    // --- 图数据 ---
    pub snarl: Snarl<MassFlowNode>,             // egui-snarl 管理的节点图
    pub graph_nodes: HashMap<usize, NodeData>,  // snarl内部ID → 节点数据
    pub connections: Vec<Connection>,           // 连线列表（计算用）
    next_node_num: usize,                       // 节点编号计数器

    // --- 引擎与数据 ---
    pub engine: Engine,
    pub db_components: Vec<ComponentDef>,       // SQLite 中所有组分
    pub project_components: Vec<ComponentDef>,  // 当前项目使用的组分

    // --- UI 状态 ---
    pub right_panel_open: bool,                 // 右侧项目组分面板
    pub status_msg: String,                     // 状态栏消息
    pub compute_results: Option<HashMap<Uuid, Vec<(String, MaterialStream)>>>,

    // 弹窗标志
    show_db_mgr: bool,                          // 组分数据库管理
    show_project_mgr: bool,                     // 项目组分管理
    show_results: bool,                         // 计算结果窗口
    show_log: bool,                             // 日志窗口
    log_messages: Vec<String>,                  // 内存日志缓冲（最多500条）

    new_comp: ComponentDef,                     // 添加组分的临时表单
    editing_components: std::collections::HashSet<String>, // 正在编辑的组分CAS集合
    current_file: Option<std::path::PathBuf>,   // 当前项目文件路径
}

/// 项目文件的序列化结构
#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectFile {
    graph: FlowGraph,
    project_components: Vec<ComponentDef>,
}

/// snarl 内部节点 ID 与引擎 Node 之间的映射
pub struct NodeData {
    pub id: Uuid,               // 引擎层的 UUID
    pub node: Node,             // 引擎 Node（用于计算和序列化）
    pub sn_node: MassFlowNode,  // UI 层节点数据（可编辑）
}

// ==================== 反应数据（仅 UI 使用） ====================

#[derive(Debug, Clone)]
pub struct ReactionData {
    pub key_component: String,                  // 关键组分 CAS
    pub conversion: f64,                        // 转化率 (0~1)
    pub stoichiometry: HashMap<String, f64>,    // CAS → 化学计量系数
}

#[derive(Debug, Clone)]
pub struct ReactorNode {
    pub reactions: Vec<ReactionData>,           // 可包含多个反应
}

// ==================== MassFlowNode — UI 层的节点数据 ====================

/// UI 层节点枚举，每个变体对应一种节点类型
///
/// 与 models::graph::NodeParams 不同，此枚举存储的是 UI 可编辑的数据，
/// 在保存/计算时通过 to_graph_node() 转换为引擎层数据结构
#[derive(Debug, Clone)]
pub enum MassFlowNode {
    Feed { cas: String, flow_basis: FlowBasis, total_flow: f64 },
    Product { label: String },
    Reactor(ReactorNode),
    Splitter { splits: Vec<f64> },
    Mixer,
    Separator { kind: SeparatorKind, split_mode: SplitMode },
    Decanter { split_mode: SplitMode },
    Calculator { op: CalculatorOp },
    UnitConversion { target_basis: FlowBasis },
    Copy { copies: usize },
    Filter,
}

impl MassFlowNode {
    /// 生成节点标题
    pub fn title(&self) -> String {
        match self {
            Self::Feed { cas, total_flow, .. } => format!("进料: {}@{:.1}", cas, total_flow),
            Self::Product { label } => if label.is_empty() { "出料".into() } else { format!("出料: {}", label) },
            Self::Reactor(rx) => format!("反应器: {}个反应", rx.reactions.len()),
            Self::Splitter { splits } => format!("分流: {}路", splits.len()),
            Self::Mixer => "混合器".into(),
            Self::Separator { kind, .. } => format!("分离器: {:?}", kind),
            Self::Decanter { .. } => "倾析器".into(),
            Self::Calculator { op } => format!("计算: {:?}", op),
            Self::UnitConversion { target_basis } => format!("变换→{:?}", target_basis),
            Self::Copy { copies } => format!("复制: {}路", copies),
            Self::Filter => "过滤".into(),
        }
    }

    /// 节点标题栏颜色
    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Feed { .. } => egui::Color32::from_rgb(0x16, 0x65, 0x34),
            Self::Product { .. } => egui::Color32::from_rgb(0x99, 0x1b, 0x1b),
            Self::Reactor { .. } => egui::Color32::from_rgb(0x9a, 0x34, 0x12),
            Self::Splitter { .. } => egui::Color32::from_rgb(0x6b, 0x21, 0xa8),
            Self::Mixer => egui::Color32::from_rgb(0x4c, 0x1d, 0x95),
            Self::Separator { .. } => egui::Color32::from_rgb(0x1e, 0x3a, 0x5f),
            Self::Decanter { .. } => egui::Color32::from_rgb(0x15, 0x5e, 0x75),
            Self::Calculator { .. } => egui::Color32::from_rgb(0x37, 0x41, 0x51),
            Self::UnitConversion { .. } => egui::Color32::from_rgb(0x85, 0x4d, 0x0e),
            Self::Copy { .. } => egui::Color32::from_rgb(0x0d, 0x5e, 0x5e),
            Self::Filter => egui::Color32::from_rgb(0x5e, 0x0d, 0x0d),
        }
    }

    /// 将 UI 层节点数据转换为引擎层 Node（包含完整的端口定义、参数等）
    pub fn to_graph_node(&self, id: Uuid, title: String) -> Node {
        // --- 输入端口定义 ---
        let inputs = match self {
            Self::Feed { .. } => vec![Port { name: "feed_in".into(), label: "输入".into(), port_type: PortType::Input }],
            Self::Product { .. } => vec![Port { name: "product_in".into(), label: "输入".into(), port_type: PortType::Input }],
            Self::Reactor { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
            Self::Splitter { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
            Self::Mixer => vec![
                Port { name: "in_1".into(), label: "入口1".into(), port_type: PortType::Input },
                Port { name: "in_2".into(), label: "入口2".into(), port_type: PortType::Input },
            ],
            Self::Separator { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
            Self::Decanter { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
            Self::Calculator { .. } => vec![
                Port { name: "in_A".into(), label: "A".into(), port_type: PortType::Input },
                Port { name: "in_B".into(), label: "B".into(), port_type: PortType::Input },
            ],
            Self::UnitConversion { .. } => vec![Port { name: "input".into(), label: "输入".into(), port_type: PortType::Input }],
            Self::Copy { .. } => vec![Port { name: "in".into(), label: "输入".into(), port_type: PortType::Input }],
            Self::Filter => vec![Port { name: "in".into(), label: "输入".into(), port_type: PortType::Input }],
        };

        // --- 输出端口定义 ---
        let outputs = match self {
            Self::Feed { .. } => vec![Port { name: "feed_out".into(), label: "输出".into(), port_type: PortType::Output }],
            Self::Product { .. } => vec![Port { name: "product_out".into(), label: "输出".into(), port_type: PortType::Output }],
            Self::Reactor { .. } => vec![Port { name: "outlet".into(), label: "出口".into(), port_type: PortType::Output }],
            Self::Mixer => vec![Port { name: "out".into(), label: "输出".into(), port_type: PortType::Output }],
            Self::Splitter { splits } => splits.iter().enumerate()
                .map(|(i, _)| Port { name: format!("out_{}", i+1), label: format!("出口{}", i+1), port_type: PortType::Output })
                .collect(),
            Self::Separator { kind, .. } => {
                let (a, b) = match kind {
                    SeparatorKind::GasLiquid => ("gas", "liquid"),
                    SeparatorKind::GasSolid => ("gas", "solid"),
                    SeparatorKind::General => ("outlet_1", "outlet_2"),
                };
                vec![
                    Port { name: a.into(), label: a.into(), port_type: PortType::Output },
                    Port { name: b.into(), label: b.into(), port_type: PortType::Output },
                ]
            }
            Self::Decanter { .. } => vec![
                Port { name: "light_phase".into(), label: "轻相".into(), port_type: PortType::Output },
                Port { name: "heavy_phase".into(), label: "重相".into(), port_type: PortType::Output },
            ],
            Self::Calculator { .. } => vec![Port { name: "result".into(), label: "结果".into(), port_type: PortType::Output }],
            Self::UnitConversion { .. } => vec![Port { name: "output".into(), label: "输出".into(), port_type: PortType::Output }],
            Self::Copy { copies } => (0..*copies).map(|i| Port { name: format!("out_{}", i+1), label: format!("复制{}", i+1), port_type: PortType::Output }).collect(),
            Self::Filter => vec![Port { name: "out".into(), label: "过滤".into(), port_type: PortType::Output }],
        };

        Node {
            id, title, inputs, outputs, pos: [0.0, 0.0], widget_values: None,
            node_type: match self {
                Self::Feed { .. } => NodeType::Feed, Self::Product { .. } => NodeType::Product,
                Self::Reactor { .. } => NodeType::Reactor, Self::Splitter { .. } => NodeType::Splitter,
                Self::Mixer => NodeType::Mixer, Self::Separator { .. } => NodeType::Separator,
                Self::Decanter { .. } => NodeType::Decanter, Self::Calculator { .. } => NodeType::Calculator,
                Self::UnitConversion { .. } => NodeType::UnitConversion,
                Self::Copy { .. } => NodeType::Copy, Self::Filter => NodeType::Filter,
            },
            // 将 UI 参数转换为引擎参数（Mixer/Product/Copy/Filter 无参数，设为 None）
            params: match self {
                Self::Feed { cas, flow_basis, total_flow } => Some(NodeParams::Feed(FeedParams { cas_number: cas.clone(), flow_basis: flow_basis.clone(), total_flow: *total_flow })),
                Self::Reactor(rx) => {
                    let reactions: Vec<ReactionDef> = rx.reactions.iter()
                        .filter(|r| !r.key_component.is_empty())
                        .map(|r| ReactionDef { stoichiometry: r.stoichiometry.clone(), key_component: r.key_component.clone(), conversion: r.conversion })
                        .collect();
                    Some(NodeParams::Reactor(ReactorParams { reactions }))
                },
                Self::Splitter { splits } => Some(NodeParams::Splitter(SplitterParams { splits: splits.clone() })),
                Self::Separator { kind, split_mode } => Some(NodeParams::Separator(SeparatorParams { separator_kind: kind.clone(), split_mode: split_mode.clone() })),
                Self::Decanter { split_mode } => {
                    let (pcs, pmr) = match split_mode {
                        SplitMode::Fraction { fractions } => (fractions.clone(), 0.0),       // pmr>=0 表示分率模式
                        SplitMode::FlowRate { flow_basis: _, flow_rates } => (flow_rates.clone(), -1.0), // pmr<0 表示流量模式
                    };
                    Some(NodeParams::Decanter(DecanterParams { partition_coefficients: pcs, phase_mass_ratio: pmr }))
                },
                Self::Calculator { op } => Some(NodeParams::Calculator(crate::models::graph::CalculatorParams { operation: op.clone() })),
                Self::UnitConversion { target_basis } => Some(NodeParams::UnitConversion(UnitConversionParams { target_basis: target_basis.clone() })),
                Self::Product { .. } | Self::Mixer | Self::Copy { .. } | Self::Filter => None,
            },
        }
    }
}

// ==================== 应用逻辑 ====================

impl MassFlowApp {
    pub fn new(db: Database) -> Self {
        let db_components = db.get_all_components().unwrap_or_default();
        let engine = Engine::new(db);
        Self {
            snarl: Snarl::new(),
            graph_nodes: HashMap::new(),
            connections: Vec::new(),
            next_node_num: 1,
            engine,
            db_components,
            project_components: Vec::new(),
            right_panel_open: false,
            status_msg: String::new(),
            compute_results: None,
            show_db_mgr: false,
            show_project_mgr: false,
            show_results: false,
            show_log: false,
            log_messages: Vec::new(),
            new_comp: ComponentDef { cas_number: String::new(), name: String::new(), formula: String::new(), molecular_weight: 0.0 },
            editing_components: std::collections::HashSet::new(),
            current_file: None,
        }
    }

    /// 内嵌日志（同时输出到 log crate 和内存缓冲）
    pub fn log(&mut self, msg: &str) {
        self.log_messages.push(msg.to_string());
        log::info!("{}", msg);
        if self.log_messages.len() > 500 { self.log_messages.remove(0); }
    }

    /// 将 snarl 中编辑过的参数同步到 graph_nodes 中的 Node 结构体
    /// 在保存和计算前必须调用，否则编辑不会持久化
    fn sync_params_from_snarl(&mut self) {
        for (nid, _pos, sn_node) in self.snarl.nodes_pos_ids() {
            if let Some(data) = self.graph_nodes.get_mut(&nid.0) {
                data.sn_node = sn_node.clone();
                data.node.params = match &data.sn_node {
                    MassFlowNode::Feed { cas, flow_basis, total_flow } =>
                        Some(NodeParams::Feed(FeedParams { cas_number: cas.clone(), flow_basis: flow_basis.clone(), total_flow: *total_flow })),
                    MassFlowNode::Reactor(rx) => {
                        let reactions: Vec<ReactionDef> = rx.reactions.iter()
                            .filter(|r| !r.key_component.is_empty())
                            .map(|r| ReactionDef { stoichiometry: r.stoichiometry.clone(), key_component: r.key_component.clone(), conversion: r.conversion })
                            .collect();
                        Some(NodeParams::Reactor(ReactorParams { reactions }))
                    },
                    MassFlowNode::Splitter { splits } => Some(NodeParams::Splitter(SplitterParams { splits: splits.clone() })),
                    MassFlowNode::Separator { kind, split_mode } => Some(NodeParams::Separator(SeparatorParams { separator_kind: kind.clone(), split_mode: split_mode.clone() })),
                    MassFlowNode::Decanter { split_mode } => {
                        let (pcs, pmr) = match split_mode {
                            SplitMode::Fraction { fractions } => (fractions.clone(), 0.0),
                            SplitMode::FlowRate { flow_basis: _, flow_rates } => (flow_rates.clone(), -1.0),
                        };
                        Some(NodeParams::Decanter(DecanterParams { partition_coefficients: pcs, phase_mass_ratio: pmr }))
                    },
                    MassFlowNode::Calculator { op } => Some(NodeParams::Calculator(crate::models::graph::CalculatorParams { operation: op.clone() })),
                    MassFlowNode::UnitConversion { target_basis } => Some(NodeParams::UnitConversion(UnitConversionParams { target_basis: target_basis.clone() })),
                    MassFlowNode::Product { .. } | MassFlowNode::Mixer | MassFlowNode::Copy { .. } | MassFlowNode::Filter => None,
                };
                data.node.title = data.sn_node.title();
            }
        }
    }

    /// 同步 snarl 连接到引擎连接列表，同时更新节点位置
    fn sync_from_snarl(&mut self) {
        self.sync_params_from_snarl();

        for (nid, pos, _node) in self.snarl.nodes_pos_ids() {
            if let Some(data) = self.graph_nodes.get_mut(&nid.0) {
                data.node.pos = [pos.x as f64, pos.y as f64];
            }
        }

        self.connections.clear();
        for (out_pin, in_pin) in self.snarl.wires() {
            self.connections.push(Connection {
                id: Uuid::new_v4(),
                origin_node: self.graph_nodes.get(&out_pin.node.0).map(|d| d.id).unwrap_or_default(),
                origin_slot: out_pin.output,
                target_node: self.graph_nodes.get(&in_pin.node.0).map(|d| d.id).unwrap_or_default(),
                target_slot: in_pin.input,
            });
        }
    }

    /// 构建计算用的 FlowGraph
    fn build_graph(&self) -> FlowGraph {
        let nodes: Vec<Node> = self.graph_nodes.values().map(|d| d.node.clone()).collect();
        FlowGraph { id: Uuid::nil(), name: "current".into(), nodes, connections: self.connections.clone() }
    }

    /// 执行物料平衡计算
    pub fn compute(&mut self) {
        self.sync_from_snarl();

        // 进料节点 CAS 校验
        for (_, data) in &self.graph_nodes {
            let node = &data.node;
            match &node.params {
                Some(NodeParams::Feed(p)) if p.cas_number.is_empty() => {
                    let msg = format!("错误: '{}' 未选择组分", node.title);
                    self.status_msg = msg.clone();
                    self.log(&msg);
                    return;
                }
                _ => {}
            }
        }

        let graph = self.build_graph();
        let msg = format!("执行计算: {}个节点, {}个连线", graph.nodes.len(), graph.connections.len());
        self.log(&msg);
        match self.engine.compute(&graph) {
            Ok(results) => {
                self.compute_results = Some(results);
                self.show_results = true;
                self.status_msg = "计算完成".into();
                self.log("计算完成");
            }
            Err(e) => {
                let msg = format!("计算失败: {}", e);
                self.status_msg = msg.clone();
                self.log(&msg);
            }
        }
    }

    /// 导出计算结果为 CSV 文件（UTF-8 BOM，Excel 兼容）
    fn export_results(&mut self) {
        let Some(ref results) = self.compute_results else { return };
        let path = rfd::FileDialog::new()
            .add_filter("CSV文件", &["csv"])
            .set_file_name("results.csv")
            .save_file();
        let Some(path) = path else { return };

        // 收集流股
        let mut streams: Vec<(String, &MaterialStream)> = Vec::new();
        for (id, outputs) in results {
            let node_label = self.graph_nodes.values()
                .find(|d| d.id == *id).map(|d| d.node.title.clone())
                .unwrap_or_else(|| id.to_string());
            for (port_name, stream) in outputs {
                streams.push((format!("{}→{}", node_label, port_name), stream));
            }
        }
        if streams.is_empty() { return; }

        let mut all_comps: Vec<String> = Vec::new();
        for (_, s) in &streams {
            for cas in s.components.keys() {
                if !all_comps.contains(cas) { all_comps.push(cas.clone()); }
            }
        }

        fn fmt_num(v: f64) -> String { if v.abs() < 0.0001 { String::new() } else { format!("{:.2}", v) } }

        let mut csv = String::from("\u{FEFF}"); // UTF-8 BOM

        // 表头
        csv.push_str("物料平衡计算结果\n");
        csv.push_str(",");
        for (name, _) in &streams { csv.push_str(&format!("{},", name)); }
        csv.push('\n');

        csv.push_str("流股号,");
        for i in 1..=streams.len() { csv.push_str(&format!("{},", i)); }
        csv.push('\n');

        // 摩尔流量
        csv.push_str("摩尔流量(kmol/h),");
        for (_, s) in &streams { csv.push_str(&format!("{},", fmt_num(s.mole_flow))); }
        csv.push('\n');
        for cas in &all_comps {
            let name = self.db_components.iter().find(|c| &c.cas_number == cas).map(|c| c.name.clone()).unwrap_or_else(|| cas.clone());
            csv.push_str(&format!("{},", name));
            for (_, s) in &streams {
                csv.push_str(&format!("{},", fmt_num(s.components.get(cas).map(|cf| cf.mole_flow).unwrap_or(0.0))));
            }
            csv.push('\n');
        }

        csv.push('\n');

        // 质量流量
        csv.push_str("质量流量(kg/h),");
        for (_, s) in &streams { csv.push_str(&format!("{},", fmt_num(s.mass_flow))); }
        csv.push('\n');
        for cas in &all_comps {
            let name = self.db_components.iter().find(|c| &c.cas_number == cas).map(|c| c.name.clone()).unwrap_or_else(|| cas.clone());
            csv.push_str(&format!("{},", name));
            for (_, s) in &streams {
                csv.push_str(&format!("{},", fmt_num(s.components.get(cas).map(|cf| cf.mass_flow).unwrap_or(0.0))));
            }
            csv.push('\n');
        }

        if let Err(e) = std::fs::write(&path, &csv) {
            self.status_msg = format!("导出失败: {}", e);
        } else {
            self.status_msg = format!("已导出: {}", path.file_name().unwrap_or_default().to_string_lossy());
        }
    }

    fn reload_db_components(&mut self) {
        self.db_components = self.engine.db().get_all_components().unwrap_or_default();
    }

    fn new_project(&mut self) {
        log::info!("新建项目");
        let nids: Vec<egui_snarl::NodeId> = self.snarl.nodes_pos_ids().map(|(id, _, _)| id).collect();
        for nid in nids { self.snarl.remove_node(nid); }
        self.graph_nodes.clear();
        self.connections.clear();
        self.project_components.clear();
        self.next_node_num = 1;
        self.compute_results = None;
        self.current_file = None;
        self.status_msg = "新建项目".into();
    }

    fn save_project_to(&mut self, path: &std::path::Path) {
        self.sync_from_snarl();
        let graph = self.build_graph();
        let proj = ProjectFile { graph, project_components: self.project_components.clone() };
        log::info!("保存项目到: {:?} ({}个节点, {}个组分)", path, proj.graph.nodes.len(), proj.project_components.len());
        match serde_json::to_string_pretty(&proj) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, &json) {
                    log::error!("保存失败: {}", e);
                    self.status_msg = format!("保存失败: {}", e);
                } else {
                    self.current_file = Some(path.to_path_buf());
                    self.status_msg = format!("已保存: {}", path.file_name().unwrap_or_default().to_string_lossy());
                    log::info!("保存成功");
                }
            }
            Err(e) => {
                log::error!("序列化失败: {}", e);
                self.status_msg = format!("序列化失败: {}", e);
            }
        }
    }

    fn save_project(&mut self) {
        log::info!("保存项目 (Ctrl+S)");
        if let Some(ref path) = self.current_file.clone() {
            self.save_project_to(path);
        } else {
            self.save_project_as();
        }
    }

    fn save_project_as(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("MassFlowCal 项目", &["mfc", "json"])
            .set_file_name("project.mfc")
            .save_file();
        if let Some(path) = path {
            self.save_project_to(&path);
        }
    }

    fn open_project(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("MassFlowCal 项目", &["mfc", "json"])
            .pick_file();
        if let Some(path) = path {
            log::info!("打开项目: {:?}", path);
            match std::fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<ProjectFile>(&json) {
                    Ok(proj) => {
                        log::info!("解析成功: {}节点, {}连线, {}组分", proj.graph.nodes.len(), proj.graph.connections.len(), proj.project_components.len());
                        self.new_project();
                        self.project_components = proj.project_components;

                        for node in &proj.graph.nodes {
                            let sn_node = self.graph_node_to_massflow(node);
                            let nid = self.snarl.insert_node(
                                egui::Pos2::new(node.pos[0] as f32, node.pos[1] as f32),
                                sn_node.clone(),
                            );
                            log::info!("  导入节点: {:?} -> snarl_id={}", node.id, nid.0);
                            self.graph_nodes.insert(nid.0, NodeData { id: node.id, node: node.clone(), sn_node });
                        }

                        for conn in &proj.graph.connections {
                            let origin = self.find_snarl_node(&conn.origin_node);
                            let target = self.find_snarl_node(&conn.target_node);
                            if origin.is_none() || target.is_none() {
                                log::warn!("  跳过连线: 找不到节点 {:?} -> {:?}", conn.origin_node, conn.target_node);
                                continue;
                            }
                            let result = self.snarl.connect(
                                egui_snarl::OutPinId { node: egui_snarl::NodeId(origin.unwrap()), output: conn.origin_slot },
                                egui_snarl::InPinId { node: egui_snarl::NodeId(target.unwrap()), input: conn.target_slot },
                            );
                            log::info!("  连线: {}:{} -> {}:{} ok={}", conn.origin_node, conn.origin_slot, conn.target_node, conn.target_slot, result);
                        }
                        self.current_file = Some(path);
                        self.status_msg = "项目已打开".into();
                    }
                    Err(e) => {
                        log::error!("解析失败: {}", e);
                        self.status_msg = format!("解析失败: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("读取失败: {}", e);
                    self.status_msg = format!("读取失败: {}", e);
                }
            }
        }
    }

    fn find_snarl_node(&self, uuid: &Uuid) -> Option<usize> {
        self.graph_nodes.iter().find(|(_, d)| d.id == *uuid).map(|(k, _)| *k)
    }

    /// 将引擎层 Node 转换回 UI 层 MassFlowNode（用于项目导入）
    fn graph_node_to_massflow(&self, node: &Node) -> MassFlowNode {
        match &node.params {
            Some(NodeParams::Feed(p)) => MassFlowNode::Feed { cas: p.cas_number.clone(), flow_basis: p.flow_basis.clone(), total_flow: p.total_flow },
            Some(NodeParams::Reactor(p)) => MassFlowNode::Reactor(ReactorNode {
                reactions: p.reactions.iter().map(|r| ReactionData {
                    key_component: r.key_component.clone(), conversion: r.conversion, stoichiometry: r.stoichiometry.clone(),
                }).collect(),
            }),
            Some(NodeParams::Splitter(p)) => MassFlowNode::Splitter { splits: p.splits.clone() },
            Some(NodeParams::Separator(p)) => MassFlowNode::Separator { kind: p.separator_kind.clone(), split_mode: p.split_mode.clone() },
            Some(NodeParams::Decanter(p)) => {
                let split_mode = if p.phase_mass_ratio < 0.0 {
                    SplitMode::FlowRate { flow_basis: FlowBasis::Mass, flow_rates: p.partition_coefficients.clone() }
                } else {
                    SplitMode::Fraction { fractions: p.partition_coefficients.clone() }
                };
                MassFlowNode::Decanter { split_mode }
            },
            Some(NodeParams::Calculator(p)) => MassFlowNode::Calculator { op: p.operation.clone() },
            Some(NodeParams::UnitConversion(p)) => MassFlowNode::UnitConversion { target_basis: p.target_basis.clone() },
            None => match node.node_type {
                NodeType::Mixer => MassFlowNode::Mixer,
                NodeType::Product => MassFlowNode::Product { label: node.title.clone() },
                NodeType::Copy => MassFlowNode::Copy { copies: node.outputs.len().max(2) },
                NodeType::Filter => MassFlowNode::Filter,
                _ => MassFlowNode::Product { label: node.title.clone() },
            },
        }
    }

    // ==================== 组分数据库管理 UI ====================

    fn component_mgr_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_db_mgr;
        egui::Window::new("组分数据库")
            .open(&mut open).resizable(true).default_size([650.0, 450.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("刷新").clicked() { self.reload_db_components(); }
                    ui.label(format!("共 {} 个组分", self.db_components.len()));
                });
                ui.separator();

                ui.collapsing("添加新组分", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("CAS"); ui.add_sized([100.0, 20.0], egui::TextEdit::singleline(&mut self.new_comp.cas_number));
                        ui.label("名称"); ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut self.new_comp.name));
                        ui.label("分子式"); ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut self.new_comp.formula));
                        ui.label("MW"); ui.add_sized([70.0, 20.0], egui::DragValue::new(&mut self.new_comp.molecular_weight).speed(0.1).min_decimals(1).max_decimals(3));
                        if ui.button("确认添加").clicked() && !self.new_comp.cas_number.is_empty() {
                            if let Err(e) = self.engine.db().insert_component(&self.new_comp) {
                                self.status_msg = format!("添加失败: {}", e);
                            } else {
                                self.new_comp = ComponentDef { cas_number: String::new(), name: String::new(), formula: String::new(), molecular_weight: 0.0 };
                                self.reload_db_components();
                            }
                        }
                    });
                });
                ui.separator();

                // 组分列表（可编辑）
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut to_delete: Option<String> = None;
                    let mut changed = false;
                    egui::Grid::new("db_grid").striped(true).min_col_width(80.0).show(ui, |ui| {
                        for idx in 0..self.db_components.len() {
                            let comp = &mut self.db_components[idx];
                            let cas = comp.cas_number.clone();
                            let is_editing = self.editing_components.contains(&cas);
                            if is_editing {
                                ui.add_sized([100.0, 20.0], egui::TextEdit::singleline(&mut comp.cas_number));
                                ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut comp.name));
                                ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut comp.formula));
                                ui.add_sized([70.0, 20.0], egui::DragValue::new(&mut comp.molecular_weight).speed(0.1).min_decimals(1).max_decimals(3));
                                if ui.button("保存").clicked() { let _ = self.engine.db().update_component(comp); self.editing_components.remove(&cas); changed = true; }
                            } else {
                                ui.label(&comp.cas_number); ui.label(&comp.name);
                                ui.label(&comp.formula); ui.label(format!("{:.3}", comp.molecular_weight));
                                if ui.button("编辑").clicked() { self.editing_components.insert(cas.clone()); }
                            }
                            if ui.button("删除").clicked() { to_delete = Some(cas.clone()); }
                            if ui.button("加入项目").clicked() && !self.project_components.iter().any(|c| c.cas_number == comp.cas_number) {
                                self.project_components.push(comp.clone());
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(cas) = to_delete {
                        let _ = self.engine.db().delete_component(&cas);
                        self.project_components.retain(|c| c.cas_number != cas);
                        self.editing_components.remove(&cas);
                        changed = true;
                    }
                    if changed { self.reload_db_components(); }
                });
            });
        self.show_db_mgr = open;
    }

    /// 项目组分管理 UI
    fn project_mgr_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_project_mgr;
        egui::Window::new("项目组分")
            .open(&mut open).resizable(true).default_size([400.0, 300.0])
            .show(ctx, |ui| {
                ui.label(format!("当前项目共 {} 个组分", self.project_components.len()));
                ui.separator();
                let mut to_remove: Option<usize> = None;
                egui::Grid::new("proj_grid").striped(true).min_col_width(100.0).show(ui, |ui| {
                    for (i, comp) in self.project_components.iter().enumerate() {
                        ui.label(&comp.cas_number); ui.label(&comp.name);
                        ui.label(format!("{:.3}", comp.molecular_weight));
                        if ui.button("移除").clicked() { to_remove = Some(i); }
                        ui.end_row();
                    }
                });
                if let Some(i) = to_remove { self.project_components.remove(i); }
                if self.project_components.is_empty() { ui.label("从组分数据库中加入组分到项目"); }
            });
        self.show_project_mgr = open;
    }
}

// ==================== eframe::App 实现 ====================

impl eframe::App for MassFlowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ctrl+S 保存
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.save_project();
        }

        // === 顶部工具栏 ===
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("新建项目").clicked() { self.new_project(); ui.close(); }
                    if ui.button("打开项目...").clicked() { self.open_project(); ui.close(); }
                    ui.separator();
                    if ui.button("保存 (Ctrl+S)").clicked() { self.save_project(); ui.close(); }
                    if ui.button("另存为...").clicked() { self.save_project_as(); ui.close(); }
                });
                ui.separator();
                if ui.button("计算").clicked() { self.compute(); }
                ui.separator();
                if ui.button("组分数据库").clicked() { self.show_db_mgr = !self.show_db_mgr; }
                if ui.button("项目组分").clicked() { self.show_project_mgr = !self.show_project_mgr; }
                if ui.button("日志").clicked() { self.show_log = !self.show_log; }
                ui.label(&self.status_msg);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("☰ 面板").clicked() { self.right_panel_open = !self.right_panel_open; }
                });
            });
        });

        // 弹窗
        if self.show_db_mgr { self.component_mgr_ui(ctx); }
        if self.show_project_mgr { self.project_mgr_ui(ctx); }

        // === 中央画布 ===
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut viewer = MassFlowViewer::new(&self.project_components);
            let style = egui_snarl::ui::SnarlStyle::default();
            self.snarl.show(&mut viewer, &style, (), ui);

            // 处理右键菜单选择的节点
            if let Some((node, pos)) = viewer.pending_node.take() {
                let id = Uuid::new_v4();
                let nn = self.next_node_num;
                self.next_node_num += 1;
                let title = format!("{} #{}", node.title(), nn);
                let graph_node = node.to_graph_node(id, title);
                let nid = self.snarl.insert_node(pos, node.clone());
                self.graph_nodes.insert(nid.0, NodeData { id, node: graph_node, sn_node: node });
            }
            if let Some(nid) = viewer.pending_remove.take() {
                self.snarl.remove_node(nid);
                self.graph_nodes.remove(&nid.0);
            }
        });

        // === 右侧面板 ===
        if self.right_panel_open {
            egui::SidePanel::right("result_panel").show(ctx, |ui| {
                ui.heading("项目组分");
                for comp in &self.project_components {
                    ui.label(format!("{} ({})", comp.name, comp.cas_number));
                }
                if self.project_components.is_empty() { ui.label("从组分数据库中添加到项目"); }
            });
        }

        // === 计算结果窗口 ===
        if self.show_results {
            let mut open = self.show_results;
            egui::Window::new("计算结果")
                .open(&mut open).resizable(true).default_size([750.0, 550.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| { if ui.button("导出结果").clicked() { self.export_results(); } });
                    if let Some(ref results) = self.compute_results {
                        egui::ScrollArea::both().show(ui, |ui| {
                            let mut streams: Vec<(String, &MaterialStream)> = Vec::new();
                            let mut gid = 0usize;
                            for (id, outputs) in results {
                                let node_label = self.graph_nodes.values().find(|d| d.id == *id)
                                    .map(|d| d.node.title.clone()).unwrap_or_else(|| id.to_string());
                                for (port_name, stream) in outputs {
                                    streams.push((format!("{} → {}", node_label, port_name), stream));
                                }
                            }
                            for (stream_name, stream) in &streams {
                                let sid = gid; gid += 1;
                                ui.collapsing(format!("{}##col{}", stream_name, sid), |ui| {
                                    ui.label(format!("总质量: {:.3} kg/h  |  总摩尔: {:.4} kmol/h  |  组分数: {}",
                                        stream.mass_flow, stream.mole_flow, stream.components.len()));
                                    ui.separator();
                                    egui::Grid::new(format!("st_{}", sid)).striped(true).min_col_width(100.0).show(ui, |ui| {
                                        ui.label("组分"); ui.label("质量流量(kg/h)");
                                        ui.label("摩尔流量(kmol/h)"); ui.label("质量分率");
                                        ui.end_row();
                                        for (cas, cf) in &stream.components {
                                            let name = self.db_components.iter().find(|c| c.cas_number == *cas)
                                                .map(|c| c.name.as_str()).unwrap_or(cas.as_str());
                                            ui.label(name);
                                            ui.label(format!("{:.4}", cf.mass_flow));
                                            ui.label(format!("{:.4}", cf.mole_flow));
                                            ui.label(format!("{:.1}%", cf.mass_fraction * 100.0));
                                            ui.end_row();
                                        }
                                    });
                                });
                            }
                        });
                    }
                });
            self.show_results = open;
        }

        // === 日志窗口 ===
        if self.show_log {
            egui::Window::new("日志").resizable(true).default_size([500.0, 300.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true)
                        .show(ui, |ui| {
                            for msg in &self.log_messages { ui.label(msg); }
                        });
                });
        }
    }
}
