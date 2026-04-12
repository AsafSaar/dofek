use std::collections::VecDeque;

/// Ring buffer for sparkline history data.
/// Stores the last `capacity` samples as u64 values (ratatui Sparkline expects &[u64]).
pub struct SparklineBuf {
    data: VecDeque<u64>,
    capacity: usize,
}

impl SparklineBuf {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a floating-point value, scaled to 0..100 range.
    pub fn push_percent(&mut self, value: f32) {
        let clamped = value.clamp(0.0, 100.0) as u64;
        self.push_raw(clamped);
    }

    /// Push a raw u64 value.
    pub fn push_raw(&mut self, value: u64) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    /// Get data as a slice for ratatui Sparkline.
    pub fn as_slice(&self) -> Vec<u64> {
        self.data.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}
