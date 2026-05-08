use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};
use anyhow::Result;

use v3_signal::binance_monitor::BinanceMonitor;
use v3_core::types::{Order, OrderSide, Outcome};

use crate::order_tracker::{OrderTracker, OrderStatus};

/// Monitor for preemptive order cancellation (distinct-baguette style)
/// Marks orders for cancellation in OrderTracker, actual cancellation done by executor
pub struct CancelMonitor {
    binance_monitor: Arc<RwLock<BinanceMonitor>>,
    order_tracker: Arc<RwLock<OrderTracker>>,
    adverse_threshold: f64, // $/second threshold for adverse movement
    check_interval_ms: u64,
}

impl CancelMonitor {
    pub fn new(
        binance_monitor: Arc<RwLock<BinanceMonitor>>,
        order_tracker: Arc<RwLock<OrderTracker>>,
        adverse_threshold: f64,
        check_interval_ms: u64,
    ) -> Self {
        Self {
            binance_monitor,
            order_tracker,
            adverse_threshold,
            check_interval_ms,
        }
    }

    /// Start monitoring loop
    pub async fn run(&self) -> Result<()> {
        info!("Starting cancel monitor (adverse threshold: ${}/s)", self.adverse_threshold);

        loop {
            sleep(Duration::from_millis(self.check_interval_ms)).await;

            if let Err(e) = self.check_and_cancel().await {
                warn!("Cancel monitor error: {}", e);
            }
        }
    }

    /// Check for adverse movements and cancel orders
    async fn check_and_cancel(&self) -> Result<()> {
        let binance = self.binance_monitor.read().await;
        let velocity = binance.price_velocity();
        drop(binance);

        // Check if velocity exceeds threshold
        if velocity.abs() < self.adverse_threshold {
            return Ok(());
        }

        debug!("Adverse movement detected: velocity = ${:.4}/s", velocity);

        let tracker = self.order_tracker.read().await;
        let active_orders: Vec<(String, Order)> = tracker.active_orders()
            .into_iter()
            .map(|(id, order)| (id, order.clone()))
            .collect();
        drop(tracker);

        let mut to_cancel = Vec::new();

        for (order_id, order) in active_orders {
            if self.should_cancel_order(&order, velocity) {
                to_cancel.push(order_id);
            }
        }

        if !to_cancel.is_empty() {
            warn!("⚠️ Adverse movement! Marking {} orders for cancellation (velocity=${:.4}/s)", to_cancel.len(), velocity);

            // Mark orders for cancellation in tracker
            // Actual cancellation will be done by executor in next tick
            let mut tracker = self.order_tracker.write().await;
            for order_id in to_cancel {
                tracker.update_status(&order_id, OrderStatus::Cancelled);
                debug!("Marked order {} for cancellation due to adverse movement", order_id);
            }
        }

        Ok(())
    }

    /// Determine if an order should be cancelled based on price movement
    fn should_cancel_order(&self, order: &Order, velocity: f64) -> bool {
        // Cancel BUY orders when price is falling fast
        // Cancel SELL orders when price is rising fast
        match (order.side, order.outcome) {
            // Buying UP = betting price goes up
            (OrderSide::Buy, Outcome::Up) => velocity < -self.adverse_threshold,
            // Selling UP = betting price goes down
            (OrderSide::Sell, Outcome::Up) => velocity > self.adverse_threshold,
            // Buying DOWN = betting price goes down
            (OrderSide::Buy, Outcome::Down) => velocity > self.adverse_threshold,
            // Selling DOWN = betting price goes up
            (OrderSide::Sell, Outcome::Down) => velocity < -self.adverse_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use v3_core::types::{Market, OrderSide, Outcome};
    use v3_core::Asset;

    fn create_test_order(side: OrderSide, outcome: Outcome) -> Order {
        Order {
            id: "test-order".to_string(),
            market: Market {
                condition_id: "test-condition".to_string(),
                token_id_yes: "test-yes".to_string(),
                token_id_no: "test-no".to_string(),
                asset: Asset::BTC,
                open_time: Utc::now(),
                close_time: Utc::now() + chrono::Duration::minutes(15),
                open_price: 50000.0,
                slug: "test-market".to_string(),
            },
            outcome,
            side,
            price: Decimal::new(5, 1), // 0.5
            size: 10,
            created_at: Utc::now(),
            ttl_secs: 30,
        }
    }

    #[test]
    fn test_should_cancel_buy_up_on_falling() {
        let binance = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let monitor = CancelMonitor::new(binance, tracker, 0.5, 100);

        let order = create_test_order(OrderSide::Buy, Outcome::Up);

        // Falling price (negative velocity) should cancel BUY UP
        assert!(monitor.should_cancel_order(&order, -0.6));
        // Rising price should not cancel BUY UP
        assert!(!monitor.should_cancel_order(&order, 0.6));
    }

    #[test]
    fn test_should_cancel_sell_up_on_rising() {
        let binance = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let monitor = CancelMonitor::new(binance, tracker, 0.5, 100);

        let order = create_test_order(OrderSide::Sell, Outcome::Up);

        // Rising price should cancel SELL UP
        assert!(monitor.should_cancel_order(&order, 0.6));
        // Falling price should not cancel SELL UP
        assert!(!monitor.should_cancel_order(&order, -0.6));
    }

    #[test]
    fn test_should_cancel_buy_down_on_rising() {
        let binance = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let monitor = CancelMonitor::new(binance, tracker, 0.5, 100);

        let order = create_test_order(OrderSide::Buy, Outcome::Down);

        // Rising price should cancel BUY DOWN
        assert!(monitor.should_cancel_order(&order, 0.6));
        // Falling price should not cancel BUY DOWN
        assert!(!monitor.should_cancel_order(&order, -0.6));
    }

    #[test]
    fn test_should_cancel_sell_down_on_falling() {
        let binance = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let monitor = CancelMonitor::new(binance, tracker, 0.5, 100);

        let order = create_test_order(OrderSide::Sell, Outcome::Down);

        // Falling price should cancel SELL DOWN
        assert!(monitor.should_cancel_order(&order, -0.6));
        // Rising price should not cancel SELL DOWN
        assert!(!monitor.should_cancel_order(&order, 0.6));
    }

    #[test]
    fn test_threshold_boundary() {
        let binance = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let monitor = CancelMonitor::new(binance, tracker, 0.5, 100);

        let order = create_test_order(OrderSide::Buy, Outcome::Up);

        // Exactly at threshold should not cancel
        assert!(!monitor.should_cancel_order(&order, -0.5));
        // Just below threshold should cancel
        assert!(monitor.should_cancel_order(&order, -0.51));
    }
}
