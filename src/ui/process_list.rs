//! 进程列表面板

use eframe::egui::{self, Color32, RichText, ScrollArea, TextEdit, Ui};

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
        // 错误消息显示
        let mut clear_error = false;
        if let Some(ref msg) = self.error_message {
            ui.horizontal(|ui| {
                ui.label(RichText::new(msg.as_str()).color(Color32::RED));
                if ui.button("X").clicked() {
                    clear_error = true;
                }
            });
            ui.add_space(4.0);
        }
        if clear_error {
            self.error_message = None;
        }

        // 搜索框
        ui.horizontal(|ui| {
            ui.label("搜索:");
            let mut filter = process_manager.filter().to_string();
            let response = ui.add(TextEdit::singleline(&mut filter).desired_width(200.0));
            if response.changed() {
                process_manager.set_filter(filter);
            }
        });

        ui.add_space(8.0);

        // 表头
        ui.horizontal(|ui| {
            let sort_field = process_manager.sort_field();
            let is_desc = process_manager.is_sort_desc();

            if self.sort_header(ui, "PID", SortField::Pid, sort_field, is_desc, 60.0) {
                process_manager.set_sort(SortField::Pid);
            }
            if self.sort_header(ui, "名称", SortField::Name, sort_field, is_desc, 150.0) {
                process_manager.set_sort(SortField::Name);
            }
            if self.sort_header(ui, "CPU%", SortField::CpuUsage, sort_field, is_desc, 60.0) {
                process_manager.set_sort(SortField::CpuUsage);
            }
            if self.sort_header(ui, "内存", SortField::Memory, sort_field, is_desc, 80.0) {
                process_manager.set_sort(SortField::Memory);
            }
            ui.label("调度策略");
            ui.label("亲和性");
        });

        ui.separator();

        // 进程列表
        ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                let processes = process_manager.filtered_processes();

                for process in processes.iter().take(100) {
                    let is_selected = self.selected_pid == Some(process.pid);
                    let is_editing = self.editing_affinity == Some(process.pid);

                    ui.horizontal(|ui| {
                        // PID
                        let pid_response = ui.selectable_label(
                            is_selected,
                            RichText::new(format!("{:>6}", process.pid)).monospace(),
                        );
                        if pid_response.clicked() {
                            self.selected_pid = Some(process.pid);
                        }

                        // 名称
                        ui.add_sized([150.0, 18.0], egui::Label::new(&process.name).truncate());

                        // CPU 使用率
                        let cpu_color = if process.cpu_usage > 50.0 {
                            Color32::from_rgb(255, 150, 50)
                        } else {
                            Color32::WHITE
                        };
                        ui.label(
                            RichText::new(format!("{:>5.1}%", process.cpu_usage)).color(cpu_color),
                        );

                        // 内存
                        ui.label(format!("{:>8}", format_memory(process.memory)));

                        // 调度策略
                        ui.label(process.sched_policy.short_name());

                        // 亲和性
                        if is_editing {
                            self.draw_affinity_editor(ui, process, logical_cores);
                        } else {
                            let affinity_str = self.format_affinity(&process.affinity, logical_cores);
                            if ui.button(&affinity_str).clicked() {
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
                }
            });

        // 选中进程的详情
        if let Some(pid) = self.selected_pid {
            if let Some(process) = process_manager
                .filtered_processes()
                .iter()
                .find(|p| p.pid == pid)
            {
                ui.add_space(16.0);
                ui.separator();
                self.draw_process_details(ui, process);
            }
        }
    }

    /// 绘制可排序的表头
    fn sort_header(
        &self,
        ui: &mut Ui,
        label: &str,
        field: SortField,
        current_field: SortField,
        is_desc: bool,
        width: f32,
    ) -> bool {
        let text = if field == current_field {
            let arrow = if is_desc { " v" } else { " ^" };
            format!("{}{}", label, arrow)
        } else {
            label.to_string()
        };

        ui.add_sized([width, 18.0], egui::Button::new(text)).clicked()
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
            format!("{}核心", affinity.len())
        }
    }

    /// 绘制亲和性编辑器
    fn draw_affinity_editor(&mut self, ui: &mut Ui, process: &ProcessInfo, logical_cores: usize) {
        ui.horizontal(|ui| {
            // 核心复选框（简化显示）
            for (i, selected) in self.affinity_selection.iter_mut().enumerate().take(logical_cores.min(16)) {
                if ui.checkbox(selected, format!("{}", i)).changed() {
                    // 更新选择状态
                }
            }

            if logical_cores > 16 {
                ui.label(format!("...+{}", logical_cores - 16));
            }

            if ui.button("应用").clicked() {
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

            if ui.button("取消").clicked() {
                self.editing_affinity = None;
            }

            if ui.button("全选").clicked() {
                for s in &mut self.affinity_selection {
                    *s = true;
                }
            }

            if ui.button("清除").clicked() {
                for s in &mut self.affinity_selection {
                    *s = false;
                }
            }
        });
    }

    /// 绘制进程详情
    fn draw_process_details(&self, ui: &mut Ui, process: &ProcessInfo) {
        ui.heading(format!("进程详情: {} (PID: {})", process.name, process.pid));
        ui.add_space(8.0);

        egui::Grid::new("process_details")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label("命令行:");
                ui.label(&process.cmd);
                ui.end_row();

                ui.label("状态:");
                ui.label(&process.status);
                ui.end_row();

                ui.label("调度策略:");
                ui.label(process.sched_policy.display_name());
                ui.end_row();

                ui.label("优先级:");
                ui.label(format!("{}", process.priority));
                ui.end_row();

                ui.label("CPU 亲和性:");
                ui.label(format!("{:?}", process.affinity));
                ui.end_row();
            });
    }
}

impl Default for ProcessListPanel {
    fn default() -> Self {
        Self::new()
    }
}
