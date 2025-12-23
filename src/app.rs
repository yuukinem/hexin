//! 主应用状态和 UI 协调

use eframe::egui::{self, CentralPanel, Color32, Context, FontData, FontDefinitions, FontFamily, Frame, Margin, RichText, Rounding, TopBottomPanel};
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
    /// 上次 CPU 更新时间
    last_cpu_update: Instant,
    /// 上次进程更新时间
    last_process_update: Instant,
    /// 启动时间（用于历史图表的时间戳）
    start_time: Instant,
}

impl HexinApp {
    /// 配置字体，添加中文支持（嵌入字体）
    fn setup_fonts(ctx: &Context) {
        let mut fonts = FontDefinitions::default();

        // 嵌入 Noto Sans SC 中文字体
        let font_data = include_bytes!("../assets/NotoSansSC-Regular.ttf");
        fonts.font_data.insert(
            "noto_sans_sc".to_owned(),
            FontData::from_static(font_data),
        );

        // 将中文字体添加到默认字体族
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .push("noto_sans_sc".to_owned());
        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .push("noto_sans_sc".to_owned());

        ctx.set_fonts(fonts);
    }

    /// 创建新应用
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 配置中文字体
        Self::setup_fonts(&cc.egui_ctx);

        let config = AppConfig::load();
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_info = CpuInfo::detect();
        let logical_cores = cpu_info.logical_cores;
        let vcache_cores = cpu_info.vcache_cores();

        let cpu_history = CpuHistory::new(logical_cores, config.history_length);
        let mut process_manager = ProcessManager::new(logical_cores);

        // 初始化时加载进程列表
        process_manager.update(&sys);

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
            last_cpu_update: Instant::now(),
            last_process_update: Instant::now(),
            start_time: Instant::now(),
        }
    }

    /// 更新系统数据
    fn update_data(&mut self) {
        let now = Instant::now();

        // CPU 更新 (每 500ms)
        let cpu_elapsed = now.duration_since(self.last_cpu_update);
        if cpu_elapsed >= Duration::from_millis(self.config.refresh_interval_ms) {
            self.last_cpu_update = now;

            // 刷新 CPU 信息
            self.sys.refresh_cpu_all();
            self.cpu_info.update(&self.sys);

            // 记录历史数据
            let core_usages: Vec<f32> = self.cpu_info.cores.iter().map(|c| c.usage_percent).collect();
            let timestamp = now.duration_since(self.start_time).as_secs_f64();
            self.cpu_history.push(&core_usages, self.cpu_info.total_usage_percent, timestamp);
        }

        // 进程更新 (每 1000ms)
        let process_elapsed = now.duration_since(self.last_process_update);
        if process_elapsed >= Duration::from_millis(1000) {
            self.last_process_update = now;
            self.sys.refresh_processes(ProcessesToUpdate::All, true);
            self.process_manager.update(&self.sys);
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
        TopBottomPanel::top("tabs")
            .frame(Frame::none()
                .fill(Color32::from_gray(30))
                .inner_margin(Margin::symmetric(16.0, 8.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Logo
                    ui.label(RichText::new("hexin").size(18.0).strong().color(Color32::from_rgb(100, 180, 255)));
                    ui.add_space(24.0);

                    // 标签按钮
                    let tabs = [
                        (Tab::CpuMonitor, "CPU 监控"),
                        (Tab::ProcessList, "进程管理"),
                        (Tab::Scheduler, "调度策略"),
                    ];

                    for (tab, label) in tabs {
                        let is_selected = self.current_tab == tab;
                        let text_color = if is_selected {
                            Color32::WHITE
                        } else {
                            Color32::from_gray(160)
                        };

                        Frame::none()
                            .fill(if is_selected { Color32::from_rgb(60, 90, 120) } else { Color32::TRANSPARENT })
                            .rounding(Rounding::same(6.0))
                            .inner_margin(Margin::symmetric(12.0, 6.0))
                            .show(ui, |ui| {
                                if ui.add(egui::Label::new(
                                    RichText::new(label).color(text_color).size(13.0)
                                ).sense(egui::Sense::click())).clicked() {
                                    self.current_tab = tab;
                                }
                            });

                        ui.add_space(4.0);
                    }

                    // 右侧状态信息
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let usage_color = if self.cpu_info.total_usage_percent > 80.0 {
                            Color32::from_rgb(255, 100, 100)
                        } else if self.cpu_info.total_usage_percent > 50.0 {
                            Color32::from_rgb(255, 200, 100)
                        } else {
                            Color32::from_rgb(100, 200, 100)
                        };

                        ui.label(RichText::new(format!("核心: {}", self.cpu_info.logical_cores))
                            .size(12.0).color(Color32::from_gray(140)));
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("CPU: {:.1}%", self.cpu_info.total_usage_percent))
                            .size(12.0).color(usage_color));
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
