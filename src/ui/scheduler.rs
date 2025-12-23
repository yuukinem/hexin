//! 调度策略配置面板

use eframe::egui::{self, Color32, ComboBox, Frame, Margin, RichText, Rounding, ScrollArea, Slider, Stroke, TextEdit, Ui};

use crate::system::{
    get_rt_priority_range, set_process_affinity, set_process_nice, set_scheduler,
    ProcessManager, SchedulePolicy, SchedulePreset,
};

/// 调度策略面板
pub struct SchedulerPanel {
    /// 选中的进程 PID
    selected_pid: Option<u32>,
    /// 编辑中的策略
    editing_policy: SchedulePolicy,
    /// 编辑中的优先级
    editing_priority: i32,
    /// 预设列表
    presets: Vec<SchedulePreset>,
    /// PID 输入框
    pid_input: String,
    /// 错误消息
    error_message: Option<String>,
    /// 成功消息
    success_message: Option<String>,
}

impl SchedulerPanel {
    pub fn new(vcache_cores: &[usize], all_cores: usize) -> Self {
        Self {
            selected_pid: None,
            editing_policy: SchedulePolicy::Other,
            editing_priority: 0,
            presets: SchedulePreset::builtin_presets(vcache_cores, all_cores),
            pid_input: String::new(),
            error_message: None,
            success_message: None,
        }
    }

    /// 绘制面板
    pub fn ui(&mut self, ui: &mut Ui, process_manager: &ProcessManager, logical_cores: usize) {
        ui.add_space(8.0);

        // 消息显示
        self.draw_messages(ui);

        // 主布局：左右分栏
        ui.horizontal(|ui| {
            // 左侧：调度配置
            ui.vertical(|ui| {
                ui.set_min_width(380.0);
                self.draw_scheduler_config(ui, process_manager);
                ui.add_space(16.0);
                self.draw_presets(ui, logical_cores);
            });

            ui.add_space(16.0);

            // 右侧：快速选择进程
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                self.draw_process_selector(ui, process_manager);
            });
        });
    }

    /// 绘制消息提示
    fn draw_messages(&mut self, ui: &mut Ui) {
        let mut clear_error = false;
        let mut clear_success = false;

        if let Some(ref msg) = self.error_message {
            Frame::none()
                .fill(Color32::from_rgb(80, 30, 30))
                .inner_margin(Margin::same(10.0))
                .rounding(Rounding::same(6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✕").size(14.0).color(Color32::from_rgb(255, 100, 100)));
                        ui.label(RichText::new(msg.as_str()).color(Color32::from_rgb(255, 150, 150)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("关闭").clicked() {
                                clear_error = true;
                            }
                        });
                    });
                });
            ui.add_space(8.0);
        }

        if let Some(ref msg) = self.success_message {
            Frame::none()
                .fill(Color32::from_rgb(30, 70, 40))
                .inner_margin(Margin::same(10.0))
                .rounding(Rounding::same(6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✓").size(14.0).color(Color32::from_rgb(100, 255, 100)));
                        ui.label(RichText::new(msg.as_str()).color(Color32::from_rgb(150, 255, 150)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("关闭").clicked() {
                                clear_success = true;
                            }
                        });
                    });
                });
            ui.add_space(8.0);
        }

        if clear_error {
            self.error_message = None;
        }
        if clear_success {
            self.success_message = None;
        }
    }

    /// 绘制调度配置区域
    fn draw_scheduler_config(&mut self, ui: &mut Ui, process_manager: &ProcessManager) {
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(16.0))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("调度策略配置").size(16.0).strong());
                ui.add_space(16.0);

                // PID 输入
                ui.horizontal(|ui| {
                    ui.label(RichText::new("进程 PID").color(Color32::from_gray(160)));
                    ui.add_space(8.0);
                    let response = ui.add(
                        TextEdit::singleline(&mut self.pid_input)
                            .desired_width(120.0)
                            .hint_text("输入 PID")
                    );
                    if response.changed() {
                        if let Ok(pid) = self.pid_input.parse::<u32>() {
                            self.selected_pid = Some(pid);
                            if let Some(process) = process_manager
                                .filtered_processes()
                                .iter()
                                .find(|p| p.pid == pid)
                            {
                                self.editing_policy = process.sched_policy;
                                self.editing_priority = process.priority;
                            }
                        }
                    }

                    // 显示选中的进程名
                    if let Some(pid) = self.selected_pid {
                        if let Some(process) = process_manager
                            .filtered_processes()
                            .iter()
                            .find(|p| p.pid == pid)
                        {
                            ui.add_space(12.0);
                            ui.label(RichText::new(&process.name).color(Color32::from_rgb(100, 180, 255)));
                        }
                    }
                });

                ui.add_space(16.0);

                // 策略选择
                ui.horizontal(|ui| {
                    ui.label(RichText::new("调度策略").color(Color32::from_gray(160)));
                    ui.add_space(8.0);
                    ComboBox::from_id_salt("sched_policy")
                        .width(180.0)
                        .selected_text(self.editing_policy.display_name())
                        .show_ui(ui, |ui| {
                            for policy in SchedulePolicy::all() {
                                ui.selectable_value(
                                    &mut self.editing_policy,
                                    *policy,
                                    policy.display_name(),
                                );
                            }
                        });
                });

                ui.add_space(12.0);

                // 优先级调整
                if self.editing_policy.is_realtime() {
                    let (min, max) = get_rt_priority_range(self.editing_policy);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("实时优先级").color(Color32::from_gray(160)));
                        ui.add_space(8.0);
                        ui.add(Slider::new(&mut self.editing_priority, min..=max).show_value(true));
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("⚠ 实时调度可能影响系统稳定性").size(11.0).color(Color32::from_rgb(255, 200, 100)));
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Nice 值").color(Color32::from_gray(160)));
                        ui.add_space(8.0);
                        ui.add(Slider::new(&mut self.editing_priority, -20..=19).show_value(true));
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new("-20 最高优先级，19 最低优先级").size(11.0).color(Color32::from_gray(140)));
                }

                ui.add_space(16.0);

                // 应用按钮
                let button = egui::Button::new(RichText::new("应用调度策略").size(14.0))
                    .fill(Color32::from_rgb(60, 100, 140))
                    .rounding(Rounding::same(6.0));

                if ui.add_sized([160.0, 32.0], button).clicked() {
                    if let Some(pid) = self.selected_pid {
                        self.apply_scheduler(pid as i32);
                    } else {
                        self.error_message = Some("请输入有效的 PID".to_string());
                    }
                }
            });
    }

    /// 绘制预设配置区域
    fn draw_presets(&mut self, ui: &mut Ui, logical_cores: usize) {
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(16.0))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("快速预设").size(16.0).strong());
                ui.add_space(12.0);

                let presets_clone: Vec<SchedulePreset> = self.presets.clone();
                let mut apply_preset: Option<(i32, SchedulePreset)> = None;

                ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for preset in &presets_clone {
                            Frame::none()
                                .fill(Color32::from_gray(45))
                                .inner_margin(Margin::same(12.0))
                                .rounding(Rounding::same(6.0))
                                .stroke(Stroke::new(1.0, Color32::from_gray(55)))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&preset.name).strong().color(Color32::WHITE));
                                        ui.label(RichText::new("-").color(Color32::from_gray(100)));
                                        ui.label(RichText::new(&preset.description).size(12.0).color(Color32::from_gray(160)));
                                    });

                                    ui.add_space(6.0);

                                    ui.horizontal(|ui| {
                                        // 策略标签
                                        Frame::none()
                                            .fill(Color32::from_rgb(50, 70, 90))
                                            .inner_margin(Margin::symmetric(8.0, 4.0))
                                            .rounding(Rounding::same(4.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(preset.policy.short_name()).size(11.0));
                                            });

                                        if preset.policy == SchedulePolicy::Other && preset.priority != 0 {
                                            Frame::none()
                                                .fill(Color32::from_rgb(70, 60, 40))
                                                .inner_margin(Margin::symmetric(8.0, 4.0))
                                                .rounding(Rounding::same(4.0))
                                                .show(ui, |ui| {
                                                    ui.label(RichText::new(format!("Nice: {}", preset.priority)).size(11.0));
                                                });
                                        }

                                        if let Some(ref cores) = preset.affinity_cores {
                                            if cores.len() < logical_cores {
                                                Frame::none()
                                                    .fill(Color32::from_rgb(40, 70, 50))
                                                    .inner_margin(Margin::symmetric(8.0, 4.0))
                                                    .rounding(Rounding::same(4.0))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(format!("{}核", cores.len())).size(11.0));
                                                    });
                                            }
                                        }

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.small_button("应用").clicked() {
                                                if let Some(pid) = self.selected_pid {
                                                    apply_preset = Some((pid as i32, preset.clone()));
                                                } else {
                                                    self.error_message = Some("请先选择进程".to_string());
                                                }
                                            }
                                        });
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });

                if let Some((pid, preset)) = apply_preset {
                    self.apply_preset(pid, &preset, logical_cores);
                }
            });
    }

    /// 绘制进程选择器
    fn draw_process_selector(&mut self, ui: &mut Ui, process_manager: &ProcessManager) {
        Frame::none()
            .fill(Color32::from_gray(35))
            .inner_margin(Margin::same(16.0))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("快速选择进程").size(16.0).strong());
                ui.add_space(4.0);
                ui.label(RichText::new("按 CPU 使用率排序").size(11.0).color(Color32::from_gray(140)));
                ui.add_space(12.0);

                ScrollArea::vertical()
                    .max_height(400.0)
                    .id_salt("process_select")
                    .show(ui, |ui| {
                        let processes = process_manager.filtered_processes();
                        for (idx, process) in processes.iter().take(30).enumerate() {
                            let is_selected = self.selected_pid == Some(process.pid);

                            let bg_color = if is_selected {
                                Color32::from_rgb(50, 80, 110)
                            } else if idx % 2 == 0 {
                                Color32::from_gray(40)
                            } else {
                                Color32::from_gray(45)
                            };

                            Frame::none()
                                .fill(bg_color)
                                .inner_margin(Margin::symmetric(10.0, 6.0))
                                .rounding(Rounding::same(4.0))
                                .show(ui, |ui| {
                                    let response = ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{:>6}", process.pid)).monospace().size(11.0).color(Color32::from_gray(140)));
                                        ui.add_space(8.0);
                                        ui.add(egui::Label::new(
                                            RichText::new(&process.name).color(Color32::WHITE)
                                        ).truncate());

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let cpu_color = if process.cpu_usage > 50.0 {
                                                Color32::from_rgb(255, 150, 50)
                                            } else if process.cpu_usage > 10.0 {
                                                Color32::from_rgb(100, 200, 100)
                                            } else {
                                                Color32::from_gray(140)
                                            };
                                            ui.label(RichText::new(format!("{:.1}%", process.cpu_usage)).color(cpu_color));
                                        });
                                    }).response;

                                    if response.interact(egui::Sense::click()).clicked() {
                                        self.selected_pid = Some(process.pid);
                                        self.pid_input = process.pid.to_string();
                                        self.editing_policy = process.sched_policy;
                                        self.editing_priority = process.priority;
                                    }
                                });
                        }
                    });
            });
    }

    /// 应用调度策略
    fn apply_scheduler(&mut self, pid: i32) {
        if self.editing_policy.is_realtime() {
            match set_scheduler(pid, self.editing_policy, self.editing_priority) {
                Ok(_) => {
                    self.success_message = Some("调度策略已应用".to_string());
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(e);
                    self.success_message = None;
                }
            }
        } else {
            match set_scheduler(pid, self.editing_policy, 0) {
                Ok(_) => {
                    if self.editing_priority != 0 {
                        if let Err(e) = set_process_nice(pid, self.editing_priority) {
                            self.error_message = Some(e);
                            return;
                        }
                    }
                    self.success_message = Some("调度策略已应用".to_string());
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(e);
                    self.success_message = None;
                }
            }
        }
    }

    /// 应用预设
    fn apply_preset(&mut self, pid: i32, preset: &SchedulePreset, _logical_cores: usize) {
        let priority = if preset.policy.is_realtime() {
            preset.priority
        } else {
            0
        };

        match set_scheduler(pid, preset.policy, priority) {
            Ok(_) => {
                if !preset.policy.is_realtime() && preset.priority != 0 {
                    if let Err(e) = set_process_nice(pid, preset.priority) {
                        self.error_message = Some(format!("设置 nice 值失败: {}", e));
                        return;
                    }
                }

                if let Some(ref cores) = preset.affinity_cores {
                    if let Err(e) = set_process_affinity(pid, cores) {
                        self.error_message = Some(format!("设置亲和性失败: {}", e));
                        return;
                    }
                }

                self.success_message = Some(format!("预设 '{}' 已应用", preset.name));
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(e);
                self.success_message = None;
            }
        }
    }
}
