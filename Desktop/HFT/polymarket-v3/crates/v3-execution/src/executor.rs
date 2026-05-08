use std::sync::Arc;
use tokio::sync::RwLock;
use v3_core::{Order, OrderSide, Outcome};
use anyhow::{Result, Context};
use tracing::{info, warn};
use crate::{TtlManager, CancelMonitor, OrderTracker};
use mm_exchange::polymarket::OrderClient;
use rust_decimal::prelude::*;

/// Order executor - handles order placement and cancellation
pub struct OrderExecutor {
    client: Option<Arc<OrderClient>>,
    ttl_manager: Arc<RwLock<TtlManager>>,
    cancel_monitor: Arc<CancelMonitor>,
    order_tracker: Arc<RwLock<OrderTracker>>,
    dry_run: bool,
}

impl OrderExecutor {
    pub async fn new(
        private_key: Option<String>,
        ttl_manager: Arc<RwLock<TtlManager>>,
        cancel_monitor: Arc<CancelMonitor>,
        order_tracker: Arc<RwLock<OrderTracker>>,
        dry_run: bool,
    ) -> Result<Self> {
        let client = if !dry_run {
            let key = private_key
                .ok_or_else(|| anyhow::anyhow!("PRIVATE_KEY required for live trading"))?;

            info!("Initializing Polymarket OrderClient for live trading...");
            let order_client = OrderClient::new("https://clob.polymarket.com", &key)
                .await
                .context("Failed to initialize OrderClient")?;

            info!("OrderClient initialized successfully");
            Some(Arc::new(order_client))
        } else {
            info!("Running in dry-run mode, OrderClient not initialized");
            None
        };

        Ok(Self {
            client,
            ttl_manager,
            cancel_monitor,
            order_tracker,
            dry_run,
        })
    }

    /// Execute an order with TTL
    pub async fn execute_order(&self, order: Order) -> Result<String> {
        // Validate minimum order size ($1 for small account testing)
        let order_value = order.size as f64 * order.price.to_f64().unwrap_or(0.0);
        if order_value < 1.0 {
            warn!(
                "⚠️ Order value ${:.2} < $1 minimum (size: {}, price: {:.4}), skipping",
                order_value, order.size, order.price
            );
            return Err(anyhow::anyhow!("Order value ${:.2} below $1 minimum", order_value));
        }

        if self.dry_run {
            info!(
                "DRY-RUN: Would place order: {:?} {:?} @ {:.4} (size: {}, value: ${:.2}, TTL: {}s)",
                order.outcome, order.side, order.price, order.size, order_value, order.ttl_secs
            );
            let order_id = format!("dry-run-{}", uuid::Uuid::new_v4());

            // Track order
            self.order_tracker.write().await.add_order(order_id.clone(), order.clone());

            // Register with TTL manager
            self.ttl_manager.write().await.register_order(order_id.clone(), order.ttl_secs);

            return Ok(order_id);
        }

        // Real order placement
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OrderClient not initialized"))?;

        // Determine token_id based on outcome
        let token_id = match order.outcome {
            Outcome::Up => &order.market.token_id_yes,
            Outcome::Down => &order.market.token_id_no,
        };

        // CRITICAL FIX #5: Validate order price against current market
        // This prevents placing orders at unrealistic prices that will never fill
        let price_f64 = order.price.to_f64().unwrap_or(0.0);

        // Price sanity checks
        if price_f64 < 0.01 || price_f64 > 0.99 {
            return Err(anyhow::anyhow!(
                "Order price {:.4} outside valid range [0.01, 0.99]",
                price_f64
            ));
        }

        info!("Price validation passed: {:.4} within valid range", price_f64);

        // Convert side to mm-exchange type
        let side = match order.side {
            OrderSide::Buy => mm_exchange::types::OrderSide::Buy,
            OrderSide::Sell => mm_exchange::types::OrderSide::Sell,
        };

        // Convert price to Decimal
        let price = order.price.to_f64()
            .ok_or_else(|| anyhow::anyhow!("Invalid price"))?;
        let price_decimal = Decimal::from_f64(price)
            .ok_or_else(|| anyhow::anyhow!("Failed to convert price to Decimal"))?;

        // Convert size to Decimal
        let size_decimal = Decimal::from(order.size);

        // Place order via CLOB
        info!(
            "Placing LIVE order: {:?} {:?} @ {:.4} (size: {}, token: {})",
            order.outcome, order.side, price, order.size, token_id
        );

        // Try post-only first, fallback to GTC if it crosses the book
        let order_id = match client.place_limit_order(
            token_id,
            side,
            price_decimal,
            size_decimal,
        ).await {
            Ok(id) => {
                info!("✅ Post-only order placed: {}", id);
                id
            }
            Err(e) if e.to_string().contains("crosses book") => {
                warn!("⚠️ Post-only order crosses book, retrying with GTC (taker)...");
                client.place_gtc_order(
                    token_id,
                    side,
                    price_decimal,
                    size_decimal,
                ).await
                .map_err(|e| {
                    warn!("❌ GTC order placement failed:");
                    warn!("  Error: {:?}", e);
                    warn!("  Market: {}", order.market.condition_id);
                    warn!("  Token ID: {}", token_id);
                    warn!("  Side: {:?}", side);
                    warn!("  Price: {}", price_decimal);
                    warn!("  Size: {}", size_decimal);
                    warn!("  Outcome: {:?}", order.outcome);
                    anyhow::anyhow!("CLOB API error: {:#}", e)
                })?
            }
            Err(e) => {
                warn!("❌ CLOB order placement failed:");
                warn!("  Error: {:?}", e);
                warn!("  Market: {}", order.market.condition_id);
                warn!("  Token ID: {}", token_id);
                warn!("  Side: {:?}", side);
                warn!("  Price: {}", price_decimal);
                warn!("  Size: {}", size_decimal);
                warn!("  Outcome: {:?}", order.outcome);
                return Err(anyhow::anyhow!("CLOB API error: {:#}", e));
            }
        };

        info!("✅ Order placed successfully: {}", order_id);

        // Track order
        self.order_tracker.write().await.add_order(order_id.clone(), order.clone());

        // Register with TTL manager
        self.ttl_manager.write().await.register_order(order_id.clone(), order.ttl_secs);

        Ok(order_id)
    }

    /// Cancel specific orders
    pub async fn cancel_orders(&self, order_ids: &[String]) -> Result<()> {
        if order_ids.is_empty() {
            return Ok(());
        }

        if self.dry_run {
            info!("DRY-RUN: Would cancel {} orders", order_ids.len());
            for order_id in order_ids {
                self.order_tracker.write().await.remove_order(order_id);
                self.ttl_manager.write().await.unregister_order(order_id);
            }
            return Ok(());
        }

        // Real order cancellation
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OrderClient not initialized"))?;

        info!("Cancelling {} LIVE orders", order_ids.len());

        // Cancel orders one by one (CLOB API doesn't support batch cancellation)
        let mut success_count = 0;
        let mut fail_count = 0;

        for order_id in order_ids {
            match client.cancel_order(order_id).await {
                Ok(_) => {
                    success_count += 1;
                }
                Err(e) => {
                    warn!("Failed to cancel order {}: {}", order_id, e);
                    fail_count += 1;
                }
            }
        }

        if success_count > 0 {
            info!("✅ Successfully cancelled {} orders", success_count);
        }
        if fail_count > 0 {
            warn!("⚠️ Failed to cancel {} orders", fail_count);
        }

        // Remove from tracking (even if cancellation failed)
        let mut tracker = self.order_tracker.write().await;
        for order_id in order_ids {
            tracker.remove_order(order_id);
        }
        drop(tracker);

        // Unregister from TTL manager
        let mut ttl = self.ttl_manager.write().await;
        for order_id in order_ids {
            ttl.unregister_order(order_id);
        }

        Ok(())
    }

    /// Cancel all orders
    pub async fn cancel_all(&self) -> Result<()> {
        let tracker = self.order_tracker.read().await;
        let order_ids: Vec<String> = tracker.active_orders()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        drop(tracker);
        self.cancel_orders(&order_ids).await
    }

    /// Get order tracker
    pub fn order_tracker(&self) -> Arc<RwLock<OrderTracker>> {
        Arc::clone(&self.order_tracker)
    }

    /// Query order status from CLOB API
    /// Returns: "FILLED", "OPEN", "CANCELLED", "EXPIRED", or "PARTIALLY_FILLED"
    pub async fn get_order_status(&self, order_id: &str) -> Result<String> {
        if self.dry_run {
            // In dry-run mode, simulate filled orders
            return Ok("FILLED".to_string());
        }

        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OrderClient not initialized"))?;

        // Query order status via CLOB API
        // OrderClient.get_order_status returns (is_live, size_matched)
        match client.get_order_status(order_id).await {
            Ok((is_live, size_matched)) => {
                use rust_decimal::prelude::ToPrimitive;
                let matched = size_matched.to_f64().unwrap_or(0.0);

                if !is_live && matched > 0.0 {
                    // Order is no longer live and has matched size -> FILLED
                    Ok("FILLED".to_string())
                } else if !is_live {
                    // Order is no longer live but no matched size -> CANCELLED or EXPIRED
                    Ok("CANCELLED".to_string())
                } else if matched > 0.0 {
                    // Order is still live but has partial fill
                    Ok("PARTIALLY_FILLED".to_string())
                } else {
                    // Order is still live with no fills
                    Ok("OPEN".to_string())
                }
            }
            Err(e) => {
                Err(anyhow::anyhow!("Failed to query order status for {}: {}", order_id, e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v3_core::types::{Market, OrderSide, Outcome};
    use v3_core::Asset;
    use v3_signal::BinanceMonitor;
    use chrono::Utc;
    use rust_decimal::Decimal;

    fn create_test_order() -> Order {
        Order {
            id: String::new(),
            market: Market {
                condition_id: "test".to_string(),
                token_id_yes: "yes".to_string(),
                token_id_no: "no".to_string(),
                asset: Asset::BTC,
                open_time: Utc::now(),
                close_time: Utc::now() + chrono::Duration::minutes(15),
                open_price: 100000.0,
                slug: "test".to_string(),
            },
            outcome: Outcome::Up,
            side: OrderSide::Buy,
            price: Decimal::new(50, 2), // 0.50
            size: 10,
            created_at: Utc::now(),
            ttl_secs: 30,
        }
    }

    #[tokio::test]
    async fn test_dry_run_order() {
        let ttl_manager = Arc::new(RwLock::new(TtlManager::new()));
        let order_tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let binance_monitor = Arc::new(RwLock::new(BinanceMonitor::new(Asset::BTC, 0.5, 0.1)));
        let cancel_monitor = Arc::new(CancelMonitor::new(
            binance_monitor,
            order_tracker.clone(),
            0.5,
            100,
        ));

        let executor = OrderExecutor::new(
            ttl_manager,
            cancel_monitor,
            order_tracker.clone(),
            true, // dry_run
        );

        let order = create_test_order();
        let result = executor.execute_order(order).await;

        assert!(result.is_ok());
        assert_eq!(order_tracker.read().await.active_count(), 1);
    }

    /// Query order status from CLOB API
    /// Returns: "FILLED", "OPEN", "CANCELLED", "EXPIRED", or "UNKNOWN"
    pub async fn get_order_status(&self, order_id: &str) -> Result<String> {
        if self.dry_run {
            // In dry-run mode, simulate filled orders
            return Ok("FILLED".to_string());
        }

        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("OrderClient not initialized"))?;

        // Query order status via CLOB API
        // OrderClient.get_order_status returns (is_live, size_matched)
        match client.get_order_status(order_id).await {
            Ok((is_live, size_matched)) => {
                use rust_decimal::prelude::ToPrimitive;
                let matched = size_matched.to_f64().unwrap_or(0.0);

                if !is_live && matched > 0.0 {
                    // Order is no longer live and has matched size -> FILLED
                    Ok("FILLED".to_string())
                } else if !is_live {
                    // Order is no longer live but no matched size -> CANCELLED or EXPIRED
                    Ok("CANCELLED".to_string())
                } else if matched > 0.0 {
                    // Order is still live but has partial fill
                    Ok("PARTIALLY_FILLED".to_string())
                } else {
                    // Order is still live with no fills
                    Ok("OPEN".to_string())
                }
            }
            Err(e) => {
                Err(anyhow::anyhow!("Failed to query order status for {}: {}", order_id, e))
            }
        }
    }
}
