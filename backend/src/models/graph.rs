//! 流程图的完整数据模型定义
//!
//! 包含图结构、节点、端口、连线、以及所有节点类型的参数定义。
//! 所有结构体均实现 Serialize/Deserialize 以支持 JSON 文件持久化。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ==================== 流程图 ====================

/// 完整的流程定义：节点列表 + 连线列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowGraph {
    pub id: Uuid,
    pub name: String,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
}

// ==================== 节点 ====================

/// 一个单元操作节点
///
/// params 为 None 的节点：Product, Mixer, Copy, Filter
/// params 为 Some 的节点：Feed, Reactor, Splitter, Separator, Decanter, Calculator, UnitConversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub node_type: NodeType,
    pub title: String,          // 显示标题，如 "进料 #1"
    pub pos: [f64; 2],          // 画布坐标 [x, y]
    pub params: Option<NodeParams>,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub widget_values: Option<serde_json::Value>, // 保留字段（Web版兼容）
}

/// 端口定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub name: String,           // 内部标识，如 "feed_in", "top"
    pub label: String,          // 显示标签
    pub port_type: PortType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PortType {
    Input,
    Output,
}

// ==================== 节点类型枚举 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodeType {
    Feed,
    Product,
    Reactor,
    Splitter,
    Mixer,
    Separator,
    Decanter,
    Calculator,
    UnitConversion,
    Copy,       // 复制节点：1 进 N 出，全相同
    Filter,     // 过滤节点：提取单组分
}

// ==================== 节点参数（各类型互斥） ====================
// 使用外部标签序列化：{"Feed": {...}}, {"Reactor": {...}} 等

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeParams {
    Feed(FeedParams),
    Reactor(ReactorParams),
    Splitter(SplitterParams),
    Separator(SeparatorParams),
    Decanter(DecanterParams),
    Calculator(CalculatorParams),
    UnitConversion(UnitConversionParams),
}

// ==================== Feed 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedParams {
    /// 组分的 CAS 号，如 "7732-18-5"
    pub cas_number: String,
    /// 流量基准：Mass(kg/h) 或 Mole(kmol/h)
    pub flow_basis: FlowBasis,
    /// 总流量（按 flow_basis 单位）
    pub total_flow: f64,
}

/// 流量基准枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowBasis {
    Mass,   // kg/h
    Mole,   // kmol/h
}

// ==================== Reactor 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorParams {
    /// 一个反应器可包含多个反应
    pub reactions: Vec<ReactionDef>,
}

/// 单个反应的定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionDef {
    /// 化学计量系数：CAS → 系数（反应物为负，产物为正）
    pub stoichiometry: HashMap<String, f64>,
    /// 关键组分 CAS 号
    pub key_component: String,
    /// 关键组分转化率 (0~1)
    pub conversion: f64,
}

// ==================== Splitter 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitterParams {
    /// 各出口的分割分率，之和应为 1.0
    pub splits: Vec<f64>,
}

// ==================== Separator 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeparatorParams {
    /// 分离器硬件类型
    pub separator_kind: SeparatorKind,
    /// 分割方式
    pub split_mode: SplitMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeparatorKind {
    GasLiquid,  // 气液分离
    GasSolid,   // 气固分离
    General,    // 通用
}

/// 分割方式：分率模式或流量模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SplitMode {
    /// 按分率：CAS → 进入出口1的比例 (0~1)
    Fraction {
        fractions: HashMap<String, f64>,
    },
    /// 按流量：CAS → 进入出口1的绝对流量
    FlowRate {
        flow_basis: FlowBasis,
        flow_rates: HashMap<String, f64>,
    },
}

// ==================== Decanter 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecanterParams {
    /// 各组分进入轻相的直接分率 (0~1) 或流量（取决于 phase_mass_ratio 标志）
    pub partition_coefficients: HashMap<String, f64>,
    /// < 0 表示 FlowRate 模式，>= 0 表示 Fraction 模式
    pub phase_mass_ratio: f64,
}

// ==================== Calculator 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorParams {
    pub operation: CalculatorOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CalculatorOp {
    Add,            // A + B
    Subtract,       // A - B
    Multiply(f64),  // A × 常数
    Divide(f64),    // A ÷ 常数
}

// ==================== UnitConversion 参数 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitConversionParams {
    pub target_basis: FlowBasis,
}

// ==================== 连线 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    /// 源节点 UUID
    pub origin_node: Uuid,
    /// 源输出端口在 outputs 数组中的索引
    pub origin_slot: usize,
    /// 目标节点 UUID
    pub target_node: Uuid,
    /// 目标输入端口在 inputs 数组中的索引
    pub target_slot: usize,
}
