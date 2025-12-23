//! Linux 调度策略 API 封装

use serde::{Deserialize, Serialize};
use std::fs;

// Linux 调度策略常量
#[cfg(target_os = "linux")]
mod linux_sched {
    pub const SCHED_OTHER: i32 = 0;
    pub const SCHED_FIFO: i32 = 1;
    pub const SCHED_RR: i32 = 2;
    pub const SCHED_BATCH: i32 = 3;
    pub const SCHED_IDLE: i32 = 5;
}

#[cfg(not(target_os = "linux"))]
mod linux_sched {
    pub const SCHED_OTHER: i32 = 0;
    pub const SCHED_FIFO: i32 = 1;
    pub const SCHED_RR: i32 = 2;
    pub const SCHED_BATCH: i32 = 3;
    pub const SCHED_IDLE: i32 = 5;
}

use linux_sched::*;

/// 调度策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulePolicy {
    /// 默认时间片轮转 (CFS)
    Other,
    /// 先进先出实时调度
    Fifo,
    /// 轮转实时调度
    RoundRobin,
    /// 批处理
    Batch,
    /// 空闲时运行
    Idle,
    /// 未知策略
    Unknown(i32),
}

impl SchedulePolicy {
    /// 从 libc 常量转换
    pub fn from_raw(policy: i32) -> Self {
        match policy {
            x if x == SCHED_OTHER => SchedulePolicy::Other,
            x if x == SCHED_FIFO => SchedulePolicy::Fifo,
            x if x == SCHED_RR => SchedulePolicy::RoundRobin,
            x if x == SCHED_BATCH => SchedulePolicy::Batch,
            x if x == SCHED_IDLE => SchedulePolicy::Idle,
            other => SchedulePolicy::Unknown(other),
        }
    }

    /// 转换为 libc 常量
    pub fn to_raw(&self) -> i32 {
        match self {
            SchedulePolicy::Other => SCHED_OTHER,
            SchedulePolicy::Fifo => SCHED_FIFO,
            SchedulePolicy::RoundRobin => SCHED_RR,
            SchedulePolicy::Batch => SCHED_BATCH,
            SchedulePolicy::Idle => SCHED_IDLE,
            SchedulePolicy::Unknown(v) => *v,
        }
    }

    /// 显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            SchedulePolicy::Other => "SCHED_OTHER (默认)",
            SchedulePolicy::Fifo => "SCHED_FIFO (实时先进先出)",
            SchedulePolicy::RoundRobin => "SCHED_RR (实时轮转)",
            SchedulePolicy::Batch => "SCHED_BATCH (批处理)",
            SchedulePolicy::Idle => "SCHED_IDLE (空闲)",
            SchedulePolicy::Unknown(_) => "未知",
        }
    }

    /// 短名称
    pub fn short_name(&self) -> &'static str {
        match self {
            SchedulePolicy::Other => "OTHER",
            SchedulePolicy::Fifo => "FIFO",
            SchedulePolicy::RoundRobin => "RR",
            SchedulePolicy::Batch => "BATCH",
            SchedulePolicy::Idle => "IDLE",
            SchedulePolicy::Unknown(_) => "???",
        }
    }

    /// 是否为实时策略
    pub fn is_realtime(&self) -> bool {
        matches!(self, SchedulePolicy::Fifo | SchedulePolicy::RoundRobin)
    }

    /// 所有可用策略
    pub fn all() -> &'static [SchedulePolicy] {
        &[
            SchedulePolicy::Other,
            SchedulePolicy::Batch,
            SchedulePolicy::Idle,
            SchedulePolicy::Fifo,
            SchedulePolicy::RoundRobin,
        ]
    }
}

/// 获取进程的调度策略和优先级 (Linux only)
#[cfg(target_os = "linux")]
pub fn get_scheduler_info(pid: i32) -> (SchedulePolicy, i32) {
    use libc::sched_getscheduler;

    unsafe {
        let policy = sched_getscheduler(pid);
        if policy < 0 {
            return (SchedulePolicy::Unknown(-1), 0);
        }

        let priority = get_process_nice(pid);
        (SchedulePolicy::from_raw(policy), priority)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn get_scheduler_info(_pid: i32) -> (SchedulePolicy, i32) {
    (SchedulePolicy::Other, 0)
}

/// 设置进程的调度策略 (Linux only)
#[cfg(target_os = "linux")]
pub fn set_scheduler(pid: i32, policy: SchedulePolicy, priority: i32) -> Result<(), String> {
    use libc::{sched_param, sched_setscheduler};

    let param = sched_param {
        sched_priority: if policy.is_realtime() { priority } else { 0 },
    };

    let result = unsafe { sched_setscheduler(pid, policy.to_raw(), &param) };

    if result == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        Err(format!("设置调度策略失败: {} (可能需要 root 权限或 CAP_SYS_NICE)", err))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_scheduler(_pid: i32, _policy: SchedulePolicy, _priority: i32) -> Result<(), String> {
    Err("调度策略设置仅支持 Linux".to_string())
}

/// 获取进程的 nice 值
pub fn get_process_nice(pid: i32) -> i32 {
    let path = format!("/proc/{}/stat", pid);
    if let Ok(content) = fs::read_to_string(&path) {
        // /proc/[pid]/stat 的第 19 个字段是 nice 值
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() > 18 {
            return parts[18].parse().unwrap_or(0);
        }
    }
    0
}

/// 设置进程的 nice 值 (Linux only)
#[cfg(target_os = "linux")]
pub fn set_process_nice(pid: i32, nice: i32) -> Result<(), String> {
    use libc::{setpriority, PRIO_PROCESS};

    let result = unsafe { setpriority(PRIO_PROCESS, pid as u32, nice) };

    if result == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        Err(format!("设置 nice 值失败: {}", err))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_process_nice(_pid: i32, _nice: i32) -> Result<(), String> {
    Err("nice 值设置仅支持 Linux".to_string())
}

/// 获取实时优先级范围
#[cfg(target_os = "linux")]
pub fn get_rt_priority_range(policy: SchedulePolicy) -> (i32, i32) {
    if policy.is_realtime() {
        unsafe {
            let min = libc::sched_get_priority_min(policy.to_raw());
            let max = libc::sched_get_priority_max(policy.to_raw());
            (min, max)
        }
    } else {
        (0, 0)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn get_rt_priority_range(_policy: SchedulePolicy) -> (i32, i32) {
    (1, 99)
}

/// 预设配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulePreset {
    pub name: String,
    pub description: String,
    pub policy: SchedulePolicy,
    pub priority: i32,
    pub affinity_cores: Option<Vec<usize>>,
}

impl SchedulePreset {
    /// 内置预设
    pub fn builtin_presets(vcache_cores: &[usize], all_cores: usize) -> Vec<SchedulePreset> {
        let mut presets = vec![
            SchedulePreset {
                name: "默认".to_string(),
                description: "使用系统默认调度".to_string(),
                policy: SchedulePolicy::Other,
                priority: 0,
                affinity_cores: None,
            },
            SchedulePreset {
                name: "高优先级".to_string(),
                description: "较低的 nice 值，获得更多 CPU 时间".to_string(),
                policy: SchedulePolicy::Other,
                priority: -10,
                affinity_cores: None,
            },
            SchedulePreset {
                name: "后台任务".to_string(),
                description: "低优先级，仅在空闲时运行".to_string(),
                policy: SchedulePolicy::Idle,
                priority: 0,
                affinity_cores: None,
            },
            SchedulePreset {
                name: "实时 (FIFO)".to_string(),
                description: "实时调度，最高优先级".to_string(),
                policy: SchedulePolicy::Fifo,
                priority: 50,
                affinity_cores: None,
            },
        ];

        // 如果有 V-Cache 核心，添加游戏模式预设
        if !vcache_cores.is_empty() {
            presets.push(SchedulePreset {
                name: "游戏模式 (V-Cache)".to_string(),
                description: "绑定到 3D V-Cache 核心".to_string(),
                policy: SchedulePolicy::Other,
                priority: -5,
                affinity_cores: Some(vcache_cores.to_vec()),
            });

            // 非 V-Cache 核心
            let non_vcache: Vec<usize> = (0..all_cores)
                .filter(|c| !vcache_cores.contains(c))
                .collect();

            if !non_vcache.is_empty() {
                presets.push(SchedulePreset {
                    name: "渲染/编译模式".to_string(),
                    description: "绑定到非 V-Cache 核心".to_string(),
                    policy: SchedulePolicy::Other,
                    priority: 0,
                    affinity_cores: Some(non_vcache),
                });
            }
        }

        presets
    }
}
