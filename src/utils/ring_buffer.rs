//! 环形缓冲区 - 用于存储历史数据

use std::collections::VecDeque;

/// 固定大小的环形缓冲区
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    data: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// 创建指定容量的环形缓冲区
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// 添加元素，如果已满则移除最旧的
    pub fn push(&mut self, value: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    /// 获取所有数据的切片
    pub fn as_slice(&self) -> Vec<&T> {
        self.data.iter().collect()
    }

    /// 获取所有数据（克隆）
    pub fn to_vec(&self) -> Vec<T> {
        self.data.iter().cloned().collect()
    }

    /// 当前元素数量
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// 获取最新的值
    pub fn latest(&self) -> Option<&T> {
        self.data.back()
    }

    /// 获取最旧的值
    pub fn oldest(&self) -> Option<&T> {
        self.data.front()
    }

    /// 迭代器
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }
}

/// CPU 使用率历史记录
#[derive(Debug, Clone)]
pub struct CpuHistory {
    /// 每个核心的历史数据
    core_history: Vec<RingBuffer<f32>>,
    /// 总体使用率历史
    total_history: RingBuffer<f32>,
    /// 时间戳
    timestamps: RingBuffer<f64>,
}

impl CpuHistory {
    /// 创建新的历史记录
    /// - `core_count`: 核心数量
    /// - `history_size`: 历史记录长度（数据点数量）
    pub fn new(core_count: usize, history_size: usize) -> Self {
        let mut core_history = Vec::with_capacity(core_count);
        for _ in 0..core_count {
            core_history.push(RingBuffer::new(history_size));
        }

        Self {
            core_history,
            total_history: RingBuffer::new(history_size),
            timestamps: RingBuffer::new(history_size),
        }
    }

    /// 添加新的数据点
    pub fn push(&mut self, core_usages: &[f32], total_usage: f32, timestamp: f64) {
        for (i, &usage) in core_usages.iter().enumerate() {
            if i < self.core_history.len() {
                self.core_history[i].push(usage);
            }
        }
        self.total_history.push(total_usage);
        self.timestamps.push(timestamp);
    }

    /// 获取指定核心的历史数据
    pub fn core_history(&self, core_id: usize) -> Option<Vec<f32>> {
        self.core_history.get(core_id).map(|h| h.to_vec())
    }

    /// 获取总体使用率历史
    pub fn total_history(&self) -> Vec<f32> {
        self.total_history.to_vec()
    }

    /// 获取时间戳历史
    pub fn timestamps(&self) -> Vec<f64> {
        self.timestamps.to_vec()
    }

    /// 获取用于绘图的数据点（时间戳，使用率）
    pub fn plot_data(&self) -> Vec<[f64; 2]> {
        let times = self.timestamps.to_vec();
        let usages = self.total_history.to_vec();

        times
            .iter()
            .zip(usages.iter())
            .map(|(&t, &u)| [t, u as f64])
            .collect()
    }

    /// 获取指定核心用于绘图的数据点
    pub fn core_plot_data(&self, core_id: usize) -> Vec<[f64; 2]> {
        let times = self.timestamps.to_vec();
        if let Some(history) = self.core_history.get(core_id) {
            let usages = history.to_vec();
            times
                .iter()
                .zip(usages.iter())
                .map(|(&t, &u)| [t, u as f64])
                .collect()
        } else {
            vec![]
        }
    }

    /// 数据点数量
    pub fn len(&self) -> usize {
        self.total_history.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.total_history.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);

        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.to_vec(), vec![1, 2, 3]);

        buf.push(4);
        assert_eq!(buf.to_vec(), vec![2, 3, 4]);

        assert_eq!(buf.latest(), Some(&4));
        assert_eq!(buf.oldest(), Some(&2));
    }

    #[test]
    fn test_cpu_history() {
        let mut history = CpuHistory::new(2, 3);

        history.push(&[10.0, 20.0], 15.0, 1.0);
        history.push(&[30.0, 40.0], 35.0, 2.0);

        assert_eq!(history.len(), 2);
        assert_eq!(history.core_history(0), Some(vec![10.0, 30.0]));
        assert_eq!(history.total_history(), vec![15.0, 35.0]);
    }
}
