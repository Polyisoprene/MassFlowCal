//! MassFlowCal — 物料平衡计算器
//!
//! 基于 egui/eframe 的桌面端应用，提供节点图编辑器用于化工物料平衡计算。
//! 程序入口：初始化日志、数据库、GUI 窗口。

mod config;
mod db;
mod engine;
mod error;
mod models;
mod ui;

use crate::config::Config;
use crate::db::Database;
use crate::ui::app::MassFlowApp;

/// 设置 egui 全局样式（深色主题，现代化按钮）
fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    // 按钮配色：深色背景 + 白色文字
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x3b, 0x3b, 0x4e);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x52, 0x52, 0x6e);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x2a, 0x2a, 0x3e);
    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(0x25, 0x25, 0x35);
    style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(0xd0, 0xd0, 0xe0);
    style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    style.visuals.widgets.inactive.expansion = 3.0;
    style.visuals.widgets.hovered.expansion = 3.0;
    style.visuals.widgets.active.expansion = 3.0;
    ctx.set_style(style);
}

/// 加载 Windows 系统中文字体（simhei.ttf 黑体）用于 UI 中文渲染
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // 优先 TTF 格式（egui 不支持 TTC 字体集合）
    let font_paths = [
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
    ];

    let mut loaded = false;
    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "chinese".to_owned(),
                egui::FontData::from_owned(data).into(),
            );
            loaded = true;
            break;
        }
    }

    if loaded {
        fonts.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "chinese".to_owned());
        fonts.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("chinese".to_owned());
    } else {
        // 调试：列出可用 TTF 字体
        if let Ok(entries) = std::fs::read_dir("C:\\Windows\\Fonts") {
            let ttfs: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    name.ends_with(".ttf") && (name.contains("hei") || name.contains("song") || name.contains("ming"))
                })
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            eprintln!("未找到中文字体，可用 TTF 候选: {:?}", ttfs);
        }
    }

    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result {
    // 初始化日志（RUST_LOG 环境变量控制级别，默认 info）
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("MassFlowCal 启动");

    let config = Config::from_env();

    // 打开 SQLite 数据库并初始化表结构 + 预置数据
    let db = Database::open(&config.db_path)
        .expect("无法打开数据库");

    let app = MassFlowApp::new(db);

    // 创建原生窗口
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("MassFlowCal — 物料平衡计算器"),
        ..Default::default()
    };

    eframe::run_native(
        "MassFlowCal",
        native_options,
        Box::new(|cc| {
            setup_style(&cc.egui_ctx);
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
