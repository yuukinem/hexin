//! 主应用状态和 UI 协调

use eframe::egui::{self, CentralPanel, Context, TopBottomPanel};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

use crate::system::{CpuInfo, ProcessManager};
use crate::ui::{CpuMonitorPanel, ProcessListPanel, SchedulerPanel};
use crate::utils::CpuHistory;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 刷新间隔 (毫秒)
    pub refresh_interval_ms: u64,
    /// 历史数据长度 (数据点数)
    pub history_length: usize,
    /// 窗口宽度
    pub window_width: f32,
    /// 窗口高度
    pub window_height: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 500,
            history_length: 120, // 60 秒 @ 500ms
            window_width: 1000.0,
            window_height: 700.0,
        }
    }
}

impl AppConfig {
    /// 获取配置文件路径
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("hexin").join("config.toml"))
    }

    /// 加载配置
    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// 保存配置
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = toml::to_string_pretty(self) {
                let _ = fs::write(&path, content);
            }
        }
    }
}

/// 当前标签页
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    CpuMonitor,
    ProcessList,
    Scheduler,
}

/// 主应用
pub struct HexinApp {
    /// 应用配置
    config: AppConfig,
    /// 系统信息
    sys: System,
    /// CPU 信息
    cpu_info: CpuInfo,
    /// CPU 历史数据
    cpu_history: CpuHistory,
    /// 进程管理器
    process_manager: ProcessManager,
    /// 当前标签页
    current_tab: Tab,
    /// CPU 监控面板
    cpu_monitor_panel: CpuMonitorPanel,
    /// 进程列表面板
    process_list_panel: ProcessListPanel,
    /// 调度策略面板
    scheduler_panel: SchedulerPanel,
    /// 上次更新时间
    last_update: Instant,
    /// 启动时间（用于历史图表的时间戳）
    start_time: Instant,
}

impl HexinApp {
    /// 创建新应用
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_info = CpuInfo::detect();
        let logical_cores = cpu_info.logical_cores;
        let vcache_cores = cpu_info.vcache_cores();

        let cpu_history = CpuHistory::new(logical_cores, config.history_length);
        let process_manager = ProcessManager::new(logical_cores);

        Self {
            config,
            sys,
            cpu_info,
            cpu_history,
            process_manager,
            current_tab: Tab::CpuMonitor,
            cpu_monitor_panel: CpuMonitorPanel::new(),
            process_list_panel: ProcessListPanel::new(),
            scheduler_panel: SchedulerPanel::new(&vcache_cores, logical_cores),
            last_update: Instant::now(),
            start_time: Instant::now(),
        }
    }

    /// 更新系统数据
    fn update_data(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);

        if elapsed >= Duration::from_millis(self.config.refresh_interval_ms) {
            self.last_update = now;

            // 刷新 CPU 信息
            self.sys.refresh_cpu_all();
            self.cpu_info.update(&self.sys);

            // 记录历史数据
            let core_usages: Vec<f32> = self.cpu_info.cores.iter().map(|c| c.usage_percent).collect();
            let timestamp = now.duration_since(self.start_time).as_secs_f64();
            self.cpu_history.push(&core_usages, self.cpu_info.total_usage_percent, timestamp);

            // 刷新进程信息（不是每次都刷新，减少开销）
            if elapsed >= Duration::from_millis(1000) {
                self.sys.refresh_processes(ProcessesToUpdate::All, true);
                self.process_manager.update(&self.sys);
            }
        }
    }
}

impl eframe::App for HexinApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 更新数据
        self.update_data();

        // 请求持续重绘
        ctx.request_repaint_after(Duration::from_millis(self.config.refresh_interval_ms));

        // 顶部标签栏
        TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("hexin");
                ui.separator();

                if ui.selectable_label(self.current_tab == Tab::CpuMonitor, "CPU 监控").clicked() {
                    self.current_tab = Tab::CpuMonitor;
                }
                if ui.selectable_label(self.current_tab == Tab::ProcessList, "进程管理").clicked() {
                    self.current_tab = Tab::ProcessList;
                }
                if ui.selectable_label(self.current_tab == Tab::Scheduler, "调度策略").clicked() {
                    self.current_tab = Tab::Scheduler;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "CPU: {:.1}% | 核心: {}",
                        self.cpu_info.total_usage_percent,
                        self.cpu_info.logical_cores
                    ));
                });
            });
        });

        // 主内容区域
        CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.current_tab {
                    Tab::CpuMonitor => {
                        self.cpu_monitor_panel.ui(ui, &self.cpu_info, &self.cpu_history);
                    }
                    Tab::ProcessList => {
                        self.process_list_panel.ui(
                            ui,
                            &mut self.process_manager,
                            self.cpu_info.logical_cores,
                        );
                    }
                    Tab::Scheduler => {
                        self.scheduler_panel.ui(
                            ui,
                            &self.process_manager,
                            self.cpu_info.logical_cores,
                        );
                    }
                }
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config.save();
    }
}
