//! CPU 拓扑和信息检测模块
//! 支持自动检测 AMD/Intel CPU 的核心拓扑、缓存信息等

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use sysinfo::System;

/// CPU 核心类型（用于 Intel 混合架构）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreType {
    /// 性能核心 (Intel P-Core 或 AMD 标准核心)
    Performance,
    /// 效率核心 (Intel E-Core)
    Efficiency,
    /// 未知类型
    Unknown,
}

/// L3 缓存信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L3CacheInfo {
    /// 缓存 ID
    pub id: u32,
    /// 缓存大小 (KB)
    pub size_kb: u64,
    /// 共享此缓存的 CPU 列表
    pub shared_cpus: Vec<usize>,
    /// 是否为 3D V-Cache（大于 64MB 的 L3）
    pub is_vcache: bool,
}

/// 单个 CPU 核心的拓扑信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCore {
    /// 逻辑 CPU ID
    pub cpu_id: usize,
    /// 物理核心 ID
    pub core_id: usize,
    /// 物理封装 ID（多路 CPU）
    pub package_id: usize,
    /// NUMA 节点
    pub numa_node: usize,
    /// 核心类型
    pub core_type: CoreType,
    /// 所属 CCD/CCX ID（AMD）或核心集群（Intel）
    pub cluster_id: Option<usize>,
    /// 关联的 L3 缓存 ID
    pub l3_cache_id: Option<u32>,
    /// 当前频率 (MHz)
    pub frequency_mhz: u64,
    /// 当前使用率 (0.0 - 100.0)
    pub usage_percent: f32,
}

/// CPU 总体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU 型号名称
    pub model_name: String,
    /// CPU 厂商
    pub vendor: CpuVendor,
    /// 物理核心数
    pub physical_cores: usize,
    /// 逻辑核心数（线程数）
    pub logical_cores: usize,
    /// 是否启用 SMT/HT
    pub smt_enabled: bool,
    /// 每个核心的详细信息
    pub cores: Vec<CpuCore>,
    /// L3 缓存信息
    pub l3_caches: Vec<L3CacheInfo>,
    /// 基础频率 (MHz)
    pub base_frequency_mhz: u64,
    /// 最大频率 (MHz)
    pub max_frequency_mhz: u64,
    /// 总体使用率
    pub total_usage_percent: f32,
}

/// CPU 厂商
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuVendor {
    AMD,
    Intel,
    Other,
}

impl CpuInfo {
    /// 检测并创建 CPU 信息
    pub fn detect() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();

        let model_name = System::cpu_arch()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // 从 /proc/cpuinfo 获取详细信息
        let cpuinfo = read_cpuinfo();
        let vendor = detect_vendor(&cpuinfo);
        let model = cpuinfo.get("model name")
            .cloned()
            .unwrap_or_else(|| model_name.clone());

        let logical_cores = sys.cpus().len();
        let physical_cores = detect_physical_cores(logical_cores);

        // 检测每个核心的拓扑
        let mut cores = Vec::with_capacity(logical_cores);
        for cpu_id in 0..logical_cores {
            cores.push(detect_core_topology(cpu_id, vendor));
        }

        // 检测 L3 缓存
        let l3_caches = detect_l3_caches(logical_cores);

        // 关联核心和 L3 缓存
        for core in &mut cores {
            for cache in &l3_caches {
                if cache.shared_cpus.contains(&core.cpu_id) {
                    core.l3_cache_id = Some(cache.id);
                    break;
                }
            }
        }

        // 检测频率范围
        let (base_freq, max_freq) = detect_frequency_range();

        CpuInfo {
            model_name: model,
            vendor,
            physical_cores,
            logical_cores,
            smt_enabled: logical_cores > physical_cores,
            cores,
            l3_caches,
            base_frequency_mhz: base_freq,
            max_frequency_mhz: max_freq,
            total_usage_percent: 0.0,
        }
    }

    /// 更新 CPU 使用率和频率
    pub fn update(&mut self, sys: &System) {
        let cpus = sys.cpus();
        let mut total_usage = 0.0;

        for (i, cpu) in cpus.iter().enumerate() {
            if i < self.cores.len() {
                self.cores[i].usage_percent = cpu.cpu_usage();
                self.cores[i].frequency_mhz = cpu.frequency();
                total_usage += cpu.cpu_usage();
            }
        }

        self.total_usage_percent = if !cpus.is_empty() {
            total_usage / cpus.len() as f32
        } else {
            0.0
        };
    }

    /// 计算适合显示的网格布局（列数）
    pub fn grid_columns(&self) -> usize {
        match self.logical_cores {
            1..=4 => 2,
            5..=8 => 4,
            9..=16 => 4,
            17..=32 => 8,
            33..=64 => 8,
            _ => 16,
        }
    }

    /// 获取按 L3 缓存分组的核心
    pub fn cores_by_l3(&self) -> HashMap<u32, Vec<&CpuCore>> {
        let mut groups: HashMap<u32, Vec<&CpuCore>> = HashMap::new();
        for core in &self.cores {
            if let Some(l3_id) = core.l3_cache_id {
                groups.entry(l3_id).or_default().push(core);
            }
        }
        groups
    }

    /// 获取 3D V-Cache 核心列表
    pub fn vcache_cores(&self) -> Vec<usize> {
        let vcache_ids: Vec<u32> = self.l3_caches
            .iter()
            .filter(|c| c.is_vcache)
            .map(|c| c.id)
            .collect();

        self.cores
            .iter()
            .filter(|c| c.l3_cache_id.map(|id| vcache_ids.contains(&id)).unwrap_or(false))
            .map(|c| c.cpu_id)
            .collect()
    }
}

/// 读取 /proc/cpuinfo
fn read_cpuinfo() -> HashMap<String, String> {
    let mut info = HashMap::new();
    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                info.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    info
}

/// 检测 CPU 厂商
fn detect_vendor(cpuinfo: &HashMap<String, String>) -> CpuVendor {
    if let Some(vendor) = cpuinfo.get("vendor_id") {
        if vendor.contains("AMD") {
            return CpuVendor::AMD;
        } else if vendor.contains("Intel") {
            return CpuVendor::Intel;
        }
    }
    CpuVendor::Other
}

/// 检测物理核心数
fn detect_physical_cores(logical_cores: usize) -> usize {
    // 尝试从 sysfs 读取
    let path = "/sys/devices/system/cpu/cpu0/topology/core_siblings_list";
    if let Ok(content) = fs::read_to_string(path) {
        // 计算兄弟线程数量
        if let Some(count) = parse_cpu_list(&content).map(|list| list.len()) {
            if count > 0 {
                return logical_cores / (logical_cores / count).max(1);
            }
        }
    }
    // 回退：假设启用了 SMT，每个物理核心有 2 个线程
    logical_cores / 2
}

/// 检测单个核心的拓扑信息
fn detect_core_topology(cpu_id: usize, vendor: CpuVendor) -> CpuCore {
    let base_path = format!("/sys/devices/system/cpu/cpu{}/topology", cpu_id);

    let core_id = read_sysfs_value(&format!("{}/core_id", base_path)).unwrap_or(cpu_id);
    let package_id = read_sysfs_value(&format!("{}/physical_package_id", base_path)).unwrap_or(0);

    // NUMA 节点
    let numa_node = detect_numa_node(cpu_id);

    // 核心类型检测（主要针对 Intel 混合架构）
    let core_type = if vendor == CpuVendor::Intel {
        detect_intel_core_type(cpu_id)
    } else {
        CoreType::Performance
    };

    // AMD CCD/CCX 检测
    let cluster_id = if vendor == CpuVendor::AMD {
        detect_amd_cluster(cpu_id)
    } else {
        None
    };

    CpuCore {
        cpu_id,
        core_id,
        package_id,
        numa_node,
        core_type,
        cluster_id,
        l3_cache_id: None, // 稍后填充
        frequency_mhz: 0,
        usage_percent: 0.0,
    }
}

/// 检测 NUMA 节点
fn detect_numa_node(cpu_id: usize) -> usize {
    let numa_path = "/sys/devices/system/node";
    if let Ok(entries) = fs::read_dir(numa_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("node") {
                if let Ok(node_id) = name_str[4..].parse::<usize>() {
                    let cpulist_path = format!("{}/node{}/cpulist", numa_path, node_id);
                    if let Ok(content) = fs::read_to_string(&cpulist_path) {
                        if let Some(cpus) = parse_cpu_list(&content) {
                            if cpus.contains(&cpu_id) {
                                return node_id;
                            }
                        }
                    }
                }
            }
        }
    }
    0
}

/// 检测 Intel 核心类型（P-Core vs E-Core）
fn detect_intel_core_type(cpu_id: usize) -> CoreType {
    // Intel 混合架构通过 cpuid 或 sysfs 检测
    // 简化实现：检查是否有不同的 L2 缓存大小
    let cache_path = format!("/sys/devices/system/cpu/cpu{}/cache/index2/size", cpu_id);
    if let Ok(content) = fs::read_to_string(&cache_path) {
        let size = parse_cache_size(&content);
        // E-Core 通常有较小的 L2 缓存 (2MB vs 1.25MB)
        if size < 1500 {
            return CoreType::Efficiency;
        }
    }
    CoreType::Performance
}

/// 检测 AMD CCD/CCX
fn detect_amd_cluster(cpu_id: usize) -> Option<usize> {
    // AMD 使用 L3 缓存共享来识别 CCD
    let cache_path = format!("/sys/devices/system/cpu/cpu{}/cache/index3/id", cpu_id);
    read_sysfs_value(&cache_path)
}

/// 检测 L3 缓存信息
fn detect_l3_caches(logical_cores: usize) -> Vec<L3CacheInfo> {
    let mut caches: HashMap<u32, L3CacheInfo> = HashMap::new();

    for cpu_id in 0..logical_cores {
        let base_path = format!("/sys/devices/system/cpu/cpu{}/cache/index3", cpu_id);
        if !Path::new(&base_path).exists() {
            continue;
        }

        let id = read_sysfs_value(&format!("{}/id", base_path)).unwrap_or(0);

        if !caches.contains_key(&id) {
            let size_str = fs::read_to_string(format!("{}/size", base_path))
                .unwrap_or_default();
            let size_kb = parse_cache_size(&size_str);

            let shared_str = fs::read_to_string(format!("{}/shared_cpu_list", base_path))
                .unwrap_or_default();
            let shared_cpus = parse_cpu_list(&shared_str).unwrap_or_default();

            // 3D V-Cache 检测：L3 > 64MB (65536 KB)
            let is_vcache = size_kb > 65536;

            caches.insert(id, L3CacheInfo {
                id,
                size_kb,
                shared_cpus,
                is_vcache,
            });
        }
    }

    let mut result: Vec<L3CacheInfo> = caches.into_values().collect();
    result.sort_by_key(|c| c.id);
    result
}

/// 检测频率范围
fn detect_frequency_range() -> (u64, u64) {
    let base = read_sysfs_value("/sys/devices/system/cpu/cpu0/cpufreq/base_frequency")
        .or_else(|| read_sysfs_value("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq"))
        .map(|f: u64| f / 1000) // KHz -> MHz
        .unwrap_or(0);

    let max = read_sysfs_value("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
        .map(|f: u64| f / 1000)
        .unwrap_or(0);

    (base, max)
}

/// 读取 sysfs 数值
fn read_sysfs_value<T: std::str::FromStr>(path: &str) -> Option<T> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// 解析 CPU 列表字符串 (如 "0-7,16-23")
fn parse_cpu_list(s: &str) -> Option<Vec<usize>> {
    let mut result = Vec::new();
    for part in s.trim().split(',') {
        let part = part.trim();
        if part.contains('-') {
            let mut range = part.split('-');
            let start: usize = range.next()?.parse().ok()?;
            let end: usize = range.next()?.parse().ok()?;
            for i in start..=end {
                result.push(i);
            }
        } else if !part.is_empty() {
            result.push(part.parse().ok()?);
        }
    }
    Some(result)
}

/// 解析缓存大小字符串 (如 "32768K" 或 "32M")
fn parse_cache_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    if let Some(kb) = s.strip_suffix('K') {
        kb.parse().unwrap_or(0)
    } else if let Some(mb) = s.strip_suffix('M') {
        mb.parse::<u64>().unwrap_or(0) * 1024
    } else {
        s.parse().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_list() {
        assert_eq!(parse_cpu_list("0-3"), Some(vec![0, 1, 2, 3]));
        assert_eq!(parse_cpu_list("0,2,4"), Some(vec![0, 2, 4]));
        assert_eq!(parse_cpu_list("0-1,4-5"), Some(vec![0, 1, 4, 5]));
    }

    #[test]
    fn test_parse_cache_size() {
        assert_eq!(parse_cache_size("32768K"), 32768);
        assert_eq!(parse_cache_size("32M"), 32768);
        assert_eq!(parse_cache_size("96M"), 98304);
    }
}
