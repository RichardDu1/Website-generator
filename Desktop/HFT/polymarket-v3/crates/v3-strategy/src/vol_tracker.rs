//! Rolling-window realized volatility tracker used by the Gengar strategy.
//!
//! Each `WindowKind` (5m / 15m / 1h) gets its own `VolTracker`.
//! We record the **closing** `btc_delta_pct` of each completed window and
//! return the sample stddev once the buffer reaches capacity (12 by default).
//! Until then we fall back to a static volatility supplied by the caller.

use std::collections::VecDeque;

pub struct VolTracker {
    capacity: usize,
    buffer: VecDeque<f64>,
}

impl VolTracker {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    /// Record the closing `btc_delta_pct` of a finished window.
    pub fn record_window_delta(&mut self, delta_pct: f64) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(delta_pct);
    }

    /// Return sample stddev when buffer is full, else `fallback`.
    pub fn realized_vol(&self, fallback: f64) -> f64 {
        if self.buffer.len() < self.capacity {
            return fallback;
        }
        let n = self.buffer.len() as f64;
        let mean: f64 = self.buffer.iter().sum::<f64>() / n;
        let var: f64 = self
            .buffer
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        var.sqrt()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    pub fn is_full(&self) -> bool {
        self.buffer.len() == self.capacity
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_default_when_empty() {
        let t = VolTracker::new(12);
        assert!((t.realized_vol(0.12) - 0.12).abs() < 1e-9);
    }

    #[test]
    fn falls_back_when_below_capacity() {
        let mut t = VolTracker::new(12);
        for _ in 0..5 {
            t.record_window_delta(0.1);
        }
        // buffer 未满 → 静态 fallback
        assert!((t.realized_vol(0.12) - 0.12).abs() < 1e-9);
        assert!(!t.is_full());
    }

    #[test]
    fn uses_stddev_when_buffer_full() {
        let mut t = VolTracker::new(3);
        t.record_window_delta(0.1);
        t.record_window_delta(-0.1);
        t.record_window_delta(0.2);
        assert!(t.is_full());
        let v = t.realized_vol(0.12);
        // sample stddev of [0.1,-0.1,0.2] ≈ 0.1528
        assert!(v > 0.14 && v < 0.17, "got {v}");
    }

    #[test]
    fn ring_buffer_drops_oldest() {
        let mut t = VolTracker::new(3);
        t.record_window_delta(100.0); // will be dropped
        t.record_window_delta(0.1);
        t.record_window_delta(-0.1);
        t.record_window_delta(0.2);
        let v = t.realized_vol(0.12);
        // 100.0 already dropped → stddev of [0.1,-0.1,0.2]
        assert!(v > 0.14 && v < 0.17, "got {v}");
    }
}
