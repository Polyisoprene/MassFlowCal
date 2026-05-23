//! UI ↔ 引擎层数据转换
//!
//! MassFlowNode (GUI) ↔ Node (Engine) 的双向转换

use uuid::Uuid;

use mfc_core::graph::{
    CalculatorParams, DecanterParams, FeedParams, FilterParams, FlowBasis, Node, NodeParams,
    NodeType, Port, PortType, ReactionDef, ReactorParams, SeparatorKind, SeparatorParams,
    SplitMode, SplitterParams,
};

use super::nodes::{MassFlowNode, MassFlowNodeKind, ReactionData, ReactorNode};

/// 将 UI 层节点转换为引擎层 Node
pub fn to_graph_node(sn_node: &MassFlowNode, id: Uuid, title: String) -> Node {
    let inputs = match &sn_node.kind {
        MassFlowNodeKind::Feed { .. } => vec![Port { name: "feed_in".into(), label: "输入".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Product { .. } => vec![Port { name: "product_in".into(), label: "输入".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Reactor { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Splitter { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Mixer => vec![
            Port { name: "in_1".into(), label: "入口1".into(), port_type: PortType::Input },
            Port { name: "in_2".into(), label: "入口2".into(), port_type: PortType::Input },
        ],
        MassFlowNodeKind::Separator { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Decanter { .. } => vec![Port { name: "feed".into(), label: "进料".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Calculator { .. } => vec![
            Port { name: "in_A".into(), label: "A".into(), port_type: PortType::Input },
            Port { name: "in_B".into(), label: "B".into(), port_type: PortType::Input },
        ],
        MassFlowNodeKind::Copy { .. } => vec![Port { name: "in".into(), label: "输入".into(), port_type: PortType::Input }],
        MassFlowNodeKind::Filter { .. } => vec![Port { name: "in".into(), label: "输入".into(), port_type: PortType::Input }],
    };

    let outputs = match &sn_node.kind {
        MassFlowNodeKind::Feed { .. } => vec![Port { name: "feed_out".into(), label: "输出".into(), port_type: PortType::Output }],
        MassFlowNodeKind::Product { .. } => vec![Port { name: "product_out".into(), label: "输出".into(), port_type: PortType::Output }],
        MassFlowNodeKind::Reactor { .. } => vec![Port { name: "outlet".into(), label: "出口".into(), port_type: PortType::Output }],
        MassFlowNodeKind::Mixer => vec![Port { name: "out".into(), label: "输出".into(), port_type: PortType::Output }],
        MassFlowNodeKind::Splitter { splits } => splits.iter().enumerate()
            .map(|(i, _)| Port { name: format!("out_{}", i+1), label: format!("出口{}", i+1), port_type: PortType::Output })
            .collect(),
        MassFlowNodeKind::Separator { kind, .. } => {
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
        MassFlowNodeKind::Decanter { .. } => vec![
            Port { name: "light_phase".into(), label: "轻相".into(), port_type: PortType::Output },
            Port { name: "heavy_phase".into(), label: "重相".into(), port_type: PortType::Output },
        ],
        MassFlowNodeKind::Calculator { .. } => vec![Port { name: "result".into(), label: "结果".into(), port_type: PortType::Output }],
        MassFlowNodeKind::Copy { copies } => (0..*copies).map(|i| Port { name: format!("out_{}", i+1), label: format!("复制{}", i+1), port_type: PortType::Output }).collect(),
        MassFlowNodeKind::Filter { .. } => vec![Port { name: "out".into(), label: "过滤".into(), port_type: PortType::Output }],
    };

    Node {
        id, title, inputs, outputs, pos: [0.0, 0.0],
        custom_name: sn_node.custom_name.clone(),
        node_type: match &sn_node.kind {
            MassFlowNodeKind::Feed { .. } => NodeType::Feed,
            MassFlowNodeKind::Product { .. } => NodeType::Product,
            MassFlowNodeKind::Reactor { .. } => NodeType::Reactor,
            MassFlowNodeKind::Splitter { .. } => NodeType::Splitter,
            MassFlowNodeKind::Mixer => NodeType::Mixer,
            MassFlowNodeKind::Separator { .. } => NodeType::Separator,
            MassFlowNodeKind::Decanter { .. } => NodeType::Decanter,
            MassFlowNodeKind::Calculator { .. } => NodeType::Calculator,
            MassFlowNodeKind::Copy { .. } => NodeType::Copy,
            MassFlowNodeKind::Filter { .. } => NodeType::Filter,
        },
        params: match &sn_node.kind {
            MassFlowNodeKind::Feed { cas, flow_basis, total_flow } =>
                Some(NodeParams::Feed(FeedParams { cas_number: cas.clone(), flow_basis: flow_basis.clone(), total_flow: *total_flow })),
            MassFlowNodeKind::Reactor(rx) => {
                let reactions: Vec<ReactionDef> = rx.reactions.iter()
                    .filter(|r| !r.key_component.is_empty())
                    .map(|r| ReactionDef { stoichiometry: r.stoichiometry.clone(), key_component: r.key_component.clone(), conversion: r.conversion })
                    .collect();
                Some(NodeParams::Reactor(ReactorParams { reactions }))
            },
            MassFlowNodeKind::Splitter { splits } =>
                Some(NodeParams::Splitter(SplitterParams { splits: splits.clone() })),
            MassFlowNodeKind::Separator { kind, split_mode } =>
                Some(NodeParams::Separator(SeparatorParams { separator_kind: kind.clone(), split_mode: split_mode.clone() })),
            MassFlowNodeKind::Decanter { split_mode } => {
                let (pcs, pmr) = match split_mode {
                    SplitMode::Fraction { fractions } => (fractions.clone(), 0.0),
                    SplitMode::FlowRate { flow_basis: _, flow_rates } => (flow_rates.clone(), -1.0),
                };
                Some(NodeParams::Decanter(DecanterParams { partition_coefficients: pcs, phase_mass_ratio: pmr }))
            },
            MassFlowNodeKind::Calculator { op } =>
                Some(NodeParams::Calculator(CalculatorParams { operation: op.clone() })),
            MassFlowNodeKind::Filter { cas, flow_basis } =>
                Some(NodeParams::Filter(FilterParams { cas_number: cas.clone(), flow_basis: flow_basis.clone() })),
            MassFlowNodeKind::Product { .. } | MassFlowNodeKind::Mixer | MassFlowNodeKind::Copy { .. } => None,
        },
    }
}

/// 将引擎层 Node 转换回 UI 层 MassFlowNode
pub fn graph_node_to_massflow(node: &Node) -> MassFlowNode {
    let kind = match &node.params {
        Some(NodeParams::Feed(p)) =>
            MassFlowNodeKind::Feed { cas: p.cas_number.clone(), flow_basis: p.flow_basis.clone(), total_flow: p.total_flow },
        Some(NodeParams::Reactor(p)) => MassFlowNodeKind::Reactor(ReactorNode {
            reactions: p.reactions.iter().map(|r| ReactionData {
                name: String::new(), key_component: r.key_component.clone(), conversion: r.conversion, stoichiometry: r.stoichiometry.clone(),
            }).collect(),
        }),
        Some(NodeParams::Splitter(p)) => MassFlowNodeKind::Splitter { splits: p.splits.clone() },
        Some(NodeParams::Separator(p)) =>
            MassFlowNodeKind::Separator { kind: p.separator_kind.clone(), split_mode: p.split_mode.clone() },
        Some(NodeParams::Decanter(p)) => {
            let split_mode = if p.phase_mass_ratio < 0.0 {
                SplitMode::FlowRate { flow_basis: FlowBasis::Mass, flow_rates: p.partition_coefficients.clone() }
            } else {
                SplitMode::Fraction { fractions: p.partition_coefficients.clone() }
            };
            MassFlowNodeKind::Decanter { split_mode }
        },
        Some(NodeParams::Calculator(p)) => MassFlowNodeKind::Calculator { op: p.operation.clone() },
        Some(NodeParams::Filter(p)) => MassFlowNodeKind::Filter { cas: p.cas_number.clone(), flow_basis: p.flow_basis.clone() },
        None => match node.node_type {
            NodeType::Mixer => MassFlowNodeKind::Mixer,
            NodeType::Product => MassFlowNodeKind::Product { label: node.title.clone() },
            NodeType::Copy => MassFlowNodeKind::Copy { copies: node.outputs.len().max(2) },
            NodeType::Filter => MassFlowNodeKind::Filter { cas: String::new(), flow_basis: FlowBasis::Mass },
            _ => MassFlowNodeKind::Product { label: node.title.clone() },
        },
    };
    MassFlowNode { kind, custom_name: node.custom_name.clone() }
}

/// 将 snarl 节点数据同步到 graph_nodes
pub fn sync_params(sn_node: &MassFlowNode) -> (Option<NodeParams>, Option<String>, String) {
    let params = match &sn_node.kind {
        MassFlowNodeKind::Feed { cas, flow_basis, total_flow } =>
            Some(NodeParams::Feed(FeedParams { cas_number: cas.clone(), flow_basis: flow_basis.clone(), total_flow: *total_flow })),
        MassFlowNodeKind::Reactor(rx) => {
            let reactions: Vec<ReactionDef> = rx.reactions.iter()
                .filter(|r| !r.key_component.is_empty())
                .map(|r| ReactionDef { stoichiometry: r.stoichiometry.clone(), key_component: r.key_component.clone(), conversion: r.conversion })
                .collect();
            Some(NodeParams::Reactor(ReactorParams { reactions }))
        },
        MassFlowNodeKind::Splitter { splits } =>
            Some(NodeParams::Splitter(SplitterParams { splits: splits.clone() })),
        MassFlowNodeKind::Separator { kind, split_mode } =>
            Some(NodeParams::Separator(SeparatorParams { separator_kind: kind.clone(), split_mode: split_mode.clone() })),
        MassFlowNodeKind::Decanter { split_mode } => {
            let (pcs, pmr) = match split_mode {
                SplitMode::Fraction { fractions } => (fractions.clone(), 0.0),
                SplitMode::FlowRate { flow_basis: _, flow_rates } => (flow_rates.clone(), -1.0),
            };
            Some(NodeParams::Decanter(DecanterParams { partition_coefficients: pcs, phase_mass_ratio: pmr }))
        },
        MassFlowNodeKind::Calculator { op } =>
            Some(NodeParams::Calculator(CalculatorParams { operation: op.clone() })),
        MassFlowNodeKind::Filter { cas, flow_basis } =>
            Some(NodeParams::Filter(FilterParams { cas_number: cas.clone(), flow_basis: flow_basis.clone() })),
        MassFlowNodeKind::Product { .. } | MassFlowNodeKind::Mixer | MassFlowNodeKind::Copy { .. } => None,
    };
    let custom_name = sn_node.custom_name.clone();
    let title = sn_node.title();
    (params, custom_name, title)
}
