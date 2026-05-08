use std::collections::HashMap;
use tracing::{debug, info};
use v3_core::types::Order;

/// Track active orders
pub struct OrderTracker {
    orders: HashMap<String, OrderInfo>,
}

struct OrderInfo {
    order: Order,
    status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderStatus {
    Pending,
    Active,
    Filled,
    Cancelled,
    Expired,
}

impl OrderTracker {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
        }
    }

    /// Add a new order
    pub fn add_order(&mut self, order_id: String, order: Order) {
        debug!("Tracking order {}", order_id);
        self.orders.insert(
            order_id,
            OrderInfo {
                order,
                status: OrderStatus::Pending,
            },
        );
    }

    /// Update order status
    pub fn update_status(&mut self, order_id: &str, status: OrderStatus) {
        if let Some(info) = self.orders.get_mut(order_id) {
            debug!("Order {} status: {:?} -> {:?}", order_id, info.status, status);
            info.status = status;
        }
    }

    /// Remove an order
    pub fn remove_order(&mut self, order_id: &str) -> Option<Order> {
        self.orders.remove(order_id).map(|info| {
            debug!("Removed order {}", order_id);
            info.order
        })
    }

    /// Get order info
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        self.orders.get(order_id).map(|info| &info.order)
    }

    /// Get order status
    pub fn get_status(&self, order_id: &str) -> Option<OrderStatus> {
        self.orders.get(order_id).map(|info| info.status.clone())
    }

    /// Get all active orders
    pub fn active_orders(&self) -> Vec<(String, &Order)> {
        self.orders
            .iter()
            .filter(|(_, info)| info.status == OrderStatus::Active || info.status == OrderStatus::Pending)
            .map(|(id, info)| (id.clone(), &info.order))
            .collect()
    }

    /// Get count of active orders
    pub fn active_count(&self) -> usize {
        self.orders
            .values()
            .filter(|info| info.status == OrderStatus::Active || info.status == OrderStatus::Pending)
            .count()
    }

    /// Clean up completed orders
    pub fn cleanup_completed(&mut self) {
        let to_remove: Vec<String> = self
            .orders
            .iter()
            .filter(|(_, info)| {
                matches!(
                    info.status,
                    OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Expired
                )
            })
            .map(|(id, _)| id.clone())
            .collect();

        for order_id in to_remove {
            self.orders.remove(&order_id);
        }

        if !self.orders.is_empty() {
            info!("Cleaned up completed orders, {} active remaining", self.orders.len());
        }
    }

    /// Get all orders (for dashboard)
    pub fn get_all_orders(&self) -> Vec<TrackedOrder> {
        self.orders
            .iter()
            .map(|(id, info)| TrackedOrder {
                order_id: id.clone(),
                market_id: info.order.market.condition_id.clone(),
                side: format!("{:?}", info.order.side),
                price: 0.0, // TODO: extract from order
                size: info.order.size,
                ttl_secs: info.order.ttl_secs,
                created_at: info.order.created_at,
            })
            .collect()
    }

    /// Clear all orders
    pub fn clear(&mut self) {
        self.orders.clear();
    }
}

/// Tracked order info for dashboard
#[derive(Debug, Clone)]
pub struct TrackedOrder {
    pub order_id: String,
    pub market_id: String,
    pub side: String,
    pub price: f64,
    pub size: u64,
    pub ttl_secs: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Default for OrderTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use v3_core::types::{Market, OrderSide, Outcome};
    use v3_core::Asset;

    fn create_test_order() -> Order {
        Order {
            id: "test-order-1".to_string(),
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
            outcome: Outcome::Up,
            side: OrderSide::Buy,
            price: Decimal::new(5, 1), // 0.5
            size: 10,
            created_at: Utc::now(),
            ttl_secs: 30,
        }
    }

    #[test]
    fn test_add_and_get_order() {
        let mut tracker = OrderTracker::new();
        let order = create_test_order();

        tracker.add_order("order1".to_string(), order.clone());

        let retrieved = tracker.get_order("order1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-order-1");
    }

    #[test]
    fn test_update_status() {
        let mut tracker = OrderTracker::new();
        let order = create_test_order();

        tracker.add_order("order1".to_string(), order);
        tracker.update_status("order1", OrderStatus::Active);

        let status = tracker.get_status("order1");
        assert_eq!(status, Some(OrderStatus::Active));
    }

    #[test]
    fn test_active_orders() {
        let mut tracker = OrderTracker::new();

        tracker.add_order("order1".to_string(), create_test_order());
        tracker.add_order("order2".to_string(), create_test_order());
        tracker.update_status("order1", OrderStatus::Active);
        tracker.update_status("order2", OrderStatus::Filled);

        let active = tracker.active_orders();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_cleanup_completed() {
        let mut tracker = OrderTracker::new();

        tracker.add_order("order1".to_string(), create_test_order());
        tracker.add_order("order2".to_string(), create_test_order());
        tracker.update_status("order1", OrderStatus::Filled);
        tracker.update_status("order2", OrderStatus::Active);

        tracker.cleanup_completed();

        assert_eq!(tracker.active_count(), 1);
        assert!(tracker.get_order("order1").is_none());
        assert!(tracker.get_order("order2").is_some());
    }

    #[test]
    fn test_remove_order() {
        let mut tracker = OrderTracker::new();
        let order = create_test_order();

        tracker.add_order("order1".to_string(), order);
        let removed = tracker.remove_order("order1");

        assert!(removed.is_some());
        assert!(tracker.get_order("order1").is_none());
    }
}
