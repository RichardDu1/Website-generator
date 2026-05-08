use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info};

/// TTL (Time-To-Live) manager for orders
pub struct TtlManager {
    orders: HashMap<String, OrderTtl>,
}

struct OrderTtl {
    order_id: String,
    created_at: Instant,
    ttl_secs: u64,
}

impl TtlManager {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
        }
    }

    /// Register an order with TTL
    pub fn register_order(&mut self, order_id: String, ttl_secs: u64) {
        debug!("Registering order {} with TTL {}s", order_id, ttl_secs);
        self.orders.insert(
            order_id.clone(),
            OrderTtl {
                order_id,
                created_at: Instant::now(),
                ttl_secs,
            },
        );
    }

    /// Unregister an order (when filled or manually cancelled)
    pub fn unregister_order(&mut self, order_id: &str) {
        if self.orders.remove(order_id).is_some() {
            debug!("Unregistered order {}", order_id);
        }
    }

    /// Check for expired orders
    pub fn check_expired(&self) -> Vec<String> {
        let now = Instant::now();
        let mut expired = Vec::new();

        for (order_id, ttl) in &self.orders {
            let elapsed = now.duration_since(ttl.created_at).as_secs();
            if elapsed >= ttl.ttl_secs {
                expired.push(order_id.clone());
            }
        }

        if !expired.is_empty() {
            info!("Found {} expired orders", expired.len());
        }

        expired
    }

    /// Get time remaining for an order
    pub fn time_remaining(&self, order_id: &str) -> Option<u64> {
        self.orders.get(order_id).map(|ttl| {
            let elapsed = Instant::now().duration_since(ttl.created_at).as_secs();
            ttl.ttl_secs.saturating_sub(elapsed)
        })
    }

    /// Get count of tracked orders
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// Clear all orders
    pub fn clear(&mut self) {
        self.orders.clear();
    }
}

impl Default for TtlManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_ttl_registration() {
        let mut manager = TtlManager::new();

        manager.register_order("order1".to_string(), 30);
        manager.register_order("order2".to_string(), 60);

        assert_eq!(manager.order_count(), 2);
    }

    #[test]
    fn test_ttl_expiration() {
        let mut manager = TtlManager::new();

        manager.register_order("order1".to_string(), 1); // 1 second TTL

        // Should not be expired immediately
        let expired = manager.check_expired();
        assert_eq!(expired.len(), 0);

        // Wait for expiration
        thread::sleep(Duration::from_millis(1100));

        let expired = manager.check_expired();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], "order1");
    }

    #[test]
    fn test_ttl_unregister() {
        let mut manager = TtlManager::new();

        manager.register_order("order1".to_string(), 30);
        assert_eq!(manager.order_count(), 1);

        manager.unregister_order("order1");
        assert_eq!(manager.order_count(), 0);
    }

    #[test]
    fn test_time_remaining() {
        let mut manager = TtlManager::new();

        manager.register_order("order1".to_string(), 30);

        let remaining = manager.time_remaining("order1");
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= 30);
    }
}
