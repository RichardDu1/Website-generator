use std::collections::VecDeque;
use std::time::Instant;
use v3_core::{Asset, Position};
use tracing::{debug, info, warn};

/// Monitors Binance price at 100ms resolution
/// Calculates velocity and acceleration for momentum detection
pub struct BinanceMonitor {
    asset: Asset,
    price_history: VecDeque<PriceTick>,
    max_history: usize,
    velocity_threshold: f64,
    acceleration_threshold: f64,
    last_update: Option<Instant>,
    min_update_interval_ms: u64,
}

#[derive(Debug, Clone)]
struct PriceTick {
    timestamp: Instant,
    price: f64,
}

impl BinanceMonitor {
    pub fn new(asset: Asset, velocity_threshold: f64, acceleration_threshold: f64) -> Self {
        Self {
            asset,
            price_history: VecDeque::with_capacity(600),
            max_history: 600, // Keep last 60 seconds at 100ms resolution
            velocity_threshold,
            acceleration_threshold,
            last_update: None,
            min_update_interval_ms: 100, // Minimum 100ms between updates
        }
    }

    /// Update with new price tick (enforces minimum 100ms interval)
    pub fn update(&mut self, price: f64) {
        let now = Instant::now();

        // Check if enough time has passed since last update
        if let Some(last) = self.last_update {
            let elapsed = now.duration_since(last).as_millis() as u64;
            if elapsed < self.min_update_interval_ms {
                // Skip this update - too soon
                return;
            }
        }

        let tick = PriceTick {
            timestamp: now,
            price,
        };

        self.price_history.push_back(tick);
        self.last_update = Some(now);

        // Keep only recent history
        while self.price_history.len() > self.max_history {
            self.price_history.pop_front();
        }
    }

    /// Calculate price volatility (price range as % of current price)
    /// Returns the percentage range over the last 30 seconds
    /// This is more suitable for detecting momentum than velocity
    pub fn price_volatility(&self) -> f64 {
        if self.price_history.len() < 10 {
            return 0.0;
        }

        // Use last 30 seconds of data (300 ticks at 100ms resolution)
        let start_idx = if self.price_history.len() > 300 {
            self.price_history.len() - 300
        } else {
            0
        };

        let recent: Vec<PriceTick> = self.price_history.iter().skip(start_idx).cloned().collect();

        if recent.is_empty() {
            return 0.0;
        }

        // Find min and max price in the window
        let mut min_price = recent[0].price;
        let mut max_price = recent[0].price;
        let current_price = recent[recent.len() - 1].price;

        for tick in &recent {
            if tick.price < min_price {
                min_price = tick.price;
            }
            if tick.price > max_price {
                max_price = tick.price;
            }
        }

        // Calculate range as percentage of current price
        let range = max_price - min_price;
        let volatility = range / current_price;

        // Log volatility details
        info!(
            "📊 Volatility: {:.6} (range: ${:.2}, min: ${:.2}, max: ${:.2}, current: ${:.2}, window: {}s)",
            volatility, range, min_price, max_price, current_price,
            recent.len() as f64 * 0.1
        );

        volatility
    }

    /// Calculate price velocity (% per second, normalized)
    /// Returns percentage change per second to make threshold independent of asset price
    pub fn price_velocity(&self) -> f64 {
        if self.price_history.len() < 2 {
            debug!("Velocity=0: price_history too short (len={})", self.price_history.len());
            return 0.0;
        }

        // Use last 30 seconds of data (300 ticks at 100ms resolution)
        // This captures medium-term momentum while still being responsive
        let start_idx = if self.price_history.len() > 300 {
            self.price_history.len() - 300
        } else {
            0
        };

        let recent: Vec<PriceTick> = self.price_history.iter().skip(start_idx).cloned().collect();

        if recent.len() < 2 {
            debug!("Velocity=0: recent too short (len={})", recent.len());
            return 0.0;
        }

        let first = &recent[0];
        let last = &recent[recent.len() - 1];

        let time_diff = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if time_diff == 0.0 || first.price == 0.0 {
            info!("⚠️ Velocity=0: time_diff={:.6}s, first.price={:.2}", time_diff, first.price);
            return 0.0;
        }

        // Return percentage change per second (e.g., 0.01 = 1% per second)
        let price_change = last.price - first.price;
        let pct_change = price_change / first.price;
        let velocity = pct_change / time_diff;

        // Always log velocity calculation details for debugging
        info!(
            "📈 Velocity: {:.6} (price: {:.2} -> {:.2}, Δ={:.2}, time_diff: {:.1}s, window: {} ticks)",
            velocity, first.price, last.price, price_change, time_diff, recent.len()
        );

        velocity
    }

    /// Calculate price acceleration ($/second²)
    pub fn price_acceleration(&self) -> f64 {
        if self.price_history.len() < 20 {
            return 0.0;
        }

        let mid = self.price_history.len() / 2;
        let first_half: Vec<PriceTick> = self.price_history.iter().take(mid).cloned().collect();
        let second_half: Vec<PriceTick> = self.price_history.iter().skip(mid).cloned().collect();

        let vel1 = self.calc_velocity(&first_half);
        let vel2 = self.calc_velocity(&second_half);

        let time_diff = if let (Some(first), Some(last)) = (first_half.first(), second_half.last()) {
            last.timestamp.duration_since(first.timestamp).as_secs_f64()
        } else {
            return 0.0;
        };

        if time_diff == 0.0 {
            return 0.0;
        }

        (vel2 - vel1) / time_diff
    }

    fn calc_velocity(&self, ticks: &[PriceTick]) -> f64 {
        if ticks.len() < 2 {
            return 0.0;
        }

        let first = &ticks[0];
        let last = &ticks[ticks.len() - 1];

        let time_diff = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if time_diff == 0.0 || first.price == 0.0 {
            return 0.0;
        }

        // Return percentage change per second
        let price_change = last.price - first.price;
        let pct_change = price_change / first.price;
        pct_change / time_diff
    }

    /// Get current price
    pub fn current_price(&self) -> Option<f64> {
        self.price_history.back().map(|t| t.price)
    }

    /// Check if price movement is adverse to position
    pub fn is_adverse_move(&self, position: &Position) -> bool {
        let velocity = self.price_velocity();

        // For long positions, negative velocity is adverse
        // For short positions, positive velocity is adverse
        match position.side {
            v3_core::PositionSide::Long => {
                if velocity < -self.velocity_threshold {
                    warn!(
                        "Adverse move detected for LONG position: velocity={:.4}",
                        velocity
                    );
                    return true;
                }
            }
            v3_core::PositionSide::Short => {
                if velocity > self.velocity_threshold {
                    warn!(
                        "Adverse move detected for SHORT position: velocity={:.4}",
                        velocity
                    );
                    return true;
                }
            }
        }

        false
    }

    /// Check if strong momentum exists
    pub fn has_strong_momentum(&self) -> bool {
        let velocity = self.price_velocity().abs();
        let acceleration = self.price_acceleration().abs();

        velocity > self.velocity_threshold || acceleration > self.acceleration_threshold
    }

    /// Get momentum direction (-1, 0, 1)
    pub fn momentum_direction(&self) -> i8 {
        let velocity = self.price_velocity();

        if velocity > self.velocity_threshold {
            1 // Up
        } else if velocity < -self.velocity_threshold {
            -1 // Down
        } else {
            0 // Neutral
        }
    }

    /// Get price change over last N ticks
    pub fn price_change(&self, n: usize) -> Option<f64> {
        if self.price_history.len() < n {
            return None;
        }

        let start_idx = self.price_history.len() - n;
        let start_price = self.price_history[start_idx].price;
        let end_price = self.price_history.back()?.price;

        Some(end_price - start_price)
    }

    /// Get price change percentage
    pub fn price_change_pct(&self, n: usize) -> Option<f64> {
        let change = self.price_change(n)?;
        let start_idx = self.price_history.len() - n;
        let start_price = self.price_history[start_idx].price;

        if start_price == 0.0 {
            return None;
        }

        Some(change / start_price)
    }

    /// Returns true when Binance price velocity exceeds the cancel threshold,
    /// indicating adverse selection risk for resting market-making orders.
    /// MM orders should be cancelled immediately when this fires.
    pub fn cancel_signal(&self) -> bool {
        let velocity = self.price_velocity().abs();
        // Use 3x the velocity threshold as the cancel trigger
        velocity > self.velocity_threshold * 3.0
    }

    /// Clear history (useful for new market)
    pub fn clear(&mut self) {
        self.price_history.clear();
        debug!("Cleared Binance monitor history for {:?}", self.asset);
    }

    /// Get signal snapshot for strategy decision
    pub fn get_signal(&self) -> crate::signal_types::SignalSnapshot {
        use crate::signal_types::{SignalSnapshot, MomentumSignalType};

        let velocity = self.price_velocity();
        let volatility = self.price_volatility();
        let acceleration = self.price_acceleration();
        let price = self.current_price().unwrap_or(0.0);

        // Determine momentum signal based on volatility (price range) instead of velocity
        // volatility > 0.0003 means price moved more than 0.03% in the last 30 seconds
        let volatility_threshold = 0.0003; // 0.03% range in 30 seconds

        let momentum_signal = if volatility > volatility_threshold && velocity > 0.0 {
            // High volatility + positive direction = bullish momentum
            if acceleration > self.acceleration_threshold {
                MomentumSignalType::StrongUp
            } else {
                MomentumSignalType::WeakUp
            }
        } else if volatility > volatility_threshold && velocity < 0.0 {
            // High volatility + negative direction = bearish momentum
            if acceleration < -self.acceleration_threshold {
                MomentumSignalType::StrongDown
            } else {
                MomentumSignalType::WeakDown
            }
        } else {
            MomentumSignalType::Neutral
        };

        SignalSnapshot {
            market: v3_core::Market::default(),
            market_state: v3_core::MarketState {
                market: v3_core::Market::default(),
                best_bid_up: 0.48,
                best_ask_up: 0.52,
                best_bid_down: 0.48,
                best_ask_down: 0.52,
                spread_up: 0.04,
                spread_down: 0.04,
                mid_price_up: 0.50,
                mid_price_down: 0.50,
                volatility: volatility,
                binance_price: price,
                binance_velocity: velocity,
            },
            binance_price: price,
            binance_velocity: velocity,
            binance_acceleration: acceleration,
            momentum_signal,
            volatility: volatility,
            has_arbitrage: false,
            // These fields are populated by the orchestrator from BookCache
            // (binance_monitor has no access to the order book)
            order_book_imbalance: 0.0,
            bid_depth_3: 0.0,
            ask_depth_3: 0.0,
            spread_capture_margin: 0.0,
            binance_cancel_signal: self.cancel_signal(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_velocity() {
        let mut monitor = BinanceMonitor::new(Asset::BTC, 0.5, 0.1);

        // Simulate price increase
        for i in 0..20 {
            monitor.update(100.0 + i as f64);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let velocity = monitor.price_velocity();
        assert!(velocity > 0.0);
    }

    #[test]
    fn test_momentum_direction() {
        let mut monitor = BinanceMonitor::new(Asset::BTC, 0.5, 0.1);

        // Flat price
        for _ in 0..10 {
            monitor.update(100.0);
        }
        assert_eq!(monitor.momentum_direction(), 0);
    }
}
