//! 进程列表面板

use eframe::egui::{self, Color32, Frame, Margin, RichText, Rounding, ScrollArea, Stroke, TextEdit, Ui};

use crate::system::{
    format_memory, set_process_affinity, ProcessInfo, ProcessManager, SortField,
};

/// 进程列表面板
pub struct ProcessListPanel {
    /// 选中的进程 PID
    selected_pid: Option<u32>,
    /// 亲和性编辑模式
    editing_affinity: Option<u32>,
    /// 亲和性选择状态
    affinity_selection: Vec<bool>,
    /// 错误消息
    error_message: Option<String>,
}

impl ProcessListPanel {
    pub fn new() -> Self {
        Self {
            selected_pid: None,
            editing_affinity: None,
            affinity_selection: Vec::new(),
            error_message: None,
        }
    }

    /// 绘制面板
    pub fn ui(&mut self, ui: &mut Ui, process_manager: &mut ProcessManager, logical_cores: usize) {
        ui.add_space(8.0);

        // 错误消息显示
        let mut clear_error = false;
        if let Some(ref msg) = self.error_message {
            Frame::none()
                .fill(Color32::from_rgb(80, 30, 30))
                .inner_margin(Margin::same(8.0))
                .rounding(Rounding::same(4.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠").color(Color32::from_rgb(255, 100, 100)));
                        ui.label(RichText::new(msg.as_str()).color(Color32::from_rgb(255, 150, 150)));
                        if ui.small_button("✕").clicked() {
                            clear_error = true;
                        }
                    });
                });
            ui.add_space(8.0);
        }
        if clear_error {
            self.error_message = None;
        }

        // 搜索框
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(12.0))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍").size(16.0));
                    ui.add_space(8.0);
                    let mut filter = process_manager.filter().to_string();
                    let response = ui.add(
                        TextEdit::singleline(&mut filter)
                            .desired_width(300.0)
                            .hint_text("搜索进程名称、命令或 PID...")
                    );
                    if response.changed() {
                        process_manager.set_filter(filter);
                    }

                    ui.add_space(20.0);
                    ui.label(RichText::new(format!("共 {} 个进程", process_manager.filtered_processes().len()))
                        .color(Color32::from_gray(160)));
                });
            });

        ui.add_space(12.0);

        // 进程表格
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(12.0))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                // 表头
                self.draw_table_header(ui, process_manager);

                ui.add_space(4.0);

                // 分隔线
                ui.add(egui::Separator::default().spacing(0.0));

                // 进程列表
                ScrollArea::vertical()
                    .max_height(350.0)
                    .show(ui, |ui| {
                        let processes = process_manager.filtered_processes();

                        for (idx, process) in processes.iter().take(100).enumerate() {
                            self.draw_process_row(ui, process, logical_cores, idx);
                        }
                    });
            });

        // 选中进程的详情
        if let Some(pid) = self.selected_pid {
            if let Some(process) = process_manager
                .filtered_processes()
                .iter()
                .find(|p| p.pid == pid)
            {
                ui.add_space(12.0);
                self.draw_process_details(ui, process);
            }
        }
    }

    /// 绘制表头
    fn draw_table_header(&mut self, ui: &mut Ui, process_manager: &mut ProcessManager) {
        let sort_field = process_manager.sort_field();
        let is_desc = process_manager.is_sort_desc();

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            if self.sort_header_button(ui, "PID", SortField::Pid, sort_field, is_desc, 70.0) {
                process_manager.set_sort(SortField::Pid);
            }

            if self.sort_header_button(ui, "名称", SortField::Name, sort_field, is_desc, 180.0) {
                process_manager.set_sort(SortField::Name);
            }

            if self.sort_header_button(ui, "CPU%", SortField::CpuUsage, sort_field, is_desc, 70.0) {
                process_manager.set_sort(SortField::CpuUsage);
            }

            if self.sort_header_button(ui, "内存", SortField::Memory, sort_field, is_desc, 90.0) {
                process_manager.set_sort(SortField::Memory);
            }

            ui.add_sized([70.0, 20.0], egui::Label::new(
                RichText::new("策略").color(Color32::from_gray(180))
            ));

            ui.add_sized([70.0, 20.0], egui::Label::new(
                RichText::new("亲和性").color(Color32::from_gray(180))
            ));
        });
    }

    /// 绘制可排序的表头按钮
    fn sort_header_button(
        &self,
        ui: &mut Ui,
        label: &str,
        field: SortField,
        current_field: SortField,
        is_desc: bool,
        width: f32,
    ) -> bool {
        let is_active = field == current_field;
        let arrow = if is_active {
            if is_desc { " ▼" } else { " ▲" }
        } else {
            ""
        };

        let text = format!("{}{}", label, arrow);
        let color = if is_active {
            Color32::from_rgb(100, 180, 255)
        } else {
            Color32::from_gray(180)
        };

        let response = ui.add_sized(
            [width, 20.0],
            egui::Button::new(RichText::new(text).color(color))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
        );

        response.clicked()
    }

    /// 绘制进程行
    fn draw_process_row(&mut self, ui: &mut Ui, process: &ProcessInfo, logical_cores: usize, idx: usize) {
        let is_selected = self.selected_pid == Some(process.pid);
        let is_editing = self.editing_affinity == Some(process.pid);

        // 斑马纹背景
        let bg_color = if is_selected {
            Color32::from_rgb(50, 70, 90)
        } else if idx % 2 == 0 {
            Color32::from_gray(30)
        } else {
            Color32::from_gray(38)
        };

        Frame::none()
            .fill(bg_color)
            .inner_margin(Margin::symmetric(8.0, 6.0))
            .rounding(Rounding::same(4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // PID
                    let pid_response = ui.add_sized(
                        [70.0, 18.0],
                        egui::SelectableLabel::new(
                            is_selected,
                            RichText::new(format!("{:>6}", process.pid)).monospace(),
                        )
                    );
                    if pid_response.clicked() {
                        self.selected_pid = Some(process.pid);
                    }

                    // 名称
                    ui.add_sized([180.0, 18.0], egui::Label::new(
                        RichText::new(&process.name).color(Color32::WHITE)
                    ).truncate());

                    // CPU 使用率
                    let cpu_color = cpu_usage_color(process.cpu_usage);
                    ui.add_sized([70.0, 18.0], egui::Label::new(
                        RichText::new(format!("{:>5.1}%", process.cpu_usage)).color(cpu_color)
                    ));

                    // 内存
                    ui.add_sized([90.0, 18.0], egui::Label::new(
                        format!("{:>8}", format_memory(process.memory))
                    ));

                    // 调度策略
                    ui.add_sized([70.0, 18.0], egui::Label::new(
                        RichText::new(process.sched_policy.short_name()).color(Color32::from_gray(180))
                    ));

                    // 亲和性
                    if is_editing {
                        self.draw_affinity_editor(ui, process, logical_cores);
                    } else {
                        let affinity_str = self.format_affinity(&process.affinity, logical_cores);
                        if ui.add_sized([70.0, 18.0], egui::Button::new(
                            RichText::new(&affinity_str).size(11.0)
                        ).rounding(Rounding::same(4.0))).clicked() {
                            self.editing_affinity = Some(process.pid);
                            self.affinity_selection = vec![false; logical_cores];
                            for &core in &process.affinity {
                                if core < logical_cores {
                                    self.affinity_selection[core] = true;
                                }
                            }
                        }
                    }
                });
            });
    }

    /// 格式化亲和性显示
    fn format_affinity(&self, affinity: &[usize], logical_cores: usize) -> String {
        if affinity.len() == logical_cores {
            "全部".to_string()
        } else if affinity.len() <= 4 {
            affinity
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",")
        } else {
            format!("{}核", affinity.len())
        }
    }

    /// 绘制亲和性编辑器
    fn draw_affinity_editor(&mut self, ui: &mut Ui, process: &ProcessInfo, logical_cores: usize) {
        ui.horizontal(|ui| {
            // 核心复选框（简化显示）
            let show_count = logical_cores.min(8);
            for (i, selected) in self.affinity_selection.iter_mut().enumerate().take(show_count) {
                let label = format!("{}", i);
                ui.checkbox(selected, label);
            }

            if logical_cores > 8 {
                ui.label(format!("+{}", logical_cores - 8));
            }

            if ui.small_button("✓").clicked() {
                let cores: Vec<usize> = self
                    .affinity_selection
                    .iter()
                    .enumerate()
                    .filter(|(_, &selected)| selected)
                    .map(|(i, _)| i)
                    .collect();

                if cores.is_empty() {
                    self.error_message = Some("至少选择一个核心".to_string());
                } else {
                    match set_process_affinity(process.pid as i32, &cores) {
                        Ok(_) => {
                            self.editing_affinity = None;
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
            }

            if ui.small_button("✕").clicked() {
                self.editing_affinity = None;
            }
        });
    }

    /// 绘制进程详情
    fn draw_process_details(&self, ui: &mut Ui, process: &ProcessInfo) {
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(16.0))
            .rounding(Rounding::same(8.0))
            .stroke(Stroke::new(1.0, Color32::from_gray(60)))
            .show(ui, |ui| {
                ui.label(RichText::new(format!("进程详情: {} (PID: {})", process.name, process.pid))
                    .size(16.0).strong());
                ui.add_space(12.0);

                egui::Grid::new("process_details")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("命令行").color(Color32::from_gray(160)));
                        ui.label(&process.cmd);
                        ui.end_row();

                        ui.label(RichText::new("状态").color(Color32::from_gray(160)));
                        ui.label(&process.status);
                        ui.end_row();

                        ui.label(RichText::new("调度策略").color(Color32::from_gray(160)));
                        ui.label(process.sched_policy.display_name());
                        ui.end_row();

                        ui.label(RichText::new("优先级").color(Color32::from_gray(160)));
                        ui.label(format!("{}", process.priority));
                        ui.end_row();

                        ui.label(RichText::new("CPU 亲和性").color(Color32::from_gray(160)));
                        ui.label(format!("{:?}", process.affinity));
                        ui.end_row();
                    });
            });
    }
}

impl Default for ProcessListPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU 使用率转颜色
fn cpu_usage_color(usage: f32) -> Color32 {
    if usage < 10.0 {
        Color32::from_gray(180)
    } else if usage < 30.0 {
        Color32::from_rgb(100, 200, 100)
    } else if usage < 60.0 {
        Color32::from_rgb(230, 200, 50)
    } else if usage < 85.0 {
        Color32::from_rgb(255, 150, 50)
    } else {
        Color32::from_rgb(255, 80, 80)
    }
}
