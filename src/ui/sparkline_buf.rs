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

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

// --- Candlestick data for CPU chart ---

/// A single candlestick sample aggregated from multiple data ticks.
#[derive(Debug, Clone, Copy, Default)]
pub struct CandleSample {
    pub mean: f32,
    pub min: f32,
    pub max: f32,
    pub p25: f32,
    pub p75: f32,
}

/// Ring buffer that accumulates raw values and produces CandleSamples.
/// Each candle represents `samples_per_candle` data ticks aggregated together.
pub struct CandleBuf {
    data: VecDeque<CandleSample>,
    accumulator: Vec<f32>,
    capacity: usize,
    samples_per_candle: usize,
}

impl CandleBuf {
    pub fn new(capacity: usize, samples_per_candle: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            accumulator: Vec::with_capacity(samples_per_candle),
            capacity,
            samples_per_candle: samples_per_candle.max(1),
        }
    }

    /// Push a raw value into the accumulator. When enough samples are collected,
    /// a CandleSample is computed and pushed into the ring buffer.
    pub fn push(&mut self, value: f32) {
        self.accumulator.push(value);

        if self.accumulator.len() >= self.samples_per_candle {
            let sample = self.compute_candle();
            if self.data.len() >= self.capacity {
                self.data.pop_front();
            }
            self.data.push_back(sample);
            self.accumulator.clear();
        }
    }

    fn compute_candle(&self) -> CandleSample {
        let n = self.accumulator.len();
        if n == 0 {
            return CandleSample::default();
        }

        let mut sorted = self.accumulator.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mean = sorted.iter().sum::<f32>() / n as f32;

        if n == 1 {
            // Synthetic spread for single-sample candles (matches GUI behavior)
            let v = (mean * 0.3 + 2.0).abs();
            let min = (mean - v * 0.7).max(0.0);
            let max = (mean + v * 0.6).min(100.0);
            let p25 = min + (mean - min) * 0.4;
            let p75 = mean + (max - mean) * 0.4;
            return CandleSample { mean, min, max, p25, p75 };
        }

        let min = sorted[0];
        let max = sorted[n - 1];
        let p25 = sorted[n / 4];
        let p75 = sorted[(n * 3) / 4];

        CandleSample { mean, min, max, p25, p75 }
    }

    /// Get all candle samples as a slice.
    pub fn as_slice(&self) -> &VecDeque<CandleSample> {
        &self.data
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}
