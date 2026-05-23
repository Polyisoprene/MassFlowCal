//! 主应用状态与 UI 面板
//!
//! MassFlowApp 负责 GUI 状态管理和 egui 渲染。
//! 业务逻辑（计算、持久化）委托给 mfc-engine 和 mfc-persistence。

use eframe::egui;
use egui_ariel::Snarl;
use std::collections::HashMap;
use uuid::Uuid;

use mfc_core::component::ComponentDef;
use mfc_core::graph::{Connection, FlowGraph, Node};
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;
use mfc_engine::Engine;
use mfc_persistence as persist;

use super::canvas::MassFlowViewer;
use super::convert;
use super::nodes::{MassFlowNode, MassFlowNodeKind};

pub struct NodeData {
    pub id: Uuid,
    pub node: Node,
    pub sn_node: MassFlowNode,
}

pub struct MassFlowApp {
    pub snarl: Snarl<MassFlowNode>,
    pub graph_nodes: HashMap<usize, NodeData>,
    pub connections: Vec<Connection>,
    pub wire_names: HashMap<(usize, usize, usize, usize), String>,
    next_node_num: usize,

    pub db: persist::SqliteDatabase,
    pub db_components: Vec<ComponentDef>,
    pub project_components: Vec<ComponentDef>,

    // right panel removed
    pub status_msg: String,
    pub compute_results: Option<HashMap<Uuid, Vec<(String, MaterialStream)>>>,

    show_db_mgr: bool,
    show_project_mgr: bool,
    show_results: bool,
    show_console: bool,
    log_messages: Vec<String>,
    label_detail: Option<(String, MaterialStream)>,
    reaction_editor: Option<(egui_ariel::NodeId, usize, super::nodes::ReactionData, Vec<(String, f64, bool)>)>,

    new_comp: ComponentDef,
    editing_components: std::collections::HashSet<String>,
    current_file: Option<std::path::PathBuf>,
    rename_state: Option<(egui_ariel::NodeId, String)>,
    wire_rename_state: Option<(egui_ariel::OutPinId, egui_ariel::InPinId, String)>,
    wire_menu_state: Option<(egui_ariel::OutPinId, egui_ariel::InPinId, egui::Pos2)>,
}

impl MassFlowApp {
    pub fn new(db: persist::SqliteDatabase) -> Self {
        let db_components = db.get_all_components().unwrap_or_default();
        Self {
            snarl: Snarl::new(),
            graph_nodes: HashMap::new(),
            connections: Vec::new(),
            wire_names: HashMap::new(),
            next_node_num: 1,
            db,
            db_components,
            project_components: Vec::new(),
            // right panel removed
            status_msg: String::new(),
            compute_results: None,
            show_db_mgr: false, show_project_mgr: false, show_results: false, show_console: false,
            log_messages: Vec::new(),
            new_comp: ComponentDef { cas_number: String::new(), name: String::new(), formula: String::new(), molecular_weight: 0.0 },
            editing_components: std::collections::HashSet::new(),
            current_file: None,
            rename_state: None,
            wire_rename_state: None,
            wire_menu_state: None,
            label_detail: None,
            reaction_editor: None,
        }
    }

    pub fn log(&mut self, msg: &str) {
        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
        let line = format!("[{}] {}", ts, msg);
        self.log_messages.push(line.clone());
        log::info!("{}", msg);
        if self.log_messages.len() > 500 { self.log_messages.remove(0); }
        // 同时写入日志文件
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("massflowcal.log") {
            use std::io::Write;
            let _ = writeln!(file, "{}", line);
        }
    }

    fn sync_params_from_snarl(&mut self) {
        for (nid, _pos, sn_node) in self.snarl.nodes_pos_ids() {
            if let Some(data) = self.graph_nodes.get_mut(&nid.0) {
                data.sn_node = sn_node.clone();
                let (params, custom_name, title) = convert::sync_params(&data.sn_node);
                data.node.params = params;
                data.node.custom_name = custom_name;
                data.node.title = title;
            }
        }
    }

    fn sync_from_snarl(&mut self) {
        self.sync_params_from_snarl();
        for (nid, pos, _node) in self.snarl.nodes_pos_ids() {
            if let Some(data) = self.graph_nodes.get_mut(&nid.0) {
                data.node.pos = [pos.x as f64, pos.y as f64];
            }
        }
        self.connections.clear();
        for (out_pin, in_pin) in self.snarl.wires() {
            let key = (out_pin.node.0, out_pin.output, in_pin.node.0, in_pin.input);
            let stream_name = self.wire_names.get(&key).cloned();
            self.connections.push(Connection {
                id: Uuid::new_v4(),
                origin_node: self.graph_nodes.get(&out_pin.node.0).map(|d| d.id).unwrap_or_default(),
                origin_slot: out_pin.output,
                target_node: self.graph_nodes.get(&in_pin.node.0).map(|d| d.id).unwrap_or_default(),
                target_slot: in_pin.input,
                stream_name,
            });
        }
    }

    fn build_graph(&self) -> FlowGraph {
        let nodes: Vec<Node> = self.graph_nodes.values().map(|d| d.node.clone()).collect();
        FlowGraph { id: Uuid::nil(), name: "current".into(), nodes, connections: self.connections.clone() }
    }

    pub fn compute(&mut self) {
        self.log("=== 开始计算 ===");
        self.sync_from_snarl();
        self.log(&format!("同步完成: {}个节点, {}条连线", self.graph_nodes.len(), self.connections.len()));
        for (_, data) in &self.graph_nodes {
            if let Some(mfc_core::graph::NodeParams::Feed(p)) = &data.node.params {
                if p.cas_number.is_empty() {
                    let msg = format!("错误: '{}' 未选择组分", data.node.title);
                    self.status_msg = msg.clone();
                    self.log(&msg);
                    return;
                }
            }
        }
        let graph = self.build_graph();
        self.log("构建计算图...");
        self.log(&format!("执行计算: {}个节点, {}个连线", graph.nodes.len(), graph.connections.len()));
        self.log("正在构建DAG与拓扑排序...");
        match Engine::compute(&graph, &self.db) {
            Ok(results) => {
                let node_count = results.len();
                // 计算总输入/总输出
                let mut total_in = 0.0;
                let mut total_out = 0.0;
                for node in &graph.nodes {
                    match node.node_type {
                        mfc_core::graph::NodeType::Feed => {
                            if let Some(outputs) = results.get(&node.id) {
                                for (_, s) in outputs { total_in += s.mass_flow; }
                            }
                        }
                        mfc_core::graph::NodeType::Product => {
                            for conn in &graph.connections {
                                if conn.target_node == node.id {
                                    if let Some(upstream) = results.get(&conn.origin_node) {
                                        if let Some((_, s)) = upstream.get(conn.origin_slot) {
                                            total_out += s.mass_flow;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                self.log(&format!("计算完成: {}个节点, 总输入 {:.2} kg/h, 总输出 {:.2} kg/h", node_count, total_in, total_out));
                self.compute_results = Some(results);
                self.show_results = true;
                self.status_msg = "计算完成".into();
            }
            Err(e) => { let msg = format!("计算失败: {}", e); self.status_msg = msg.clone(); self.log(&msg); }
        }
    }

    fn new_project(&mut self) {
        self.log("新建项目");
        let nids: Vec<egui_ariel::NodeId> = self.snarl.nodes_pos_ids().map(|(id, _, _)| id).collect();
        for nid in nids { self.snarl.remove_node(nid); }
        self.graph_nodes.clear();
        self.connections.clear();
        self.project_components.clear();
        self.wire_names.clear();
        self.next_node_num = 1;
        self.compute_results = None;
        self.current_file = None;
        self.status_msg = "新建项目".into();
    }

    fn save_project_to(&mut self, path: &std::path::Path) {
        self.sync_from_snarl();
        let graph = self.build_graph();
        let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        self.log(&format!("保存项目: {} ({}个节点)", fname, graph.nodes.len()));
        match persist::save_project(path, &graph, &self.project_components) {
            Ok(()) => {
                self.current_file = Some(path.to_path_buf());
                self.status_msg = format!("已保存: {}", fname);
            }
            Err(e) => { self.status_msg = format!("保存失败: {}", e); self.log(&format!("保存失败: {}", e)); }
        }
    }

    fn save_project(&mut self) {
        if let Some(ref path) = self.current_file.clone() { self.save_project_to(path); }
        else { self.save_project_as(); }
    }

    fn save_project_as(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("MassFlowCal 项目", &["mfc", "json"]).set_file_name("project.mfc").save_file();
        if let Some(path) = path { self.save_project_to(&path); }
    }

    fn open_project(&mut self) {
        let path = rfd::FileDialog::new().add_filter("MassFlowCal 项目", &["mfc", "json"]).pick_file();
        if let Some(path) = path {
            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            self.log(&format!("打开项目: {}", fname));
            match persist::load_project(&path) {
                Ok((graph, comps)) => {
                    self.log(&format!("加载成功: {}个节点, {}个组分", graph.nodes.len(), comps.len()));
                    self.new_project();
                    self.project_components = comps;
                    for node in &graph.nodes {
                        let sn_node = convert::graph_node_to_massflow(node);
                        let nid = self.snarl.insert_node(
                            egui::Pos2::new(node.pos[0] as f32, node.pos[1] as f32), sn_node.clone());
                        self.graph_nodes.insert(nid.0, NodeData { id: node.id, node: node.clone(), sn_node });
                    }
                    for conn in &graph.connections {
                        let origin = self.find_snarl_node(&conn.origin_node);
                        let target = self.find_snarl_node(&conn.target_node);
                        if origin.is_none() || target.is_none() { continue; }
                        let o = origin.unwrap(); let t = target.unwrap();
                        let _ = self.snarl.connect(
                            egui_ariel::OutPinId { node: egui_ariel::NodeId(o), output: conn.origin_slot },
                            egui_ariel::InPinId { node: egui_ariel::NodeId(t), input: conn.target_slot },
                        );
                        if let Some(ref name) = conn.stream_name {
                            self.wire_names.insert((o, conn.origin_slot, t, conn.target_slot), name.clone());
                        }
                    }
                    self.current_file = Some(path);
                    self.status_msg = "项目已打开".into();
                }
                Err(e) => { self.status_msg = format!("加载失败: {}", e); }
            }
        }
    }

    fn find_snarl_node(&self, uuid: &Uuid) -> Option<usize> {
        self.graph_nodes.iter().find(|(_, d)| d.id == *uuid).map(|(k, _)| *k)
    }

    fn export_results(&mut self) {
        let Some(ref results) = self.compute_results else { return };
        let path = rfd::FileDialog::new().add_filter("CSV文件", &["csv"]).set_file_name("results.csv").save_file();
        let Some(ref path) = path else { return };
        let names: Vec<(String, String)> = self.db_components.iter()
            .map(|c| (c.cas_number.clone(), c.name.clone())).collect();
        let component_names = |cas: &str| -> String {
            names.iter().find(|(c,_)| c == cas).map(|(_, n)| n.clone()).unwrap_or_else(|| cas.to_string())
        };
        // 按连线（流股）收集数据
        let mut streams: Vec<(String, &MaterialStream)> = Vec::new();
        for conn in &self.connections {
            if let Some(stream) = results.get(&conn.origin_node)
                .and_then(|outputs| outputs.get(conn.origin_slot))
                .map(|(_, s)| s)
            {
                let name = conn.stream_name.clone().unwrap_or_else(|| {
                    let from = self.graph_nodes.values().find(|d| d.id == conn.origin_node)
                        .map(|d| d.node.title.clone()).unwrap_or_default();
                    let to = self.graph_nodes.values().find(|d| d.id == conn.target_node)
                        .map(|d| d.node.title.clone()).unwrap_or_default();
                    format!("{} → {}", from, to)
                });
                streams.push((name, stream));
            }
        }
        match persist::export_csv(&streams, &component_names) {
            Ok(csv) => {
                if let Err(e) = std::fs::write(path, &csv) {
                    self.status_msg = format!("导出失败: {}", e);
                } else {
                    let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    self.log(&format!("导出结果: {}", fname));
                    self.status_msg = format!("已导出: {}", fname);
                }
            }
            Err(e) => { self.status_msg = format!("导出失败: {}", e); }
        }
    }

    fn reload_db_components(&mut self) {
        self.db_components = self.db.get_all_components().unwrap_or_default();
    }

    // ===== UI 窗口 =====

    fn component_mgr_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_db_mgr;
        egui::Window::new("组分数据库").open(&mut open).resizable(true).default_size([650.0, 450.0])
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
                            if let Err(e) = self.db.insert_component(&self.new_comp) {
                                self.status_msg = format!("添加失败: {}", e);
                            } else {
                                self.log(&format!("添加组分: {} ({})", self.new_comp.name, self.new_comp.cas_number));
                                self.new_comp = ComponentDef { cas_number: String::new(), name: String::new(), formula: String::new(), molecular_weight: 0.0 };
                                self.reload_db_components();
                            }
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut to_delete: Option<String> = None; let mut changed = false;
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
                                if ui.button("保存").clicked() { let _ = self.db.update_component(comp); self.editing_components.remove(&cas); changed = true; }
                            } else {
                                ui.label(&comp.cas_number); ui.label(&comp.name); ui.label(&comp.formula);
                                ui.label(format!("{:.3}", comp.molecular_weight));
                                if ui.button("编辑").clicked() { self.editing_components.insert(cas.clone()); }
                            }
                            if ui.button("删除").clicked() { to_delete = Some(cas.clone()); }
                            if ui.button("加入项目").clicked() && !self.project_components.iter().any(|c| c.cas_number == comp.cas_number) {
                                self.project_components.push(comp.clone());
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(ref cas) = to_delete {
                        let _ = self.db.delete_component(cas);
                        self.project_components.retain(|c| c.cas_number != *cas);
                        self.editing_components.remove(cas);
                        self.log(&format!("删除组分: {}", cas));
                        changed = true;
                    }
                    if changed { self.reload_db_components(); }
                });
            });
        self.show_db_mgr = open;
    }

    fn results_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_results;
        egui::Window::new("计算结果").open(&mut open).resizable(true).default_size([850.0, 550.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| { if ui.button("导出结果").clicked() { self.export_results(); } });
                if let Some(ref results) = self.compute_results {
                    egui::ScrollArea::both().show(ui, |ui| {
                        // 按连线（流股）报告，每条连线对应一个流股
                        for (gid, conn) in self.connections.iter().enumerate() {
                            // 从计算结果中查找源节点的输出端口
                            let stream = results.get(&conn.origin_node)
                                .and_then(|outputs| outputs.get(conn.origin_slot))
                                .map(|(_, s)| s);
                            let Some(stream) = stream else { continue; };

                            let stream_name = conn.stream_name.clone().unwrap_or_else(|| {
                                let from = self.graph_nodes.values().find(|d| d.id == conn.origin_node)
                                    .map(|d| d.node.title.clone()).unwrap_or_default();
                                let to = self.graph_nodes.values().find(|d| d.id == conn.target_node)
                                    .map(|d| d.node.title.clone()).unwrap_or_default();
                                format!("{} → {}", from, to)
                            });

                            egui::CollapsingHeader::new(format!("{}##col{}", stream_name, gid))
                                .default_open(true)
                                .show(ui, |ui| {
                                ui.label(format!("总质量: {:.3} kg/h  |  总摩尔: {:.4} kmol/h  |  组分数: {}",
                                    stream.mass_flow, stream.mole_flow, stream.components.len()));
                                ui.separator();
                                egui::Grid::new(format!("st_{}", gid)).striped(true).min_col_width(90.0).show(ui, |ui| {
                                    ui.label("组分"); ui.label("质量流量(kg/h)"); ui.label("摩尔流量(kmol/h)");
                                    ui.label("质量分率"); ui.label("摩尔分率");
                                    ui.end_row();
                                    for (cas, cf) in &stream.components {
                                        let name = self.db_components.iter().find(|c| c.cas_number == *cas)
                                            .map(|c| c.name.as_str()).unwrap_or(cas.as_str());
                                        ui.label(name);
                                        ui.label(format!("{:.4}", cf.mass_flow));
                                        ui.label(format!("{:.4}", cf.mole_flow));
                                        ui.label(format!("{:.1}%", cf.mass_fraction * 100.0));
                                        ui.label(format!("{:.1}%", cf.mole_fraction * 100.0));
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

    fn rename_dialog(&mut self, ctx: &egui::Context) {
        if self.rename_state.is_some() {
            let (nid, current_name) = self.rename_state.take().unwrap();
            let mut edit_buf = current_name;
            let mut open = true; let mut done = false;
            egui::Window::new("重命名节点").collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open)
                .show(ctx, |ui| {
                    ui.label("输入新名称（留空恢复自动生成）:");
                    let resp = ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut edit_buf));
                    ui.horizontal(|ui| {
                        if ui.button("确认").clicked() || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                            let custom_name = if edit_buf.trim().is_empty() { None } else { Some(edit_buf.trim().to_string()) };
                            let new_label = custom_name.as_deref().unwrap_or("(自动生成)");
                            self.log(&format!("重命名节点: {} → {}", edit_buf.trim(), new_label));
                            if let Some(sn_node) = self.snarl.get_node_mut(nid) { sn_node.custom_name = custom_name.clone(); }
                            if let Some(data) = self.graph_nodes.get_mut(&nid.0) {
                                data.sn_node.custom_name = custom_name.clone();
                                data.node.custom_name = custom_name;
                            }
                            done = true;
                        }
                        if ui.button("取消").clicked() { done = true; }
                    });
                });
            if !open { done = true; }
            if !done { self.rename_state = Some((nid, edit_buf)); }
        }
    }
    fn wire_rename_dialog(&mut self, ctx: &egui::Context) {
        if self.wire_rename_state.is_some() {
            let (from, to, current_name) = self.wire_rename_state.take().unwrap();
            let default_name = if current_name.is_empty() {
                format!("流股{}->{}", from.output, to.input)
            } else {
                current_name
            };
            let mut edit_buf = default_name;
            let mut open = true; let mut done = false;
            egui::Window::new("重命名流股").collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open)
                .show(ctx, |ui| {
                    ui.label("输入流股名称:");
                    let resp = ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut edit_buf));
                    ui.horizontal(|ui| {
                        let w = ui.available_width();
                        ui.allocate_ui_with_layout(
                            egui::vec2(w * 0.5, 20.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("确认").clicked() || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                    let name = edit_buf.trim().to_string();
                                    if !name.is_empty() {
                                        let key = (from.node.0, from.output, to.node.0, to.input);
                                        self.wire_names.insert(key, name.clone());
                                        for conn in &mut self.connections {
                                            let o = self.graph_nodes.iter().find(|(_, d)| d.id == conn.origin_node).map(|(k, _)| *k).unwrap_or(0);
                                            let t = self.graph_nodes.iter().find(|(_, d)| d.id == conn.target_node).map(|(k, _)| *k).unwrap_or(0);
                                            if o == key.0 && conn.origin_slot == key.1 && t == key.2 && conn.target_slot == key.3 {
                                                conn.stream_name = Some(name.clone());
                                                break;
                                            }
                                        }
                                        self.log(&format!("重命名流股: {}", name));
                                    }
                                    done = true;
                                }
                            });
                        ui.allocate_ui_with_layout(
                            egui::vec2(w * 0.5, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                if ui.button("取消").clicked() { done = true; }
                            });
                    });
                });
            if !open { done = true; }
            if !done { self.wire_rename_state = Some((from, to, edit_buf)); }
        }
    }


    fn wire_menu_popup(&mut self, ctx: &egui::Context) {
        if let Some((from, to, pos)) = self.wire_menu_state {
            let win_size = egui::vec2(40.0, 50.0);
            egui::Window::new("##wire_menu")
                .collapsible(false).resizable(false).title_bar(false)
                .fixed_pos(pos).fixed_size(win_size)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        if ui.button("改名").clicked() {
                            self.wire_rename_state = Some((from, to, String::new()));
                            self.wire_menu_state = None;
                        }
                        if ui.button("删除").clicked() {
                            self.snarl.disconnect(from, to);
                            let key = (from.node.0, from.output, to.node.0, to.input);
                            self.wire_names.remove(&key);
                            self.log("删除连线");
                            self.wire_menu_state = None;
                        }
                    });
                });
            if ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary)) {
                let mouse = ctx.input(|i| i.pointer.interact_pos());
                if let Some(m) = mouse {
                    if !egui::Rect::from_min_size(pos, win_size).contains(m) {
                        self.wire_menu_state = None;
                    }
                }
            }
        }
    }

    fn project_mgr_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.show_project_mgr;
        egui::Window::new("项目组分").open(&mut open).resizable(true).default_size([400.0, 300.0])
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

    fn reaction_editor_popup(&mut self, ctx: &egui::Context) {
        if self.reaction_editor.is_some() {
            let (nid, idx, mut rx_data, mut entries) = self.reaction_editor.take().unwrap();
            let mut open = true;

            // 首次打开时初始化 entries
            if entries.is_empty() {
                for (cas, coeff) in &rx_data.stoichiometry {
                    entries.push((cas.clone(), coeff.abs(), !coeff.is_sign_negative()));
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
                if !entries.iter().any(|e| !e.2) { entries.push((String::new(), 0.0, false)); }
                if !entries.iter().any(|e| e.2) { entries.push((String::new(), 0.0, true)); }
            }

            let comp_names: Vec<&str> = self.project_components.iter().map(|c| c.name.as_str()).collect();

            egui::Window::new("编辑反应")
                .open(&mut open).resizable(true).default_size([500.0, 380.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // 反应物
                        ui.vertical(|ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.label(egui::RichText::new("反应物").strong());
                                let mut to_rm: Option<usize> = None;
                                for i in 0..entries.len() {
                                    if entries[i].2 { continue; }
                                    ui.horizontal(|ui| {
                                        let mut sel = self.project_components.iter().position(|c| c.cas_number == entries[i].0);
                                        let preview = sel.map_or("选择...".into(), |j| comp_names[j].to_string());
                                        egui::ComboBox::from_id_salt(("rx_ent", i)).width(80.0).selected_text(preview)
                                            .show_ui(ui, |ui| {
                                                for (j, c) in self.project_components.iter().enumerate() {
                                                    if ui.selectable_value(&mut sel, Some(j), comp_names[j]).clicked() {
                                                        entries[i].0 = c.cas_number.clone();
                                                    }
                                                }
                                            });
                                        ui.add_sized([45.0, 18.0], egui::DragValue::new(&mut entries[i].1).speed(0.1));
                                        if ui.small_button("X").clicked() { to_rm = Some(i); }
                                    });
                                }
                                if let Some(i) = to_rm { entries.remove(i); }
                                if ui.small_button("+ 添加反应物").clicked() {
                                    entries.push((String::new(), 0.0, false));
                                }
                            });
                        });
                        ui.separator();
                        // 产物
                        ui.vertical(|ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.label(egui::RichText::new("产物").strong());
                                let mut to_rm: Option<usize> = None;
                                for i in 0..entries.len() {
                                    if !entries[i].2 { continue; }
                                    ui.horizontal(|ui| {
                                        let mut sel = self.project_components.iter().position(|c| c.cas_number == entries[i].0);
                                        let preview = sel.map_or("选择...".into(), |j| comp_names[j].to_string());
                                        egui::ComboBox::from_id_salt(("rx_ent", i)).width(80.0).selected_text(preview)
                                            .show_ui(ui, |ui| {
                                                for (j, c) in self.project_components.iter().enumerate() {
                                                    if ui.selectable_value(&mut sel, Some(j), comp_names[j]).clicked() {
                                                        entries[i].0 = c.cas_number.clone();
                                                    }
                                                }
                                            });
                                        ui.add_sized([45.0, 18.0], egui::DragValue::new(&mut entries[i].1).speed(0.1));
                                        if ui.small_button("X").clicked() { to_rm = Some(i); }
                                    });
                                }
                                if let Some(i) = to_rm { entries.remove(i); }
                                if ui.small_button("+ 添加产物").clicked() {
                                    entries.push((String::new(), 0.0, true));
                                }
                            });
                        });
                    });
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        ui.label("转化率");
                        ui.add_sized([60.0, 18.0], egui::DragValue::new(&mut rx_data.conversion).speed(0.01).range(0.0..=1.0));
                        ui.label("  关键组分");
                        let mut kc_sel = self.project_components.iter().position(|c| c.cas_number == rx_data.key_component);
                        let kc_prev = kc_sel.map_or("选择".into(), |j| comp_names[j].to_string());
                        egui::ComboBox::from_id_salt("rx_kc").width(80.0).selected_text(&kc_prev)
                            .show_ui(ui, |ui| {
                                for (j, _c) in self.project_components.iter().enumerate() {
                                    if ui.selectable_value(&mut kc_sel, Some(j), comp_names[j]).clicked() {
                                        rx_data.key_component = self.project_components[j].cas_number.clone();
                                    }
                                }
                            });
                    });
                });
            // 关闭时自动保存
            if !open {
                let mut stoich = HashMap::new();
                for e in &entries {
                    if !e.0.is_empty() {
                        let sign = if e.2 { 1.0 } else { -1.0 };
                        stoich.insert(e.0.clone(), sign * e.1);
                    }
                }
                rx_data.stoichiometry = stoich;
                if let Some(node_data) = self.snarl.get_node_mut(nid) {
                    if let MassFlowNode { kind: MassFlowNodeKind::Reactor(ref mut rx), .. } = node_data {
                        if idx < rx.reactions.len() { rx.reactions[idx] = rx_data.clone(); }
                    }
                }
                if let Some(node_data) = self.graph_nodes.get_mut(&nid.0) {
                    if let MassFlowNode { kind: MassFlowNodeKind::Reactor(ref mut rx), .. } = &mut node_data.sn_node {
                        if idx < rx.reactions.len() { rx.reactions[idx] = rx_data; }
                    }
                }
                self.log(&format!("更新反应 #{}", idx + 1));
                self.reaction_editor = None;
            }
            else if self.reaction_editor.is_none() { self.reaction_editor = Some((nid, idx, rx_data, entries)); }
        }
    }

    fn label_detail_popup(&mut self, ctx: &egui::Context) {
        if self.label_detail.is_some() {
            let (name, stream) = self.label_detail.take().unwrap();
            let mut open = true;
            egui::Window::new("流股详情").open(&mut open).resizable(true).default_size([550.0, 400.0])
                .show(ctx, |ui| {
                    ui.label(format!("流股: {}", name));
                    ui.label(format!("总质量: {:.3} kg/h  |  总摩尔: {:.4} kmol/h  |  组分数: {}",
                        stream.mass_flow, stream.mole_flow, stream.components.len()));
                    ui.separator();
                    egui::ScrollArea::both().show(ui, |ui| {
                        egui::Grid::new("detail_grid").striped(true).min_col_width(90.0).show(ui, |ui| {
                            ui.label("组分"); ui.label("质量流量(kg/h)"); ui.label("摩尔流量(kmol/h)");
                            ui.label("质量分率"); ui.label("摩尔分率");
                            ui.end_row();
                            for (cas, cf) in &stream.components {
                                let cname = self.db_components.iter().find(|c| c.cas_number == *cas)
                                    .map(|c| c.name.as_str()).unwrap_or(cas.as_str());
                                ui.label(cname);
                                ui.label(format!("{:.4}", cf.mass_flow));
                                ui.label(format!("{:.4}", cf.mole_flow));
                                ui.label(format!("{:.1}%", cf.mass_fraction * 100.0));
                                ui.label(format!("{:.1}%", cf.mole_fraction * 100.0));
                                ui.end_row();
                            }
                        });
                    });
                });
            if !open { self.label_detail = None; } else { self.label_detail = Some((name, stream)); }
        }
    }
}

impl eframe::App for MassFlowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) { self.save_project(); }

        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::vec2(8.0, 6.0)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("新建项目").clicked() { self.new_project(); ui.close(); }
                    if ui.button("打开项目...").clicked() { self.open_project(); ui.close(); }
                    ui.separator();
                    if ui.button("保存 (Ctrl+S)").clicked() { self.save_project(); ui.close(); }
                    if ui.button("另存为...").clicked() { self.save_project_as(); ui.close(); }
                });
                ui.separator();
                if ui.button("组分数据库").clicked() { self.show_db_mgr = !self.show_db_mgr; }
                ui.menu_button("视图", |ui| {
                    if ui.button("控制台").clicked() {
                        self.show_console = !self.show_console; ui.close();
                    }
                    if ui.button("项目组分").clicked() {
                        self.show_project_mgr = !self.show_project_mgr; ui.close();
                    }
                });
                ui.label(&self.status_msg);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("计算").clicked() { self.compute(); }
                });
            });
        });

        if self.show_db_mgr { self.component_mgr_ui(ctx); }

        if self.show_project_mgr { self.project_mgr_ui(ctx); }

        // 底部控制台
        if self.show_console {
            egui::TopBottomPanel::bottom("console")
                .min_height(100.0).resizable(true)
                .frame(egui::Frame {
                    inner_margin: egui::vec2(4.0, 2.0).into(),
                    fill: egui::Color32::from_rgb(20, 20, 24),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false]).stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.set_min_height(ui.available_height());
                            for msg in &self.log_messages {
                                ui.label(egui::RichText::new(msg).size(12.0).color(egui::Color32::from_rgb(180, 200, 180)));
                            }
                        });
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            // 先同步连线数据，再构建标签
            self.sync_from_snarl();
            let mut viewer = MassFlowViewer::new(&self.project_components);
            let results = &self.compute_results;
            viewer.wire_labels = self.connections.iter().filter_map(|conn| {
                let origin_key = self.graph_nodes.iter().find(|(_, d)| d.id == conn.origin_node).map(|(k, _)| *k)?;
                let target_key = self.graph_nodes.iter().find(|(_, d)| d.id == conn.target_node).map(|(k, _)| *k)?;
                let from_id = egui_ariel::OutPinId { node: egui_ariel::NodeId(origin_key), output: conn.origin_slot };
                let to_id = egui_ariel::InPinId { node: egui_ariel::NodeId(target_key), input: conn.target_slot };
                let mut label = conn.stream_name.clone().unwrap_or_else(|| {
                    let from = self.graph_nodes.values().find(|d| d.id == conn.origin_node)
                        .map(|d| d.node.title.clone()).unwrap_or_default();
                    let to = self.graph_nodes.values().find(|d| d.id == conn.target_node)
                        .map(|d| d.node.title.clone()).unwrap_or_default();
                    format!("{} → {}", from, to)
                });
                if let Some(ref res) = results {
                    if let Some(stream) = res.get(&conn.origin_node)
                        .and_then(|outputs| outputs.get(conn.origin_slot))
                        .map(|(_, s)| s)
                    {
                        label.push_str(&format!("\n{:.1} kg/h\n{:.2} kmol/h", stream.mass_flow, stream.mole_flow));
                    }
                }
                Some((from_id, to_id, label))
            }).collect();
            self.snarl.show(&mut viewer, &egui_ariel::ui::SnarlStyle::default(), (), ui);

            let mut logs: Vec<String> = Vec::new();

            if let Some((node, pos)) = viewer.pending_node.take() {
                let id = Uuid::new_v4();
                let nn = self.next_node_num; self.next_node_num += 1;
                let title = format!("{} #{}", node.title(), nn);
                logs.push(format!("添加节点: {}", title));
                let graph_node = convert::to_graph_node(&node, id, title);
                let nid = self.snarl.insert_node(pos, node.clone());
                self.graph_nodes.insert(nid.0, NodeData { id, node: graph_node, sn_node: node });
            }
            if let Some(nid) = viewer.pending_remove.take() {
                let label = self.graph_nodes.get(&nid.0).map(|d| d.node.title.clone()).unwrap_or_default();
                logs.push(format!("删除节点: {}", label));
                self.snarl.remove_node(nid);
                self.graph_nodes.remove(&nid.0);
                // 清理与该节点相关的流股名称
                self.wire_names.retain(|&(from_nid, _, to_nid, _), _| from_nid != nid.0 && to_nid != nid.0);
            }
            if let Some((nid, current)) = viewer.pending_rename.take() {
                self.rename_state = Some((nid, current));
            }
            if let Some((from, to, current)) = viewer.pending_wire_rename.take() {
                self.wire_rename_state = Some((from, to, current));
            }
            if let Some((from, to, pos)) = viewer.pending_wire_menu.take() {
                self.wire_menu_state = Some((from, to, pos));
            }
            if let Some((nid, idx, rx_data)) = viewer.pending_edit_reaction.take() {
                self.reaction_editor = Some((nid, idx, rx_data, Vec::new()));
            }
            if let Some((from_id, _to_id, _label)) = viewer.pending_label_detail.take() {
                if let Some(ref results) = self.compute_results {
                    let node_uuid = self.graph_nodes.values().find(|d| {
                        let o = self.graph_nodes.iter().find(|(k, _)| **k == from_id.node.0).map(|(_, v)| v.id);
                        o == Some(d.id)
                    }).map(|d| d.id).unwrap_or_default();
                    if let Some(stream) = results.get(&node_uuid)
                        .and_then(|outputs| outputs.get(from_id.output))
                        .map(|(_, s)| s.clone())
                    {
                        let name = self.connections.iter().find(|c| {
                            let o = self.graph_nodes.iter().find(|(k, _)| **k == from_id.node.0).map(|(_, v)| v.id);
                            o == Some(c.origin_node) && c.origin_slot == from_id.output
                        }).and_then(|c| c.stream_name.clone()).unwrap_or_else(|| "未命名流股".to_string());
                        self.label_detail = Some((name, stream));
                    }
                }
            }

            for msg in logs { self.log(&msg); }
        });

        self.rename_dialog(ctx);
        self.wire_rename_dialog(ctx);
        self.wire_menu_popup(ctx);
        self.label_detail_popup(ctx);
        self.reaction_editor_popup(ctx);

        if self.show_results { self.results_window(ctx); }
    }
}
