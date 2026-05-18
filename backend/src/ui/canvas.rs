use egui::*;
use egui_snarl::ui::*;
use egui_snarl::*;

use crate::models::component::ComponentDef;
use crate::models::graph::{CalculatorOp, FlowBasis, SeparatorKind, SplitMode};
use super::app::{MassFlowNode, ReactionData};

pub struct MassFlowViewer<'a> {
    pub pending_node: Option<(MassFlowNode, Pos2)>,
    pub pending_remove: Option<NodeId>,
    pub components: &'a [ComponentDef],
}

impl<'a> MassFlowViewer<'a> {
    pub fn new(components: &'a [ComponentDef]) -> Self {
        Self { pending_node: None, pending_remove: None, components }
    }
}

fn component_combo(ui: &mut Ui, cas: &mut String, components: &[ComponentDef], id: impl std::hash::Hash) {
    let mut selected = components.iter().position(|c| c.cas_number == *cas);
    let labels: Vec<String> = components.iter().map(|c| format!("{} ({})", c.name, c.cas_number)).collect();
    let preview = if let Some(i) = selected { labels[i].clone() } else if cas.is_empty() { "选择组分...".into() } else { cas.clone() };
    egui::ComboBox::from_id_salt(id)
        .width(160.0)
        .selected_text(preview)
        .show_ui(ui, |ui| {
            for (i, c) in components.iter().enumerate() {
                if ui.selectable_value(&mut selected, Some(i), &labels[i]).clicked() {
                    *cas = c.cas_number.clone();
                }
            }
        });
}

fn draw_reaction(
    ui: &mut Ui,
    idx: usize,
    rx: &mut ReactionData,
    components: &[ComponentDef],
) -> bool {
    let mut remove = false;
    ui.spacing_mut().item_spacing.y = 1.0;
    ui.set_max_width(260.0);
    ui.label(format!("反应 #{}", idx + 1));
    ui.horizontal(|ui| {
        ui.label("组分");
        narrow_combo(ui, &mut rx.key_component, components, ("rk", idx));
        ui.add_sized([55.0, 16.0], egui::DragValue::new(&mut rx.conversion).speed(0.01).min_decimals(2).max_decimals(3).range(0.0..=1.0));
        if ui.small_button("X").clicked() { remove = true; }
    });
    let mut to_remove_key: Option<String> = None;
    let mut add_new = false;
    let mut entries: Vec<(String, f64)> = rx.stoichiometry.iter().map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| {
        if a.0.is_empty() { std::cmp::Ordering::Greater }
        else if b.0.is_empty() { std::cmp::Ordering::Less }
        else { a.0.cmp(&b.0) }
    });
    for (ei, (cas_key, coeff)) in entries.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            narrow_combo(ui, cas_key, components, ("sc", idx, ei));
            ui.add_sized([45.0, 16.0], egui::DragValue::new(coeff).speed(0.1));
            if ui.small_button("X").clicked() { to_remove_key = Some(cas_key.clone()); }
        });
    }
    if let Some(ref rm) = to_remove_key { entries.retain(|(k, _)| k != rm); }
    rx.stoichiometry = entries.into_iter().collect();
    // 检查编辑后的状态
    if rx.stoichiometry.iter().all(|(k, _)| !k.is_empty()) {
        if ui.small_button("+ 组分").clicked() { add_new = true; }
    }
    if add_new { rx.stoichiometry.insert(String::new(), 0.0); }
    remove
}

fn draw_split_editor(ui: &mut Ui, split_mode: &mut SplitMode, components: &[ComponentDef]) {
    ui.spacing_mut().item_spacing.y = 1.0;
    match split_mode {
        SplitMode::Fraction { fractions } => {
            let mut to_remove: Option<String> = None;
            let mut add_new = false;
            let mut items: Vec<(String, f64)> = fractions.iter().map(|(k,v)| (k.clone(), *v)).collect();
            items.sort_by(|a,b| { if a.0.is_empty() { std::cmp::Ordering::Greater } else if b.0.is_empty() { std::cmp::Ordering::Less } else { a.0.cmp(&b.0) } });
            let has_empty = items.iter().any(|(k,_)| k.is_empty());
            for (ei, (cas_key, frac)) in items.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    narrow_combo(ui, cas_key, components, ("sep_f", ei));
                    ui.add_sized([50.0, 16.0], egui::DragValue::new(frac).speed(0.01).range(0.0..=1.0).min_decimals(2));
                    if ui.small_button("X").clicked() { to_remove = Some(cas_key.clone()); }
                });
            }
            // 先写回再删除（否则写回会覆盖删除）
            *fractions = items.into_iter().collect();
            if let Some(ref key) = to_remove { fractions.remove(key); }
            if !has_empty {
                if ui.small_button("+ 组分").clicked() { add_new = true; }
            }
            if add_new { fractions.insert(String::new(), 0.0); }
        }
        SplitMode::FlowRate { flow_basis, flow_rates } => {
            egui::ComboBox::from_id_salt("sp_fb").width(160.0)
                .selected_text(format!("{:?}", flow_basis))
                .show_ui(ui, |ui| {
                    ui.selectable_value(flow_basis, FlowBasis::Mass, "Mass (kg/h)");
                    ui.selectable_value(flow_basis, FlowBasis::Mole, "Mole (kmol/h)");
                });
            let mut to_remove: Option<String> = None;
            let mut add_new = false;
            let mut items: Vec<(String, f64)> = flow_rates.iter().map(|(k,v)| (k.clone(), *v)).collect();
            items.sort_by(|a,b| { if a.0.is_empty() { std::cmp::Ordering::Greater } else if b.0.is_empty() { std::cmp::Ordering::Less } else { a.0.cmp(&b.0) } });
            let has_empty = items.iter().any(|(k,_)| k.is_empty());
            for (ei, (cas_key, rate)) in items.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    narrow_combo(ui, cas_key, components, ("sep_r", ei));
                    ui.add_sized([60.0, 16.0], egui::DragValue::new(rate).speed(1.0).min_decimals(2));
                    if ui.small_button("X").clicked() { to_remove = Some(cas_key.clone()); }
                });
            }
            *flow_rates = items.into_iter().collect();
            if let Some(ref key) = to_remove { flow_rates.remove(key); }
            if !has_empty {
                if ui.small_button("+ 组分").clicked() { add_new = true; }
            }
            if add_new { flow_rates.insert(String::new(), 0.0); }
        }
    }
}

fn narrow_combo(ui: &mut Ui, cas: &mut String, components: &[ComponentDef], id: impl std::hash::Hash) {
    let preview = components.iter().find(|c| c.cas_number == *cas).map(|c| c.name.clone())
        .unwrap_or_else(|| if cas.is_empty() { "选择".into() } else { cas.clone() });
    egui::ComboBox::from_id_salt(id)
        .width(120.0)
        .selected_text(preview)
        .show_ui(ui, |ui| {
            for c in components.iter() {
                if ui.selectable_label(c.cas_number == *cas, &c.name).clicked() {
                    *cas = c.cas_number.clone();
                }
            }
        });
}

impl<'a> SnarlViewer<MassFlowNode> for MassFlowViewer<'a> {
    fn title(&mut self, node: &MassFlowNode) -> String {
        match node {
            MassFlowNode::Feed { cas, total_flow, .. } => {
                let name = self.components.iter().find(|c| c.cas_number == *cas).map(|c| c.name.as_str()).unwrap_or(cas.as_str());
                format!("进料: {}@{:.1}", name, total_flow)
            }
            MassFlowNode::Product { label } => {
                if label.is_empty() { "出料".into() } else { format!("出料: {}", label) }
            }
            MassFlowNode::Reactor(rx) => {
                format!("反应器: {}个反应", rx.reactions.len())
            }
            MassFlowNode::Splitter { splits } => format!("分流: {}路", splits.len()),
            MassFlowNode::Mixer => "混合器".into(),
            MassFlowNode::Separator { kind, .. } => format!("分离器: {:?}", kind),
            MassFlowNode::Decanter { .. } => "倾析器".into(),
            MassFlowNode::Calculator { op } => format!("计算: {:?}", op),
            MassFlowNode::UnitConversion { target_basis } => format!("变换→{:?}", target_basis),
            MassFlowNode::Copy { copies } => format!("复制: {}路", copies),
            MassFlowNode::Filter => "过滤".into(),
        }
    }

    fn inputs(&mut self, node: &MassFlowNode) -> usize {
        match node {
            MassFlowNode::Feed { .. } => 1,
            MassFlowNode::Product { .. } => 1,
            MassFlowNode::Reactor(..) => 1,
            MassFlowNode::Splitter { .. } => 1,
            MassFlowNode::Mixer => 2,
            MassFlowNode::Separator { .. } => 1,
            MassFlowNode::Decanter { .. } => 1,
            MassFlowNode::Calculator { .. } => 2,
            MassFlowNode::UnitConversion { .. } => 1,
            MassFlowNode::Copy { .. } => 1,
            MassFlowNode::Filter => 1,
        }
    }

    fn outputs(&mut self, node: &MassFlowNode) -> usize {
        match node {
            MassFlowNode::Product { .. } => 1,
            MassFlowNode::Mixer => 1,
            MassFlowNode::Splitter { splits } => splits.len(),
            MassFlowNode::Separator { .. } => 2,
            MassFlowNode::Decanter { .. } => 2,
            MassFlowNode::Calculator { .. } => 1,
            MassFlowNode::Copy { copies } => *copies,
            MassFlowNode::Filter => 1,
            _ => 1,
        }
    }

    fn show_input(&mut self, _pin: &InPin, _ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) -> impl SnarlPin + 'static {
        PinInfo::circle().with_fill(Color32::from_rgb(0x06, 0xb6, 0xd4))
    }

    fn show_output(&mut self, _pin: &OutPin, _ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) -> impl SnarlPin + 'static {
        PinInfo::circle().with_fill(Color32::from_rgb(0xf9, 0x73, 0x16))
    }

    fn has_body(&mut self, node: &MassFlowNode) -> bool {
        !matches!(node, MassFlowNode::Product { .. } | MassFlowNode::Mixer)
    }

    fn show_body(
        &mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<MassFlowNode>,
    ) {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
        let mut node_data = snarl[node].clone();
        let comps = self.components;

        match &mut node_data {
            MassFlowNode::Feed { cas, flow_basis, total_flow } => {
                let has_input = !snarl.in_pin(egui_snarl::InPinId { node, input: 0 }).remotes.is_empty();
                ui.vertical(|ui| {
                    component_combo(ui, cas, comps, "feed_cas");
                    ui.label("基准");
                    egui::ComboBox::from_id_salt("fb").width(160.0)
                        .selected_text(format!("{:?}", flow_basis))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(flow_basis, FlowBasis::Mass, "Mass (kg/h)");
                            ui.selectable_value(flow_basis, FlowBasis::Mole, "Mole (kmol/h)");
                        });
                    if !has_input {
                        ui.label("总流量");
                        ui.add_sized([160.0, 18.0], egui::DragValue::new(total_flow).speed(1.0).min_decimals(1).max_decimals(4));
                    } else {
                        ui.label("总流量");
                        ui.label("上游输入");
                    }
                });
            }
            MassFlowNode::Reactor(rx) => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    let mut remove_indices: Vec<usize> = Vec::new();
                    for i in 0..rx.reactions.len() {
                        if i > 0 { ui.separator(); }
                        if draw_reaction(ui, i, &mut rx.reactions[i], comps) {
                            remove_indices.push(i);
                        }
                    }
                    for i in remove_indices.iter().rev() { rx.reactions.remove(*i); }
                    if ui.small_button("+ 添加反应").clicked() {
                        rx.reactions.push(ReactionData {
                            key_component: String::new(), conversion: 0.95,
                            stoichiometry: std::collections::HashMap::new(),
                        });
                    }
                });
            }
            MassFlowNode::Product { label } => {
                ui.label("标签");
                ui.add_sized([160.0, 18.0], egui::TextEdit::singleline(label));
            }
            MassFlowNode::Splitter { splits } => {
                ui.vertical(|ui| {
                    let n = splits.len();
                    for i in 0..n - 1 {
                        ui.label(format!("出口{}分率", i + 1));
                        let mut val = splits[i];
                        if ui.add_sized([80.0, 18.0], egui::DragValue::new(&mut val).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(3)).changed() {
                            let remaining = 1.0 - val;
                            splits[i] = val;
                            // 均分剩余给其他出口
                            if n > 2 {
                                let rest_each = remaining / (n - 1) as f64;
                                for j in 0..n { if j != i { splits[j] = rest_each; } }
                            } else {
                                splits[1 - i] = remaining;
                            }
                        }
                    }
                    let last = 1.0 - splits[..n-1].iter().sum::<f64>();
                    ui.label(format!("出口{}分率 (自动)", n));
                    ui.label(format!("{:.3}", last.max(0.0)));
                });
            }
            MassFlowNode::Separator { kind, split_mode } => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    egui::ComboBox::from_id_salt("sk").width(160.0)
                        .selected_text(format!("{:?}", kind))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(kind, SeparatorKind::GasLiquid, "气液分离");
                            ui.selectable_value(kind, SeparatorKind::GasSolid, "气固分离");
                            ui.selectable_value(kind, SeparatorKind::General, "通用");
                        });
                    draw_split_editor(ui, split_mode, comps);
                });
            }
            MassFlowNode::Decanter { split_mode } => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    // 模式切换
                    let mut is_fraction = matches!(split_mode, SplitMode::Fraction { .. });
                    let mode_label = if is_fraction { "按分率" } else { "按流量" };
                    egui::ComboBox::from_id_salt("dm").width(160.0)
                        .selected_text(mode_label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut is_fraction, true, "按分率").clicked() {
                                *split_mode = SplitMode::Fraction { fractions: std::collections::HashMap::new() };
                            }
                            if ui.selectable_value(&mut is_fraction, false, "按流量").clicked() {
                                *split_mode = SplitMode::FlowRate { flow_basis: FlowBasis::Mass, flow_rates: std::collections::HashMap::new() };
                            }
                        });
                    draw_split_editor(ui, split_mode, comps);
                });
            }
            MassFlowNode::Calculator { op } => {
                ui.vertical(|ui| {
                    let mut idx = match op {
                        CalculatorOp::Add => 0, CalculatorOp::Subtract => 1,
                        CalculatorOp::Multiply(_) => 2, CalculatorOp::Divide(_) => 3,
                    };
                    ui.label("运算类型");
                    egui::ComboBox::from_id_salt("co").width(160.0)
                        .selected_text(format!("{:?}", op))
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut idx, 0, "Add (+)").clicked() { *op = CalculatorOp::Add; }
                            if ui.selectable_value(&mut idx, 1, "Sub (−)").clicked() { *op = CalculatorOp::Subtract; }
                            if ui.selectable_value(&mut idx, 2, "Mul (×)").clicked() { *op = CalculatorOp::Multiply(1.0); }
                            if ui.selectable_value(&mut idx, 3, "Div (÷)").clicked() { *op = CalculatorOp::Divide(1.0); }
                        });
                    if let CalculatorOp::Multiply(v) | CalculatorOp::Divide(v) = op {
                        ui.label("常数");
                        ui.add_sized([160.0, 18.0], egui::DragValue::new(v).speed(0.1));
                    }
                });
            }
            MassFlowNode::UnitConversion { target_basis } => {
                ui.label("目标基准");
                egui::ComboBox::from_id_salt("tb").width(160.0)
                    .selected_text(format!("{:?}", target_basis))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(target_basis, FlowBasis::Mass, "Mass (kg/h)");
                        ui.selectable_value(target_basis, FlowBasis::Mole, "Mole (kmol/h)");
                    });
            }
            MassFlowNode::Copy { copies } => {
                ui.label("复制份数");
                let mut c = *copies as f64;
                if ui.add_sized([60.0, 18.0], egui::DragValue::new(&mut c).speed(1).range(1..=10)).changed() {
                    *copies = c as usize;
                }
            }
            MassFlowNode::Filter => {
                ui.label("将上游多组分流过滤为单组分");
            }
            _ => {}
        }

        *snarl.get_node_mut(node).unwrap() = node_data;
    }

    fn has_node_menu(&mut self, _node: &MassFlowNode) -> bool { true }
    fn show_node_menu(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) {
        if ui.button("删除节点").clicked() { self.pending_remove = Some(node); ui.close(); }
    }

    fn has_graph_menu(&mut self, _pos: Pos2, _snarl: &mut Snarl<MassFlowNode>) -> bool { true }
    fn show_graph_menu(&mut self, pos: Pos2, ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) {
        ui.set_min_width(200.0);
        let items: Vec<(&str, MassFlowNode)> = vec![
            ("进料 (Feed)", MassFlowNode::Feed { cas: String::new(), flow_basis: FlowBasis::Mass, total_flow: 1000.0 }),
            ("出料 (Product)", MassFlowNode::Product { label: String::new() }),
            ("反应器 (Reactor)", MassFlowNode::Reactor(super::app::ReactorNode { reactions: vec![ReactionData { key_component: String::new(), conversion: 0.95, stoichiometry: std::collections::HashMap::new() }] })),
            ("分流器 (Splitter)", MassFlowNode::Splitter { splits: vec![0.5, 0.5] }),
            ("混合器 (Mixer)", MassFlowNode::Mixer),
            ("分离器 (Separator)", MassFlowNode::Separator { kind: SeparatorKind::GasLiquid, split_mode: SplitMode::Fraction { fractions: std::collections::HashMap::new() } }),
            ("倾析器 (Decanter)", MassFlowNode::Decanter { split_mode: SplitMode::Fraction { fractions: std::collections::HashMap::new() } }),
            ("计算器 (Calculator)", MassFlowNode::Calculator { op: CalculatorOp::Add }),
            ("单位变换 (UnitConversion)", MassFlowNode::UnitConversion { target_basis: FlowBasis::Mass }),
            ("复制 (Copy)", MassFlowNode::Copy { copies: 2 }),
            ("过滤 (Filter)", MassFlowNode::Filter),
        ];
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (name, node) in &items {
                if ui.button(*name).clicked() { self.pending_node = Some((node.clone(), pos)); ui.close(); }
            }
        });
    }
}
