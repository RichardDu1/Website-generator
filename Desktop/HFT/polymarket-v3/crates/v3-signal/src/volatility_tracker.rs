use std::collections::VecDeque;
use v3_core::math::{std_dev, volatility};

/// Tracks volatility over time
pub struct VolatilityTracker {
    price_history: VecDeque<f64>,
    window_size: usize,
}

impl VolatilityTracker {
    pub fn new(window_size: usize) -> Self {
        Self {
            price_history: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    /// Update with new price
    pub fn update(&mut self, price: f64) {
        self.price_history.push_back(price);

        while self.price_history.len() > self.window_size {
            self.price_history.pop_front();
        }
    }

    /// Calculate current volatility
    pub fn current_volatility(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }

        let prices: Vec<f64> = self.price_history.iter().copied().collect();
        volatility(&prices)
    }

    /// Calculate standard deviation
    pub fn std_dev(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }

        let prices: Vec<f64> = self.price_history.iter().copied().collect();
        std_dev(&prices)
    }

    /// Check if volatility is high
    pub fn is_high_volatility(&self, threshold: f64) -> bool {
        self.current_volatility() > threshold
    }

    /// Check if volatility is low
    pub fn is_low_volatility(&self, threshold: f64) -> bool {
        self.current_volatility() < threshold
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.price_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volatility_tracking() {
        let mut tracker = VolatilityTracker::new(20);

        // Low volatility
        for _ in 0..20 {
            tracker.update(100.0);
        }
        assert!(tracker.current_volatility() < 0.01);

        // High volatility
        tracker.clear();
        for i in 0..20 {
            tracker.update(100.0 + (i % 2) as f64 * 10.0);
        }
        assert!(tracker.current_volatility() > 0.01);
    }
}
