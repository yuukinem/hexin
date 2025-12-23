//! CPU 监控面板

use eframe::egui::{self, Color32, RichText, Ui, Vec2};
use egui_plot::{Line, Plot, PlotPoints};

use crate::system::{CoreType, CpuInfo};
use crate::utils::CpuHistory;

/// CPU 监控面板
pub struct CpuMonitorPanel {
    /// 选中的核心（用于显示详情）
    selected_core: Option<usize>,
}

impl CpuMonitorPanel {
    pub fn new() -> Self {
        Self {
            selected_core: None,
        }
    }

    /// 绘制面板
    pub fn ui(&mut self, ui: &mut Ui, cpu_info: &CpuInfo, history: &CpuHistory) {
        ui.horizontal(|ui| {
            // 左侧：核心网格
            ui.vertical(|ui| {
                ui.heading("CPU 核心使用率");
                ui.add_space(8.0);
                self.draw_core_grid(ui, cpu_info);
            });

            ui.add_space(20.0);

            // 右侧：总体信息
            ui.vertical(|ui| {
                self.draw_cpu_summary(ui, cpu_info);
                ui.add_space(16.0);
                self.draw_cache_info(ui, cpu_info);
            });
        });

        ui.add_space(16.0);

        // 历史曲线图
        self.draw_history_chart(ui, history);
    }

    /// 绘制核心网格
    fn draw_core_grid(&mut self, ui: &mut Ui, cpu_info: &CpuInfo) {
        let columns = cpu_info.grid_columns();
        let core_size = Vec2::new(48.0, 48.0);
        let spacing = 4.0;

        // 按 L3 缓存分组绘制
        let cores_by_l3 = cpu_info.cores_by_l3();
        let vcache_cores = cpu_info.vcache_cores();

        if cores_by_l3.is_empty() {
            // 没有 L3 分组信息，直接绘制所有核心
            egui::Grid::new("cpu_grid")
                .num_columns(columns)
                .spacing([spacing, spacing])
                .show(ui, |ui| {
                    for (i, core) in cpu_info.cores.iter().enumerate() {
                        self.draw_core_cell(ui, core.cpu_id, core.usage_percent, core.frequency_mhz,
                            core.core_type, false, core_size);
                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        } else {
            // 按 L3 缓存分组绘制
            let mut l3_ids: Vec<_> = cores_by_l3.keys().copied().collect();
            l3_ids.sort();

            for l3_id in l3_ids {
                if let (Some(cores), Some(cache_info)) = (
                    cores_by_l3.get(&l3_id),
                    cpu_info.l3_caches.iter().find(|c| c.id == l3_id),
                ) {
                    let is_vcache = cache_info.is_vcache;
                    let label = if is_vcache {
                        format!("CCD {} (3D V-Cache: {} MB)", l3_id, cache_info.size_kb / 1024)
                    } else {
                        format!("CCD {} (L3: {} MB)", l3_id, cache_info.size_kb / 1024)
                    };

                    ui.label(RichText::new(label).strong().color(
                        if is_vcache { Color32::from_rgb(100, 200, 100) } else { Color32::GRAY }
                    ));

                    egui::Grid::new(format!("cpu_grid_{}", l3_id))
                        .num_columns(columns.min(cores.len()))
                        .spacing([spacing, spacing])
                        .show(ui, |ui| {
                            for (i, core) in cores.iter().enumerate() {
                                self.draw_core_cell(
                                    ui, core.cpu_id, core.usage_percent, core.frequency_mhz,
                                    core.core_type, is_vcache, core_size,
                                );
                                if (i + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(8.0);
                }
            }
        }
    }

    /// 绘制单个核心单元格
    fn draw_core_cell(
        &mut self,
        ui: &mut Ui,
        cpu_id: usize,
        usage: f32,
        freq_mhz: u64,
        core_type: CoreType,
        is_vcache: bool,
        size: Vec2,
    ) {
        let usage_color = usage_to_color(usage);
        let border_color = if is_vcache {
            Color32::from_rgb(100, 200, 100)
        } else {
            match core_type {
                CoreType::Performance => Color32::from_rgb(100, 150, 255),
                CoreType::Efficiency => Color32::from_rgb(255, 180, 100),
                CoreType::Unknown => Color32::GRAY,
            }
        };

        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // 背景
            painter.rect_filled(rect, 4.0, usage_color);

            // 边框
            painter.rect_stroke(rect, 4.0, egui::Stroke::new(2.0, border_color));

            // 核心编号
            painter.text(
                rect.center_top() + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_TOP,
                format!("{:02}", cpu_id),
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );

            // 使用率
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}%", usage),
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            // 频率
            let freq_ghz = freq_mhz as f64 / 1000.0;
            painter.text(
                rect.center_bottom() - egui::vec2(0.0, 6.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{:.1}G", freq_ghz),
                egui::FontId::proportional(9.0),
                Color32::from_gray(200),
            );
        }

        if response.clicked() {
            self.selected_core = Some(cpu_id);
        }

        response.on_hover_text(format!(
            "CPU {}\n使用率: {:.1}%\n频率: {} MHz\n类型: {:?}",
            cpu_id, usage, freq_mhz, core_type
        ));
    }

    /// 绘制 CPU 总体信息
    fn draw_cpu_summary(&self, ui: &mut Ui, cpu_info: &CpuInfo) {
        ui.heading("CPU 信息");
        ui.add_space(8.0);

        egui::Grid::new("cpu_summary")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label("型号:");
                ui.label(&cpu_info.model_name);
                ui.end_row();

                ui.label("厂商:");
                ui.label(format!("{:?}", cpu_info.vendor));
                ui.end_row();

                ui.label("核心:");
                ui.label(format!(
                    "{} 物理 / {} 逻辑",
                    cpu_info.physical_cores, cpu_info.logical_cores
                ));
                ui.end_row();

                ui.label("SMT:");
                ui.label(if cpu_info.smt_enabled { "启用" } else { "禁用" });
                ui.end_row();

                ui.label("总使用率:");
                let usage_text = format!("{:.1}%", cpu_info.total_usage_percent);
                ui.label(RichText::new(usage_text).color(usage_to_color(cpu_info.total_usage_percent)));
                ui.end_row();

                if cpu_info.max_frequency_mhz > 0 {
                    ui.label("频率范围:");
                    ui.label(format!(
                        "{:.1} - {:.1} GHz",
                        cpu_info.base_frequency_mhz as f64 / 1000.0,
                        cpu_info.max_frequency_mhz as f64 / 1000.0
                    ));
                    ui.end_row();
                }
            });
    }

    /// 绘制缓存信息
    fn draw_cache_info(&self, ui: &mut Ui, cpu_info: &CpuInfo) {
        if cpu_info.l3_caches.is_empty() {
            return;
        }

        ui.heading("L3 缓存");
        ui.add_space(8.0);

        for cache in &cpu_info.l3_caches {
            let label = if cache.is_vcache {
                format!("CCD {}: {} MB (3D V-Cache)", cache.id, cache.size_kb / 1024)
            } else {
                format!("CCD {}: {} MB", cache.id, cache.size_kb / 1024)
            };

            let color = if cache.is_vcache {
                Color32::from_rgb(100, 200, 100)
            } else {
                Color32::GRAY
            };

            ui.label(RichText::new(label).color(color));
        }
    }

    /// 绘制历史曲线图
    fn draw_history_chart(&self, ui: &mut Ui, history: &CpuHistory) {
        ui.heading("使用率历史");
        ui.add_space(8.0);

        let plot_data = history.plot_data();
        if plot_data.is_empty() {
            ui.label("收集数据中...");
            return;
        }

        let line = Line::new(PlotPoints::new(plot_data))
            .color(Color32::from_rgb(100, 150, 255))
            .width(2.0);

        Plot::new("cpu_history_plot")
            .height(120.0)
            .include_y(0.0)
            .include_y(100.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_axes([false, true])
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });
    }
}

impl Default for CpuMonitorPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// 使用率转颜色
fn usage_to_color(usage: f32) -> Color32 {
    if usage < 30.0 {
        Color32::from_rgb(50, 150, 50) // 绿色
    } else if usage < 60.0 {
        Color32::from_rgb(200, 200, 50) // 黄色
    } else if usage < 85.0 {
        Color32::from_rgb(255, 150, 50) // 橙色
    } else {
        Color32::from_rgb(255, 80, 80) // 红色
    }
}
