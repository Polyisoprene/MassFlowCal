#![windows_subsystem = "windows"]
//! MassFlowCal Desktop Application
//!
//! 入口点：初始化日志、数据库、GUI 窗口，启动 egui 事件循环。

mod app;
mod canvas;
mod convert;
mod nodes;

use app::MassFlowApp;
use mfc_persistence::SqliteDatabase;

fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgba_premultiplied(0, 0, 0, 0);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x20, 0x20, 0x20);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x30, 0x30, 0x30);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x40, 0x40, 0x40);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(0x06, 0xb6, 0xd4);
    style.visuals.dark_mode = true;
    ctx.set_style(style);
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let font_paths = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
    ];
    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                path.to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, path.to_string());
        }
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "data/massflowcal.db".to_string());
    let db = SqliteDatabase::open(&db_path).expect("无法打开数据库");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("MassFlowCal — 物料平衡计算器"),
        ..Default::default()
    };

    eframe::run_native(
        "MassFlowCal",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            setup_style(&cc.egui_ctx);
            Ok(Box::new(MassFlowApp::new(db)))
        }),
    )
}
