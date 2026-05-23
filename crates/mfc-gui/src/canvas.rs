use std::collections::HashMap;
use egui::*;
use egui_ariel::ui::*;
use egui_ariel::*;

use mfc_core::component::ComponentDef;
use mfc_core::graph::{CalculatorOp, FlowBasis, SeparatorKind, SplitMode};
use super::nodes::{MassFlowNode, MassFlowNodeKind, ReactionData, ReactorNode};

pub struct MassFlowViewer<'a> {
    pub pending_node: Option<(MassFlowNode, Pos2)>,
    pub pending_remove: Option<NodeId>,
    pub pending_rename: Option<(NodeId, String)>,
    pub pending_wire_rename: Option<(OutPinId, InPinId, String)>,
    pub pending_wire_menu: Option<(OutPinId, InPinId, Pos2)>,
    pub pending_label_detail: Option<(OutPinId, InPinId, String)>,
    pub pending_edit_reaction: Option<(NodeId, usize, ReactionData)>,
    pub components: &'a [ComponentDef],
    pub wire_labels: Vec<(OutPinId, InPinId, String)>,
}

impl<'a> MassFlowViewer<'a> {
    pub fn new(components: &'a [ComponentDef]) -> Self {
        Self {
            pending_node: None, pending_remove: None, pending_rename: None,
            pending_wire_rename: None, pending_wire_menu: None,
            pending_label_detail: None, pending_edit_reaction: None, components,
            wire_labels: Vec::new(),
        }
    }
}

fn component_combo(ui: &mut Ui, cas: &mut String, components: &[ComponentDef], id: impl std::hash::Hash) {
    let mut selected = components.iter().position(|c| c.cas_number == *cas);
    let labels: Vec<String> = components.iter().map(|c| format!("{} ({})", c.name, c.cas_number)).collect();
    let preview = if let Some(i) = selected { labels[i].clone() } else if cas.is_empty() { "选择组分...".into() } else { cas.clone() };
    egui::ComboBox::from_id_salt(id).width(160.0).selected_text(preview)
        .show_ui(ui, |ui| {
            for (i, c) in components.iter().enumerate() {
                if ui.selectable_value(&mut selected, Some(i), &labels[i]).clicked() { *cas = c.cas_number.clone(); }
            }
        });
}


fn draw_split_editor(ui: &mut Ui, split_mode: &mut SplitMode, components: &[ComponentDef]) {
    ui.spacing_mut().item_spacing.y = 1.0;
    match split_mode {
        SplitMode::Fraction { fractions } => {
            let mut to_remove: Option<String> = None; let mut add_new = false;
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
            *fractions = items.into_iter().collect();
            if let Some(ref key) = to_remove { fractions.remove(key); }
            if !has_empty { if ui.small_button("+ 组分").clicked() { add_new = true; } }
            if add_new { fractions.insert(String::new(), 0.0); }
        }
        SplitMode::FlowRate { flow_basis, flow_rates } => {
            egui::ComboBox::from_id_salt("sp_fb").width(160.0).selected_text(format!("{:?}", flow_basis))
                .show_ui(ui, |ui| {
                    ui.selectable_value(flow_basis, FlowBasis::Mass, "Mass (kg/h)");
                    ui.selectable_value(flow_basis, FlowBasis::Mole, "Mole (kmol/h)");
                });
            let mut to_remove: Option<String> = None; let mut add_new = false;
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
            if !has_empty { if ui.small_button("+ 组分").clicked() { add_new = true; } }
            if add_new { flow_rates.insert(String::new(), 0.0); }
        }
    }
}

fn narrow_combo(ui: &mut Ui, cas: &mut String, components: &[ComponentDef], id: impl std::hash::Hash) {
    let preview = components.iter().find(|c| c.cas_number == *cas).map(|c| c.name.clone())
        .unwrap_or_else(|| if cas.is_empty() { "选择".into() } else { cas.clone() });
    egui::ComboBox::from_id_salt(id).width(120.0).selected_text(preview)
        .show_ui(ui, |ui| {
            for c in components.iter() {
                if ui.selectable_label(c.cas_number == *cas, &c.name).clicked() { *cas = c.cas_number.clone(); }
            }
        });
}

impl<'a> SnarlViewer<MassFlowNode> for MassFlowViewer<'a> {
    fn title(&mut self, node: &MassFlowNode) -> String {
        if let Some(ref name) = node.custom_name { return name.clone(); }
        match &node.kind {
            MassFlowNodeKind::Feed { cas, total_flow, .. } => {
                let name = self.components.iter().find(|c| c.cas_number == *cas).map(|c| c.name.as_str()).unwrap_or(cas.as_str());
                format!("进料: {}@{:.1}", name, total_flow)
            }
            MassFlowNodeKind::Product { label } => if label.is_empty() { "出料".into() } else { format!("出料: {}", label) },
            MassFlowNodeKind::Reactor(rx) => format!("反应器: {}个反应", rx.reactions.len()),
            MassFlowNodeKind::Splitter { splits } => format!("分流: {}路", splits.len()),
            MassFlowNodeKind::Mixer => "混合器".into(),
            MassFlowNodeKind::Separator { kind, .. } => format!("分离器: {:?}", kind),
            MassFlowNodeKind::Decanter { .. } => "倾析器".into(),
            MassFlowNodeKind::Calculator { op } => format!("计算: {:?}", op),
            MassFlowNodeKind::Copy { copies } => format!("复制: {}路", copies),
            MassFlowNodeKind::Filter { cas, .. } => if cas.is_empty() { "过滤".into() } else { format!("过滤: {}", cas) },
        }
    }

    fn inputs(&mut self, node: &MassFlowNode) -> usize {
        match &node.kind {
            MassFlowNodeKind::Feed { .. } | MassFlowNodeKind::Product { .. } | MassFlowNodeKind::Reactor(..) | MassFlowNodeKind::Splitter { .. } => 1,
            MassFlowNodeKind::Separator { .. } | MassFlowNodeKind::Decanter { .. } => 1,
            MassFlowNodeKind::Copy { .. } | MassFlowNodeKind::Filter { .. } => 1,
            MassFlowNodeKind::Mixer | MassFlowNodeKind::Calculator { .. } => 2,
        }
    }

    fn outputs(&mut self, node: &MassFlowNode) -> usize {
        match &node.kind {
            MassFlowNodeKind::Splitter { splits } => splits.len(),
            MassFlowNodeKind::Separator { .. } | MassFlowNodeKind::Decanter { .. } => 2,
            MassFlowNodeKind::Copy { copies } => *copies,
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
        !matches!(&node.kind, MassFlowNodeKind::Product { .. } | MassFlowNodeKind::Mixer)
    }

    fn show_body(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<MassFlowNode>) {
        ui.spacing_mut().item_spacing.y = 2.0; ui.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
        let mut node_data = snarl[node].clone();
        let comps = self.components;
        match &mut node_data.kind {
            MassFlowNodeKind::Feed { cas, flow_basis, total_flow } => {
                let has_input = !snarl.in_pin(egui_ariel::InPinId { node, input: 0 }).remotes.is_empty();
                ui.vertical(|ui| {
                    component_combo(ui, cas, comps, "feed_cas");
                    ui.label("基准");
                    egui::ComboBox::from_id_salt("fb").width(160.0).selected_text(format!("{:?}", flow_basis))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(flow_basis, FlowBasis::Mass, "Mass (kg/h)");
                            ui.selectable_value(flow_basis, FlowBasis::Mole, "Mole (kmol/h)");
                        });
                    if !has_input { ui.label("总流量"); ui.add_sized([160.0, 18.0], egui::DragValue::new(total_flow).speed(1.0).min_decimals(1).max_decimals(4)); }
                    else { ui.label("总流量"); ui.label("上游输入"); }
                });
            }
            MassFlowNodeKind::Reactor(rx) => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    if rx.reactions.is_empty() {
                        ui.label("暂无反应");
                    } else {
                        let mut to_delete: Option<usize> = None;
                        let mut to_edit: Option<usize> = None;
                        for (i, r) in rx.reactions.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let name = if r.name.is_empty() { format!("反应 #{}", i + 1) } else { r.name.clone() };
                                ui.label(name);
                                if ui.small_button("编辑").clicked() { to_edit = Some(i); }
                                if ui.small_button("删除").clicked() { to_delete = Some(i); }
                            });
                        }
                        if let Some(i) = to_delete { rx.reactions.remove(i); }
                        if let Some(i) = to_edit {
                            self.pending_edit_reaction = Some((node, i, rx.reactions[i].clone()));
                        }
                    }
                    if ui.button("+ 添加反应").clicked() {
                        rx.reactions.push(ReactionData {
                            name: String::new(), key_component: String::new(),
                            conversion: 0.95, stoichiometry: HashMap::new(),
                        });
                    }
                });
            }
            MassFlowNodeKind::Product { label } => {
                ui.label("标签"); ui.add_sized([160.0, 18.0], egui::TextEdit::singleline(label));
            }
            MassFlowNodeKind::Splitter { splits } => {
                ui.vertical(|ui| {
                    let n = splits.len();
                    for i in 0..n - 1 {
                        ui.label(format!("出口{}分率", i + 1));
                        let mut val = splits[i];
                        if ui.add_sized([80.0, 18.0], egui::DragValue::new(&mut val).speed(0.01).range(0.0..=1.0).min_decimals(2).max_decimals(3)).changed() {
                            let remaining = 1.0 - val; splits[i] = val;
                            if n > 2 { let rest_each = remaining / (n - 1) as f64; for j in 0..n { if j != i { splits[j] = rest_each; } } }
                            else { splits[1 - i] = remaining; }
                        }
                    }
                    let last = 1.0 - splits[..n-1].iter().sum::<f64>();
                    ui.label(format!("出口{}分率 (自动)", n)); ui.label(format!("{:.3}", last.max(0.0)));
                });
            }
            MassFlowNodeKind::Separator { kind, split_mode } => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    egui::ComboBox::from_id_salt("sk").width(160.0).selected_text(format!("{:?}", kind))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(kind, SeparatorKind::GasLiquid, "气液分离");
                            ui.selectable_value(kind, SeparatorKind::GasSolid, "气固分离");
                            ui.selectable_value(kind, SeparatorKind::General, "通用");
                        });
                    draw_split_editor(ui, split_mode, comps);
                });
            }
            MassFlowNodeKind::Decanter { split_mode } => {
                ui.set_max_width(280.0);
                ui.vertical(|ui| {
                    let mut is_fraction = matches!(split_mode, SplitMode::Fraction { .. });
                    egui::ComboBox::from_id_salt("dm").width(160.0).selected_text(if is_fraction { "按分率" } else { "按流量" })
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut is_fraction, true, "按分率").clicked() { *split_mode = SplitMode::Fraction { fractions: std::collections::HashMap::new() }; }
                            if ui.selectable_value(&mut is_fraction, false, "按流量").clicked() { *split_mode = SplitMode::FlowRate { flow_basis: FlowBasis::Mass, flow_rates: std::collections::HashMap::new() }; }
                        });
                    draw_split_editor(ui, split_mode, comps);
                });
            }
            MassFlowNodeKind::Calculator { op } => {
                ui.vertical(|ui| {
                    let mut idx = match op { CalculatorOp::Add => 0, CalculatorOp::Subtract => 1, CalculatorOp::Multiply(_) => 2, CalculatorOp::Divide(_) => 3 };
                    ui.label("运算类型");
                    egui::ComboBox::from_id_salt("co").width(160.0).selected_text(format!("{:?}", op))
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut idx, 0, "Add (+)").clicked() { *op = CalculatorOp::Add; }
                            if ui.selectable_value(&mut idx, 1, "Sub (-)").clicked() { *op = CalculatorOp::Subtract; }
                            if ui.selectable_value(&mut idx, 2, "Mul (*)").clicked() { *op = CalculatorOp::Multiply(1.0); }
                            if ui.selectable_value(&mut idx, 3, "Div (/)").clicked() { *op = CalculatorOp::Divide(1.0); }
                        });
                    if let CalculatorOp::Multiply(v) | CalculatorOp::Divide(v) = op { ui.label("常数"); ui.add_sized([160.0, 18.0], egui::DragValue::new(v).speed(0.1)); }
                });
            }
            MassFlowNodeKind::Copy { copies } => {
                ui.label("复制份数"); let mut c = *copies as f64;
                if ui.add_sized([60.0, 18.0], egui::DragValue::new(&mut c).speed(1).range(1..=10)).changed() { *copies = c as usize; }
            }
            MassFlowNodeKind::Filter { cas, flow_basis } => {
                ui.vertical(|ui| {
                    ui.label("提取组分"); component_combo(ui, cas, comps, "filter_cas");
                    ui.label("输出基准");
                    egui::ComboBox::from_id_salt("filter_fb").width(160.0).selected_text(format!("{:?}", flow_basis))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(flow_basis, FlowBasis::Mass, "Mass (kg/h)");
                            ui.selectable_value(flow_basis, FlowBasis::Mole, "Mole (kmol/h)");
                        });
                });
            }
            _ => {}
        }
        *snarl.get_node_mut(node).unwrap() = node_data;
    }

    fn has_node_menu(&mut self, _node: &MassFlowNode) -> bool { true }
    fn show_node_menu(&mut self, node: NodeId, _inputs: &[InPin], _outputs: &[OutPin], ui: &mut Ui, snarl: &mut Snarl<MassFlowNode>) {
        if ui.button("重命名").clicked() {
            let current = snarl[node].custom_name.clone().unwrap_or_default();
            self.pending_rename = Some((node, current)); ui.close();
        }
        if ui.button("删除节点").clicked() { self.pending_remove = Some(node); ui.close(); }
    }

    fn has_graph_menu(&mut self, _pos: Pos2, _snarl: &mut Snarl<MassFlowNode>) -> bool { true }
    fn show_graph_menu(&mut self, pos: Pos2, ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) {
        ui.set_min_width(200.0);
        let mk = |kind: MassFlowNodeKind| -> MassFlowNode { MassFlowNode { kind, custom_name: None } };
        let items: Vec<(&str, MassFlowNode)> = vec![
            ("进料 (Feed)", mk(MassFlowNodeKind::Feed { cas: String::new(), flow_basis: FlowBasis::Mass, total_flow: 1000.0 })),
            ("出料 (Product)", mk(MassFlowNodeKind::Product { label: String::new() })),
            ("反应器 (Reactor)", mk(MassFlowNodeKind::Reactor(ReactorNode { reactions: vec![ReactionData { name: String::new(), key_component: String::new(), conversion: 0.95, stoichiometry: std::collections::HashMap::new() }] }))),
            ("分流器 (Splitter)", mk(MassFlowNodeKind::Splitter { splits: vec![0.5, 0.5] })),
            ("混合器 (Mixer)", mk(MassFlowNodeKind::Mixer)),
            ("分离器 (Separator)", mk(MassFlowNodeKind::Separator { kind: SeparatorKind::GasLiquid, split_mode: SplitMode::Fraction { fractions: std::collections::HashMap::new() } })),
            ("倾析器 (Decanter)", mk(MassFlowNodeKind::Decanter { split_mode: SplitMode::Fraction { fractions: std::collections::HashMap::new() } })),
            ("计算器 (Calculator)", mk(MassFlowNodeKind::Calculator { op: CalculatorOp::Add })),
            ("复制 (Copy)", mk(MassFlowNodeKind::Copy { copies: 2 })),
            ("过滤 (Filter)", mk(MassFlowNodeKind::Filter { cas: String::new(), flow_basis: FlowBasis::Mass })),
        ];
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (name, node) in &items { if ui.button(*name).clicked() { self.pending_node = Some((node.clone(), pos)); ui.close(); } }
        });
    }

    fn has_wire_widget(&mut self, from: &OutPinId, to: &InPinId, _snarl: &Snarl<MassFlowNode>) -> bool {
        self.wire_labels.iter().any(|(f, t, _)| f == from && t == to)
    }
    fn show_wire_widget(&mut self, from: &OutPin, to: &InPin, ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) {
        let label = self.wire_labels.iter().find(|(f, t, _)| f == &from.id && t == &to.id)
            .map(|(_, _, l)| l.clone()).unwrap_or_default();
        if label.is_empty() { return; }
        // 计算标签尺寸，与 draw_foreground 保持一致
        let font_id = egui::FontId::proportional(14.0);
        let lines: Vec<std::sync::Arc<egui::Galley>> = label.split('\n').map(|line| {
            ui.painter().layout_no_wrap(line.to_string(), font_id.clone(), egui::Color32::WHITE)
        }).collect();
        let total_h: f32 = lines.iter().map(|l| l.size().y).sum::<f32>() + lines.len().saturating_sub(1) as f32;
        let max_w = lines.iter().map(|l| l.size().x).fold(0.0f32, f32::max);
        let pad = 3.0;
        let size = egui::vec2(max_w + pad * 2.0, total_h + pad * 2.0);
        let rect = egui::Rect::from_center_size(ui.max_rect().center(), size);
        let resp = ui.interact(rect, ui.next_auto_id(), egui::Sense::click());
        if resp.clicked() {
            self.pending_label_detail = Some((from.id, to.id, label));
        }
    }

    fn has_wire_menu(&mut self, _from: &OutPinId, _to: &InPinId, _snarl: &Snarl<MassFlowNode>) -> bool { true }
    fn show_wire_menu(&mut self, from: &OutPin, to: &InPin, ui: &mut Ui, _snarl: &mut Snarl<MassFlowNode>) {
        // 获取鼠标在屏幕上的绝对位置，用于定位弹窗
        let pos = ui.ctx().input(|i| i.pointer.interact_pos().unwrap_or(ui.cursor().min));
        self.pending_wire_menu = Some((from.id, to.id, pos));
    }

    fn has_dropped_wire_menu(&mut self, _pins: AnyPins<'_>, _snarl: &mut Snarl<MassFlowNode>) -> bool { false }
    fn show_dropped_wire_menu(&mut self, _pos: Pos2, _ui: &mut Ui, _pins: AnyPins<'_>, _snarl: &mut Snarl<MassFlowNode>) {}
    fn disconnect(&mut self, _from: &OutPin, _to: &InPin, _snarl: &mut Snarl<MassFlowNode>) {}

    fn draw_foreground(
        &mut self,
        _viewport: &Rect,
        _snarl_style: &SnarlStyle,
        _egui_style: &egui::Style,
        painter: &egui::Painter,
        _snarl: &Snarl<MassFlowNode>,
        out_positions: &HashMap<OutPinId, Pos2>,
        in_positions: &HashMap<InPinId, Pos2>,
    ) {
        // 检测原始点击事件（未被 egui response 消耗）
        let clicks: Vec<Pos2> = painter.ctx().input(|i| {
            i.events.iter().filter_map(|e| {
                if let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } = e {
                    Some(*pos)
                } else { None }
            }).collect()
        });
        for (from_id, to_id, label) in &self.wire_labels {
            let from_pos = out_positions.get(from_id).copied().unwrap_or(Pos2::ZERO);
            let to_pos = in_positions.get(to_id).copied().unwrap_or(Pos2::ZERO);
            if from_pos == Pos2::ZERO || to_pos == Pos2::ZERO { continue; }
            let mid = Pos2::new((from_pos.x + to_pos.x) / 2.0, (from_pos.y + to_pos.y) / 2.0);
            let font_id = egui::FontId::proportional(14.0);
            let color = egui::Color32::from_rgb(25, 26, 27);
            let lines: Vec<std::sync::Arc<egui::Galley>> = label.split('\n').map(|line| {
                painter.layout_no_wrap(line.to_string(), font_id.clone(), color)
            }).collect();
            let total_height: f32 = lines.iter().map(|l| l.size().y).sum::<f32>() + lines.len().saturating_sub(1) as f32;
            let max_width = lines.iter().map(|l| l.size().x).fold(0.0f32, f32::max);
            let pad = 3.0;
            let box_size = egui::vec2(max_width + pad * 2.0, total_height + pad * 2.0);
            let bg_rect = egui::Rect::from_center_size(mid, box_size);
            painter.rect_filled(bg_rect, 2.0, egui::Color32::from_rgba_premultiplied(192, 192, 192, 230));
            // 检测原始点击（不被 snarl 消耗）
            for &click_pos in &clicks {
                if bg_rect.contains(click_pos) {
                    self.pending_label_detail = Some((*from_id, *to_id, label.clone()));
                }
            }
            let mut y = mid.y - total_height / 2.0;
            for galley in &lines {
                painter.galley(Pos2::new(mid.x - max_width / 2.0, y), galley.clone(), color);
                y += galley.size().y + 1.0;
            }
        }
    }
}
