//! hexin - 通用 CPU 核心调度可视化软件
//!
//! 支持 AMD/Intel CPU 的核心拓扑检测、进程管理和调度策略配置

mod app;
mod system;
mod ui;
mod utils;

use app::{AppConfig, HexinApp};
use eframe::egui;

fn main() -> eframe::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    let config = AppConfig::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.window_width, config.window_height])
            .with_min_inner_size([800.0, 600.0])
            .with_title("hexin - CPU 核心调度器"),
        ..Default::default()
    };

    eframe::run_native(
        "hexin",
        options,
        Box::new(|cc| Ok(Box::new(HexinApp::new(cc)))),
    )
}
