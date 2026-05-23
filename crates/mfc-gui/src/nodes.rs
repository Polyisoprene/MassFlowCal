//! UI 层节点类型定义
//!
//! MassFlowNode/MassFlowNodeKind 是 GUI 专用的节点表示，
//! 与引擎层的 Node/NodeParams 分离，通过 convert 模块相互转换。

use std::collections::HashMap;
use mfc_core::graph::{CalculatorOp, FlowBasis, SeparatorKind, SplitMode};

/// UI 层节点：包含节点类型和用户自定义名称
#[derive(Debug, Clone)]
pub struct MassFlowNode {
    pub kind: MassFlowNodeKind,
    pub custom_name: Option<String>,
}

/// UI 层节点类型枚举
#[derive(Debug, Clone)]
pub enum MassFlowNodeKind {
    Feed { cas: String, flow_basis: FlowBasis, total_flow: f64 },
    Product { label: String },
    Reactor(ReactorNode),
    Splitter { splits: Vec<f64> },
    Mixer,
    Separator { kind: SeparatorKind, split_mode: SplitMode },
    Decanter { split_mode: SplitMode },
    Calculator { op: CalculatorOp },
    Copy { copies: usize },
    Filter { cas: String, flow_basis: FlowBasis },
}

/// 反应器节点的 UI 数据
#[derive(Debug, Clone)]
pub struct ReactorNode {
    pub reactions: Vec<ReactionData>,
}

/// 单个反应的 UI 编辑数据
#[derive(Debug, Clone)]
pub struct ReactionData {
    pub name: String,
    pub key_component: String,
    pub conversion: f64,
    pub stoichiometry: HashMap<String, f64>,
}

impl MassFlowNode {
    pub fn title(&self) -> String {
        if let Some(ref name) = self.custom_name {
            return name.clone();
        }
        self.kind.title()
    }
}

impl MassFlowNodeKind {
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
            Self::Copy { copies } => format!("复制: {}路", copies),
            Self::Filter { cas, .. } => if cas.is_empty() { "过滤".into() } else { format!("过滤: {}", cas) },
        }
    }
}
