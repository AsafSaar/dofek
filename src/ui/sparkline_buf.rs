use std::collections::VecDeque;

/// Ring buffer for sparkline history data.
/// Stores the last `capacity` samples as u64 values (ratatui Sparkline expects &[u64]).
pub struct SparklineBuf {
    data: VecDeque<u64>,
    cache: Vec<u64>,
    capacity: usize,
}

impl SparklineBuf {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            cache: Vec::with_capacity(capacity),
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
        // Rebuild contiguous cache
        self.cache.clear();
        self.cache.extend(self.data.iter().copied());
    }

    /// Get data as a contiguous slice for ratatui Sparkline. Zero-allocation.
    pub fn as_slice(&self) -> &[u64] {
        &self.cache
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_evicts_oldest_at_capacity() {
        let mut b = SparklineBuf::new(3);
        assert_eq!(b.as_slice(), &[] as &[u64]);

        b.push_raw(1);
        b.push_raw(2);
        b.push_raw(3);
        assert_eq!(b.as_slice(), &[1, 2, 3]);
        assert_eq!(b.len(), 3);

        // Fourth push drops the oldest, keeping insertion order.
        b.push_raw(4);
        assert_eq!(b.as_slice(), &[2, 3, 4]);
        assert_eq!(b.len(), 3, "length is capped at capacity");

        for v in 5..=100 {
            b.push_raw(v);
        }
        assert_eq!(b.as_slice(), &[98, 99, 100]);
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn sparkline_clamps_percentages_into_range() {
        let mut b = SparklineBuf::new(8);
        for v in [-5.0, 0.0, 42.7, 99.9, 100.0, 250.0, f32::NAN] {
            b.push_percent(v);
        }
        // Out-of-range values clamp; the fractional part truncates toward zero.
        // NaN clamps to the low bound (`f32::clamp` returns the min for NaN).
        assert_eq!(b.as_slice(), &[0, 0, 42, 99, 100, 100, 0]);
    }

    #[test]
    fn sparkline_with_capacity_one_keeps_only_the_latest() {
        let mut b = SparklineBuf::new(1);
        b.push_raw(7);
        b.push_raw(8);
        assert_eq!(b.as_slice(), &[8]);
    }

    #[test]
    fn candles_only_emit_once_the_accumulator_fills() {
        let mut b = CandleBuf::new(4, 4);
        for v in [10.0, 20.0, 30.0] {
            b.push(v);
        }
        assert_eq!(b.len(), 0, "a partial window produces no candle");

        b.push(40.0);
        assert_eq!(b.len(), 1);

        let c = b.as_slice()[0];
        assert_eq!(c.mean, 25.0);
        assert_eq!(c.min, 10.0);
        assert_eq!(c.max, 40.0);
        // Quartiles index the sorted window at n/4 and 3n/4.
        assert_eq!(c.p25, 20.0);
        assert_eq!(c.p75, 40.0);
    }

    #[test]
    fn candles_sort_the_window_before_aggregating() {
        let mut b = CandleBuf::new(4, 4);
        // Same values as above, pushed out of order.
        for v in [40.0, 10.0, 30.0, 20.0] {
            b.push(v);
        }
        let c = b.as_slice()[0];
        assert_eq!((c.mean, c.min, c.max), (25.0, 10.0, 40.0));
    }

    #[test]
    fn candle_ring_evicts_oldest_at_capacity() {
        let mut b = CandleBuf::new(2, 1);
        b.push(1.0);
        b.push(2.0);
        b.push(3.0);
        assert_eq!(b.len(), 2);
        // Single-sample candles carry a synthetic spread, so compare the mean.
        let means: Vec<f32> = b.as_slice().iter().map(|c| c.mean).collect();
        assert_eq!(means, vec![2.0, 3.0]);
    }

    /// A one-sample window has no real spread, so the widget fabricates one
    /// rather than drawing a flat line. It must stay inside 0..=100.
    #[test]
    fn single_sample_candles_get_a_bounded_synthetic_spread() {
        for v in [0.0, 1.0, 50.0, 99.0, 100.0] {
            let mut b = CandleBuf::new(1, 1);
            b.push(v);
            let c = b.as_slice()[0];
            assert_eq!(c.mean, v);
            assert!(c.min >= 0.0, "min {} below 0 for {v}", c.min);
            assert!(c.max <= 100.0, "max {} above 100 for {v}", c.max);
            assert!(c.min <= c.p25 && c.p25 <= c.mean, "p25 out of order for {v}");
            assert!(c.mean <= c.p75 && c.p75 <= c.max, "p75 out of order for {v}");
        }
    }

    /// `samples_per_candle: 0` would divide the window by zero / emit a candle
    /// per push forever; the constructor floors it at 1.
    #[test]
    fn zero_samples_per_candle_is_floored_to_one() {
        let mut b = CandleBuf::new(4, 0);
        b.push(5.0);
        assert_eq!(b.len(), 1);
        assert_eq!(b.as_slice()[0].mean, 5.0);
    }
}
