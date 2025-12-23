//! 图表组件

use eframe::egui::{Color32, Ui};
use egui_plot::{Line, Plot, PlotPoints};

use crate::utils::CpuHistory;

/// 绘制 CPU 使用率折线图
pub fn draw_cpu_line_chart(ui: &mut Ui, history: &CpuHistory, title: &str) {
    let data = history.plot_data();
    if data.is_empty() {
        ui.label("等待数据...");
        return;
    }

    let line = Line::new(PlotPoints::new(data))
        .color(Color32::from_rgb(100, 150, 255))
        .width(2.0)
        .name(title);

    Plot::new(title)
        .height(150.0)
        .include_y(0.0)
        .include_y(100.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show_axes([true, true])
        .show(ui, |plot_ui| {
            plot_ui.line(line);
        });
}

/// 绘制多核心使用率对比图
pub fn draw_multi_core_chart(ui: &mut Ui, history: &CpuHistory, core_ids: &[usize]) {
    let colors = [
        Color32::from_rgb(255, 100, 100),
        Color32::from_rgb(100, 255, 100),
        Color32::from_rgb(100, 100, 255),
        Color32::from_rgb(255, 255, 100),
        Color32::from_rgb(255, 100, 255),
        Color32::from_rgb(100, 255, 255),
    ];

    Plot::new("multi_core_chart")
        .height(200.0)
        .include_y(0.0)
        .include_y(100.0)
        .allow_drag(false)
        .allow_zoom(false)
        .legend(egui_plot::Legend::default())
        .show(ui, |plot_ui| {
            for (i, &core_id) in core_ids.iter().enumerate() {
                let data = history.core_plot_data(core_id);
                if !data.is_empty() {
                    let color = colors[i % colors.len()];
                    let line = Line::new(PlotPoints::new(data))
                        .color(color)
                        .width(1.5)
                        .name(format!("CPU {}", core_id));
                    plot_ui.line(line);
                }
            }
        });
}
