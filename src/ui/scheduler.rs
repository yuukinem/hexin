//! 调度策略配置面板

use eframe::egui::{Color32, ComboBox, RichText, ScrollArea, Slider, TextEdit, Ui};

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
        // 消息显示
        let mut clear_error = false;
        let mut clear_success = false;

        if let Some(ref msg) = self.error_message {
            ui.horizontal(|ui| {
                ui.label(RichText::new(msg.as_str()).color(Color32::RED));
                if ui.button("X").clicked() {
                    clear_error = true;
                }
            });
        }
        if let Some(ref msg) = self.success_message {
            ui.horizontal(|ui| {
                ui.label(RichText::new(msg.as_str()).color(Color32::GREEN));
                if ui.button("X").clicked() {
                    clear_success = true;
                }
            });
        }

        if clear_error {
            self.error_message = None;
        }
        if clear_success {
            self.success_message = None;
        }

        ui.heading("调度策略配置");
        ui.add_space(8.0);

        // PID 输入
        ui.horizontal(|ui| {
            ui.label("进程 PID:");
            let response = ui.add(TextEdit::singleline(&mut self.pid_input).desired_width(100.0));
            if response.changed() {
                if let Ok(pid) = self.pid_input.parse::<u32>() {
                    self.selected_pid = Some(pid);
                    // 查找进程并加载当前策略
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
        });

        ui.add_space(16.0);

        // 策略选择
        ui.horizontal(|ui| {
            ui.label("调度策略:");
            ComboBox::from_id_salt("sched_policy")
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

        // 优先级调整
        ui.add_space(8.0);
        if self.editing_policy.is_realtime() {
            let (min, max) = get_rt_priority_range(self.editing_policy);
            ui.horizontal(|ui| {
                ui.label("实时优先级:");
                ui.add(Slider::new(&mut self.editing_priority, min..=max));
            });
            ui.label(
                RichText::new("警告: 实时调度可能影响系统稳定性").color(Color32::YELLOW),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label("Nice 值:");
                ui.add(Slider::new(&mut self.editing_priority, -20..=19));
            });
            ui.label("(-20 最高优先级, 19 最低优先级)");
        }

        ui.add_space(16.0);

        // 应用按钮
        if ui.button("应用调度策略").clicked() {
            if let Some(pid) = self.selected_pid {
                self.apply_scheduler(pid as i32);
            } else {
                self.error_message = Some("请输入有效的 PID".to_string());
            }
        }

        ui.add_space(24.0);
        ui.separator();

        // 预设配置
        ui.heading("快速预设");
        ui.add_space(8.0);

        // 克隆预设以避免借用问题
        let presets_clone: Vec<SchedulePreset> = self.presets.clone();
        let mut apply_preset: Option<(i32, SchedulePreset)> = None;

        ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for preset in &presets_clone {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&preset.name);
                            ui.label("-");
                            ui.label(&preset.description);
                        });

                        ui.horizontal(|ui| {
                            ui.label(format!("策略: {}", preset.policy.short_name()));
                            if preset.policy == SchedulePolicy::Other {
                                ui.label(format!("Nice: {}", preset.priority));
                            }
                            if let Some(ref cores) = preset.affinity_cores {
                                ui.label(format!("核心: {:?}", cores));
                            }
                        });

                        if ui.button("应用到选中进程").clicked() {
                            if let Some(pid) = self.selected_pid {
                                apply_preset = Some((pid as i32, preset.clone()));
                            } else {
                                self.error_message = Some("请先选择进程".to_string());
                            }
                        }
                    });
                    ui.add_space(4.0);
                }
            });

        // 在循环外应用预设
        if let Some((pid, preset)) = apply_preset {
            self.apply_preset(pid, &preset, logical_cores);
        }

        ui.add_space(16.0);

        // 从进程列表选择
        ui.heading("快速选择进程");
        ui.add_space(8.0);

        ScrollArea::vertical()
            .max_height(150.0)
            .id_salt("process_select")
            .show(ui, |ui| {
                let processes = process_manager.filtered_processes();
                for process in processes.iter().take(20) {
                    let is_selected = self.selected_pid == Some(process.pid);
                    let text = format!(
                        "[{}] {} - CPU: {:.1}%",
                        process.pid, process.name, process.cpu_usage
                    );
                    if ui.selectable_label(is_selected, text).clicked() {
                        self.selected_pid = Some(process.pid);
                        self.pid_input = process.pid.to_string();
                        self.editing_policy = process.sched_policy;
                        self.editing_priority = process.priority;
                    }
                }
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
            // 非实时策略，设置 nice 值
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
        // 应用调度策略
        let priority = if preset.policy.is_realtime() {
            preset.priority
        } else {
            0
        };

        match set_scheduler(pid, preset.policy, priority) {
            Ok(_) => {
                // 应用 nice 值（如果是非实时策略）
                if !preset.policy.is_realtime() && preset.priority != 0 {
                    if let Err(e) = set_process_nice(pid, preset.priority) {
                        self.error_message = Some(format!("设置 nice 值失败: {}", e));
                        return;
                    }
                }

                // 应用亲和性
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
