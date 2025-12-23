//! 进程信息和管理模块

use serde::{Deserialize, Serialize};
use sysinfo::{Process, System};

/// 进程信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// 进程 ID
    pub pid: u32,
    /// 进程名称
    pub name: String,
    /// 命令行
    pub cmd: String,
    /// CPU 使用率
    pub cpu_usage: f32,
    /// 内存使用 (字节)
    pub memory: u64,
    /// 进程状态
    pub status: String,
    /// CPU 亲和性掩码
    pub affinity: Vec<usize>,
    /// 调度策略
    pub sched_policy: super::SchedulePolicy,
    /// 优先级/nice 值
    pub priority: i32,
}

impl ProcessInfo {
    /// 从 sysinfo Process 创建
    pub fn from_process(pid: u32, process: &Process, logical_cores: usize) -> Self {
        let cmd: Vec<String> = process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();
        let cmd_str = cmd.join(" ");
        let affinity = get_process_affinity(pid as i32, logical_cores);
        let (sched_policy, priority) = super::get_scheduler_info(pid as i32);

        ProcessInfo {
            pid,
            name: process.name().to_string_lossy().to_string(),
            cmd: if cmd_str.is_empty() {
                process.name().to_string_lossy().to_string()
            } else {
                cmd_str
            },
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
            status: format!("{:?}", process.status()),
            affinity,
            sched_policy,
            priority,
        }
    }

    /// 更新进程信息
    pub fn update(&mut self, process: &Process, logical_cores: usize) {
        self.cpu_usage = process.cpu_usage();
        self.memory = process.memory();
        self.status = format!("{:?}", process.status());
        self.affinity = get_process_affinity(self.pid as i32, logical_cores);
        let (sched_policy, priority) = super::get_scheduler_info(self.pid as i32);
        self.sched_policy = sched_policy;
        self.priority = priority;
    }
}

/// 进程列表管理器
pub struct ProcessManager {
    /// 所有进程
    processes: Vec<ProcessInfo>,
    /// 逻辑核心数
    logical_cores: usize,
    /// 搜索过滤器
    filter: String,
    /// 排序字段
    sort_by: SortField,
    /// 排序方向
    sort_desc: bool,
}

/// 排序字段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Pid,
    Name,
    CpuUsage,
    Memory,
}

impl ProcessManager {
    pub fn new(logical_cores: usize) -> Self {
        Self {
            processes: Vec::new(),
            logical_cores,
            filter: String::new(),
            sort_by: SortField::CpuUsage,
            sort_desc: true,
        }
    }

    /// 更新进程列表
    pub fn update(&mut self, sys: &System) {
        let mut new_processes = Vec::new();

        for (pid, process) in sys.processes() {
            let pid_u32 = pid.as_u32();
            new_processes.push(ProcessInfo::from_process(pid_u32, process, self.logical_cores));
        }

        self.processes = new_processes;
        self.sort();
    }

    /// 获取过滤后的进程列表
    pub fn filtered_processes(&self) -> Vec<&ProcessInfo> {
        let filter_lower = self.filter.to_lowercase();
        self.processes
            .iter()
            .filter(|p| {
                if self.filter.is_empty() {
                    true
                } else {
                    p.name.to_lowercase().contains(&filter_lower)
                        || p.cmd.to_lowercase().contains(&filter_lower)
                        || p.pid.to_string().contains(&filter_lower)
                }
            })
            .collect()
    }

    /// 设置搜索过滤器
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// 获取当前过滤器
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// 设置排序
    pub fn set_sort(&mut self, field: SortField) {
        if self.sort_by == field {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_by = field;
            self.sort_desc = true;
        }
        self.sort();
    }

    /// 获取当前排序字段
    pub fn sort_field(&self) -> SortField {
        self.sort_by
    }

    /// 是否降序
    pub fn is_sort_desc(&self) -> bool {
        self.sort_desc
    }

    fn sort(&mut self) {
        match self.sort_by {
            SortField::Pid => {
                self.processes.sort_by_key(|p| p.pid);
            }
            SortField::Name => {
                self.processes.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortField::CpuUsage => {
                self.processes.sort_by(|a, b| {
                    a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortField::Memory => {
                self.processes.sort_by_key(|p| p.memory);
            }
        }
        if self.sort_desc {
            self.processes.reverse();
        }
    }
}

/// 获取进程的 CPU 亲和性 (Linux only)
#[cfg(target_os = "linux")]
pub fn get_process_affinity(pid: i32, logical_cores: usize) -> Vec<usize> {
    use libc::{cpu_set_t, sched_getaffinity, CPU_ISSET, CPU_SETSIZE};
    use std::mem::MaybeUninit;

    unsafe {
        let mut cpuset = MaybeUninit::<cpu_set_t>::zeroed();
        let result = sched_getaffinity(
            pid,
            std::mem::size_of::<cpu_set_t>(),
            cpuset.as_mut_ptr(),
        );

        if result == 0 {
            let cpuset = cpuset.assume_init();
            let mut affinity = Vec::new();
            for i in 0..logical_cores.min(CPU_SETSIZE as usize) {
                if CPU_ISSET(i, &cpuset) {
                    affinity.push(i);
                }
            }
            affinity
        } else {
            (0..logical_cores).collect()
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn get_process_affinity(_pid: i32, logical_cores: usize) -> Vec<usize> {
    (0..logical_cores).collect()
}

/// 设置进程的 CPU 亲和性 (Linux only)
#[cfg(target_os = "linux")]
pub fn set_process_affinity(pid: i32, cores: &[usize]) -> Result<(), String> {
    use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
    use std::mem::MaybeUninit;

    unsafe {
        let mut cpuset = MaybeUninit::<cpu_set_t>::zeroed().assume_init();
        CPU_ZERO(&mut cpuset);

        for &core in cores {
            CPU_SET(core, &mut cpuset);
        }

        let result = sched_setaffinity(pid, std::mem::size_of::<cpu_set_t>(), &cpuset);

        if result == 0 {
            Ok(())
        } else {
            let err = std::io::Error::last_os_error();
            Err(format!("设置亲和性失败: {} (可能需要 root 权限)", err))
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_process_affinity(_pid: i32, _cores: &[usize]) -> Result<(), String> {
    Err("CPU 亲和性设置仅支持 Linux".to_string())
}

/// 格式化内存大小
pub fn format_memory(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
