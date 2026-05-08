use std::collections::VecDeque;
use v3_core::math::{sma, std_dev};

/// Detects momentum signals from price data
pub struct MomentumDetector {
    lookback_window: usize,
    momentum_threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MomentumSignal {
    StrongUp,
    WeakUp,
    Neutral,
    WeakDown,
    StrongDown,
}

impl MomentumDetector {
    pub fn new(lookback_window: usize, momentum_threshold: f64) -> Self {
        Self {
            lookback_window,
            momentum_threshold,
        }
    }

    /// Detect momentum from price history
    pub fn detect(&self, prices: &[f64]) -> MomentumSignal {
        if prices.len() < self.lookback_window {
            return MomentumSignal::Neutral;
        }

        let recent = &prices[prices.len() - self.lookback_window..];
        let momentum = self.calculate_momentum(recent);

        self.classify_momentum(momentum)
    }

    /// Calculate momentum score
    fn calculate_momentum(&self, prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }

        let first = prices[0];
        let last = prices[prices.len() - 1];

        if first == 0.0 {
            return 0.0;
        }

        // Percentage change
        (last - first) / first
    }

    /// Classify momentum into signal
    fn classify_momentum(&self, momentum: f64) -> MomentumSignal {
        let strong_threshold = self.momentum_threshold * 2.0;

        if momentum > strong_threshold {
            MomentumSignal::StrongUp
        } else if momentum > self.momentum_threshold {
            MomentumSignal::WeakUp
        } else if momentum < -strong_threshold {
            MomentumSignal::StrongDown
        } else if momentum < -self.momentum_threshold {
            MomentumSignal::WeakDown
        } else {
            MomentumSignal::Neutral
        }
    }

    /// Detect momentum with velocity
    pub fn detect_with_velocity(&self, prices: &[f64], velocity: f64) -> MomentumSignal {
        let price_momentum = self.detect(prices);

        // Combine price momentum with velocity
        match price_momentum {
            MomentumSignal::StrongUp if velocity > 0.0 => MomentumSignal::StrongUp,
            MomentumSignal::StrongDown if velocity < 0.0 => MomentumSignal::StrongDown,
            MomentumSignal::WeakUp if velocity > 0.0 => MomentumSignal::WeakUp,
            MomentumSignal::WeakDown if velocity < 0.0 => MomentumSignal::WeakDown,
            _ => MomentumSignal::Neutral,
        }
    }

    /// Check if momentum is strong enough to trade
    pub fn is_tradeable(&self, signal: MomentumSignal) -> bool {
        matches!(signal, MomentumSignal::StrongUp | MomentumSignal::StrongDown)
    }

    /// Get momentum strength (0-1)
    pub fn momentum_strength(&self, prices: &[f64]) -> f64 {
        if prices.len() < self.lookback_window {
            return 0.0;
        }

        let recent = &prices[prices.len() - self.lookback_window..];
        let momentum = self.calculate_momentum(recent);

        (momentum.abs() / (self.momentum_threshold * 2.0)).min(1.0)
    }
}

impl MomentumSignal {
    pub fn is_bullish(&self) -> bool {
        matches!(self, MomentumSignal::StrongUp | MomentumSignal::WeakUp)
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, MomentumSignal::StrongDown | MomentumSignal::WeakDown)
    }

    pub fn is_strong(&self) -> bool {
        matches!(self, MomentumSignal::StrongUp | MomentumSignal::StrongDown)
    }

    pub fn direction(&self) -> i8 {
        match self {
            MomentumSignal::StrongUp | MomentumSignal::WeakUp => 1,
            MomentumSignal::StrongDown | MomentumSignal::WeakDown => -1,
            MomentumSignal::Neutral => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_momentum_detection() {
        let detector = MomentumDetector::new(10, 0.01);

        // Strong uptrend
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let signal = detector.detect(&prices);
        assert!(signal.is_bullish());

        // Strong downtrend
        let prices: Vec<f64> = (0..20).map(|i| 100.0 - i as f64).collect();
        let signal = detector.detect(&prices);
        assert!(signal.is_bearish());

        // Flat
        let prices = vec![100.0; 20];
        let signal = detector.detect(&prices);
        assert_eq!(signal, MomentumSignal::Neutral);
    }

    #[test]
    fn test_momentum_strength() {
        let detector = MomentumDetector::new(10, 0.01);

        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64 * 2.0).collect();
        let strength = detector.momentum_strength(&prices);
        assert!(strength > 0.5);
    }
}
