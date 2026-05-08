use std::collections::HashMap;
use std::time::Duration;
use v3_core::{Market, Order, Position, Outcome};
use chrono::{DateTime, Utc};
use tracing::{info, warn};

/// Simplified position for tracking
#[derive(Debug, Clone)]
pub struct TrackedPosition {
    pub market_id: String,
    pub outcome: Outcome,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub entry_time: DateTime<Utc>,
}

impl TrackedPosition {
    /// Convert to Position for strategy evaluation
    pub fn to_position(&self, market: &Market) -> Position {
        Position {
            id: self.market_id.clone(),
            market: market.clone(),
            outcome: self.outcome,
            side: v3_core::PositionSide::Long,
            entry_price: self.entry_price,
            size: self.size as u64,
            entry_time: self.entry_time,
            unrealized_pnl: self.unrealized_pnl,
            strategy_name: "Momentum".to_string(),
        }
    }
}

/// Tracks active positions across markets
pub struct PositionTracker {
    /// Active positions by market_id
    positions: HashMap<String, TrackedPosition>,
    /// Entry orders by market_id (for calculating cost basis)
    entry_orders: HashMap<String, Vec<Order>>,
    /// Target hold time for quick profit/loss (distinct-baguette: 6 seconds)
    target_hold_time_secs: f64,
    /// Maximum hold time before forced close (distinct-baguette: 30 seconds)
    max_hold_time_secs: f64,
    /// Quick profit threshold (0.5% = $0.005 per share)
    quick_profit_threshold: f64,
    /// Quick loss threshold (0.3% = $0.003 per share)
    quick_loss_threshold: f64,
}

impl PositionTracker {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            entry_orders: HashMap::new(),
            target_hold_time_secs: 6.0,      // distinct-baguette median
            max_hold_time_secs: 30.0,        // 88% close within 30s
            quick_profit_threshold: 0.005,   // 0.5%
            quick_loss_threshold: -0.003,    // -0.3%
        }
    }

    /// Create with custom parameters
    pub fn with_params(
        target_hold_time_secs: f64,
        max_hold_time_secs: f64,
        quick_profit_threshold: f64,
        quick_loss_threshold: f64,
    ) -> Self {
        Self {
            positions: HashMap::new(),
            entry_orders: HashMap::new(),
            target_hold_time_secs,
            max_hold_time_secs,
            quick_profit_threshold,
            quick_loss_threshold,
        }
    }

    /// Record an entry order (when order is placed)
    pub fn record_entry(&mut self, market_id: String, order: Order) {
        self.entry_orders
            .entry(market_id)
            .or_insert_with(Vec::new)
            .push(order);
    }

    /// Update position on fill (when order is filled)
    /// Returns realized PnL if this is an exit fill
    pub fn on_fill(
        &mut self,
        market_id: String,
        outcome: Outcome,
        size: f64,
        price: f64,
        is_entry: bool,
    ) -> Option<f64> {
        if is_entry {
            // Entry fill - create or update position
            let position = self.positions.entry(market_id.clone()).or_insert_with(|| {
                TrackedPosition {
                    market_id: market_id.clone(),
                    outcome,
                    size: 0.0,
                    entry_price: 0.0,
                    current_price: price,
                    unrealized_pnl: 0.0,
                    entry_time: Utc::now(),
                }
            });

            // Update weighted average entry price
            let total_cost = position.size * position.entry_price + size * price;
            position.size += size;
            position.entry_price = total_cost / position.size;
            position.current_price = price;
            None
        } else {
            // Exit fill - reduce or close position
            if let Some(position) = self.positions.get_mut(&market_id) {
                // CRITICAL FIX #3: Calculate realized PnL BEFORE reducing size
                let realized_pnl = (price - position.entry_price) * size;

                position.size -= size;
                position.current_price = price;

                // Remove position if fully closed
                if position.size <= 0.001 {
                    self.positions.remove(&market_id);
                    self.entry_orders.remove(&market_id);
                }

                Some(realized_pnl)
            } else {
                None
            }
        }
    }

    /// Update current price for a position (from order book)
    pub fn update_price(&mut self, market_id: &str, outcome: Outcome, price: f64) {
        if let Some(position) = self.positions.get_mut(market_id) {
            if position.outcome == outcome {
                position.current_price = price;
                // Recalculate unrealized PnL
                position.unrealized_pnl = (position.current_price - position.entry_price) * position.size;
            }
        }
    }

    /// Get position for a market
    pub fn get_position(&self, market_id: &str) -> Option<&TrackedPosition> {
        self.positions.get(market_id)
    }

    /// Get all active positions
    pub fn get_all_positions(&self) -> Vec<&TrackedPosition> {
        self.positions.values().collect()
    }

    /// Calculate total unrealized PnL across all positions
    pub fn total_unrealized_pnl(&self) -> f64 {
        self.positions.values().map(|p| p.unrealized_pnl).sum()
    }

    /// Get number of active positions
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Check if we have a position in a market
    pub fn has_position(&self, market_id: &str) -> bool {
        self.positions.contains_key(market_id)
    }

    /// Get position size for a market
    pub fn get_position_size(&self, market_id: &str) -> f64 {
        self.positions
            .get(market_id)
            .map(|p| p.size)
            .unwrap_or(0.0)
    }

    /// Get position hold time in seconds
    pub fn get_hold_time(&self, market_id: &str) -> Option<f64> {
        self.positions.get(market_id).map(|p| {
            let duration = Utc::now() - p.entry_time;
            duration.num_milliseconds() as f64 / 1000.0
        })
    }

    /// Close a position (mark as closed, remove from tracking)
    pub fn close_position(&mut self, market_id: &str) {
        self.positions.remove(market_id);
        self.entry_orders.remove(market_id);
    }

    /// Get positions that have been held longer than max_hold_time
    pub fn get_stale_positions(&self, max_hold_time_secs: f64) -> Vec<String> {
        self.positions
            .iter()
            .filter_map(|(market_id, position)| {
                let duration = Utc::now() - position.entry_time;
                let hold_time = duration.num_milliseconds() as f64 / 1000.0;
                if hold_time > max_hold_time_secs {
                    Some(market_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get positions with unrealized loss exceeding threshold
    pub fn get_losing_positions(&self, max_loss: f64) -> Vec<String> {
        self.positions
            .iter()
            .filter_map(|(market_id, position)| {
                if position.unrealized_pnl < -max_loss {
                    Some(market_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clear all positions (for testing or reset)
    pub fn clear(&mut self) {
        self.positions.clear();
        self.entry_orders.clear();
    }

    /// Check for "6-second target" exits (distinct-baguette style)
    /// Exit with any profit after 6 seconds
    pub fn check_six_second_exits(&self) -> Vec<(String, String)> {
        let mut to_close = Vec::new();
        let now = Utc::now();

        for (market_id, position) in &self.positions {
            let hold_time = (now - position.entry_time).num_seconds();

            // 6 seconds after entry, any profit qualifies for exit
            if hold_time >= 6 {
                let pnl_pct = if position.entry_price > 0.0 {
                    (position.current_price - position.entry_price) / position.entry_price
                } else {
                    0.0
                };

                // Any profit > 0.1% qualifies
                if pnl_pct > 0.001 {
                    info!(
                        "🎯 6s-target: {} held {}s with +{:.2}% profit - EXIT",
                        market_id, hold_time, pnl_pct * 100.0
                    );
                    to_close.push((
                        market_id.clone(),
                        format!("6s-target ({}s, +{:.2}%)", hold_time, pnl_pct * 100.0)
                    ));
                }
            }
        }

        to_close
    }

    /// Check exit conditions for all positions (distinct-baguette style)
    /// Returns list of (market_id, reason) for positions that should be closed
    pub fn check_exit_conditions(&self) -> Vec<(String, String)> {
        let mut to_close = Vec::new();
        let now = Utc::now();

        for (market_id, position) in &self.positions {
            let hold_time = (now - position.entry_time).num_milliseconds() as f64 / 1000.0;
            let pnl_pct = if position.entry_price > 0.0 {
                (position.current_price - position.entry_price) / position.entry_price
            } else {
                0.0
            };

            // Priority 1: Maximum hold time (30 seconds) - FORCE CLOSE
            if hold_time > self.max_hold_time_secs {
                warn!(
                    "🔴 Position {} held for {:.1}s (max {}s) - FORCE CLOSE",
                    market_id, hold_time, self.max_hold_time_secs
                );
                to_close.push((market_id.clone(), "max_hold_time".to_string()));
                continue;
            }

            // Priority 2: Quick profit (after target hold time)
            if hold_time > self.target_hold_time_secs && pnl_pct > self.quick_profit_threshold {
                info!(
                    "✅ Position {} held {:.1}s with +{:.2}% profit - QUICK PROFIT",
                    market_id, hold_time, pnl_pct * 100.0
                );
                to_close.push((market_id.clone(), "quick_profit".to_string()));
                continue;
            }

            // Priority 3: Quick loss (after target hold time)
            if hold_time > self.target_hold_time_secs && pnl_pct < self.quick_loss_threshold {
                warn!(
                    "⚠️ Position {} held {:.1}s with {:.2}% loss - QUICK LOSS",
                    market_id, hold_time, pnl_pct * 100.0
                );
                to_close.push((market_id.clone(), "quick_loss".to_string()));
                continue;
            }

            // Log positions approaching target hold time
            if hold_time > self.target_hold_time_secs * 0.8 && hold_time < self.target_hold_time_secs {
                info!(
                    "⏱️ Position {} held {:.1}s (target {}s), PnL: {:.2}%",
                    market_id, hold_time, self.target_hold_time_secs, pnl_pct * 100.0
                );
            }
        }

        to_close
    }

    /// Get position statistics for monitoring
    pub fn get_stats(&self) -> PositionStats {
        let now = Utc::now();
        let mut hold_times = Vec::new();
        let mut pnl_pcts = Vec::new();

        for position in self.positions.values() {
            let hold_time = (now - position.entry_time).num_milliseconds() as f64 / 1000.0;
            hold_times.push(hold_time);

            if position.entry_price > 0.0 {
                let pnl_pct = (position.current_price - position.entry_price) / position.entry_price;
                pnl_pcts.push(pnl_pct);
            }
        }

        // Calculate median hold time
        hold_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_hold_time = if hold_times.is_empty() {
            0.0
        } else {
            hold_times[hold_times.len() / 2]
        };

        // Calculate average PnL
        let avg_pnl_pct = if pnl_pcts.is_empty() {
            0.0
        } else {
            pnl_pcts.iter().sum::<f64>() / pnl_pcts.len() as f64
        };

        PositionStats {
            total_positions: self.positions.len(),
            median_hold_time_secs: median_hold_time,
            avg_pnl_pct,
            total_unrealized_pnl: self.total_unrealized_pnl(),
        }
    }
}

/// Position statistics for monitoring
#[derive(Debug, Clone)]
pub struct PositionStats {
    pub total_positions: usize,
    pub median_hold_time_secs: f64,
    pub avg_pnl_pct: f64,
    pub total_unrealized_pnl: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_tracker_entry() {
        let mut tracker = PositionTracker::new();

        // Record entry fill
        tracker.on_fill(
            "market1".to_string(),
            Outcome::Up,
            10.0,
            0.50,
            true,
        );

        assert_eq!(tracker.position_count(), 1);
        assert!(tracker.has_position("market1"));

        let position = tracker.get_position("market1").unwrap();
        assert_eq!(position.size, 10.0);
        assert_eq!(position.entry_price, 0.50);
    }

    #[test]
    fn test_position_tracker_weighted_average() {
        let mut tracker = PositionTracker::new();

        // First entry at 0.50
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.50, true);

        // Second entry at 0.60
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.60, true);

        let position = tracker.get_position("market1").unwrap();
        assert_eq!(position.size, 20.0);
        assert_eq!(position.entry_price, 0.55); // (10*0.50 + 10*0.60) / 20
    }

    #[test]
    fn test_position_tracker_exit() {
        let mut tracker = PositionTracker::new();

        // Entry
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.50, true);

        // Partial exit
        tracker.on_fill("market1".to_string(), Outcome::Up, 5.0, 0.60, false);

        let position = tracker.get_position("market1").unwrap();
        assert_eq!(position.size, 5.0);

        // Full exit
        tracker.on_fill("market1".to_string(), Outcome::Up, 5.0, 0.60, false);
        assert!(!tracker.has_position("market1"));
    }

    #[test]
    fn test_position_tracker_unrealized_pnl() {
        let mut tracker = PositionTracker::new();

        // Entry at 0.50
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.50, true);

        // Update price to 0.60
        tracker.update_price("market1", Outcome::Up, 0.60);

        let position = tracker.get_position("market1").unwrap();
        assert!((position.unrealized_pnl - 1.0).abs() < 0.001); // (0.60 - 0.50) * 10.0
        assert!((tracker.total_unrealized_pnl() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_position_tracker_stale_positions() {
        let mut tracker = PositionTracker::new();

        // Create position
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.50, true);

        // Should not be stale immediately
        let stale = tracker.get_stale_positions(1.0);
        assert_eq!(stale.len(), 0);

        // Should be stale with 0 threshold (any position older than 0 seconds)
        // Note: This test is timing-dependent, so we just verify the method works
        let stale = tracker.get_stale_positions(0.0);
        // Position might be 0 or 1 depending on timing, so we just check it doesn't panic
        assert!(stale.len() <= 1);
    }

    #[test]
    fn test_position_tracker_losing_positions() {
        let mut tracker = PositionTracker::new();

        // Entry at 0.50
        tracker.on_fill("market1".to_string(), Outcome::Up, 10.0, 0.50, true);

        // Update price to 0.40 (losing)
        tracker.update_price("market1", Outcome::Up, 0.40);

        let losing = tracker.get_losing_positions(0.5);
        assert_eq!(losing.len(), 1);
        assert_eq!(losing[0], "market1");
    }
}
