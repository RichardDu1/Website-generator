use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};
use anyhow::Result;

use v3_core::types::Position;
use crate::limits::RiskManager;

/// Position monitor for checking hold times and forcing closes
pub struct PositionMonitor {
    risk_manager: Arc<RwLock<RiskManager>>,
    check_interval_ms: u64,
}

impl PositionMonitor {
    pub fn new(risk_manager: Arc<RwLock<RiskManager>>, check_interval_ms: u64) -> Self {
        Self {
            risk_manager,
            check_interval_ms,
        }
    }

    /// Start monitoring loop
    pub async fn run(&self) -> Result<()> {
        info!("Starting position monitor (check interval: {}ms)", self.check_interval_ms);

        loop {
            sleep(Duration::from_millis(self.check_interval_ms)).await;

            if let Err(e) = self.check_positions().await {
                warn!("Position monitor error: {}", e);
            }
        }
    }

    /// Check all positions for force-close conditions
    async fn check_positions(&self) -> Result<()> {
        let manager = self.risk_manager.read().await;

        let positions_to_close: Vec<Position> = manager
            .positions()
            .values()
            .filter(|p| manager.should_force_close(p))
            .cloned()
            .collect();

        drop(manager);

        if !positions_to_close.is_empty() {
            info!("Found {} positions that need force-closing", positions_to_close.len());

            for position in positions_to_close {
                warn!(
                    "Position {} should be force-closed: hold_time={}s, pnl={:.2}",
                    position.id,
                    (chrono::Utc::now() - position.entry_time).num_seconds(),
                    position.unrealized_pnl
                );
                // TODO: Trigger force close via executor
            }
        }

        Ok(())
    }

    /// Get positions that need force-closing
    pub async fn get_positions_to_close(&self) -> Vec<String> {
        let manager = self.risk_manager.read().await;

        manager
            .positions()
            .values()
            .filter(|p| manager.should_force_close(p))
            .map(|p| p.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v3_core::types::{Market, Outcome, PositionSide};
    use v3_core::Asset;
    use crate::limits::RiskLimits;
    use chrono::Utc;

    fn create_test_position(id: &str, hold_time_secs: i64, pnl: f64) -> Position {
        Position {
            id: id.to_string(),
            market: Market {
                condition_id: "test".to_string(),
                token_id_yes: "yes".to_string(),
                token_id_no: "no".to_string(),
                asset: Asset::BTC,
                open_time: Utc::now(),
                close_time: Utc::now() + chrono::Duration::minutes(15),
                open_price: 50000.0,
                slug: "test".to_string(),
            },
            outcome: Outcome::Up,
            side: PositionSide::Long,
            entry_price: 0.5,
            size: 10,
            entry_time: Utc::now() - chrono::Duration::seconds(hold_time_secs),
            unrealized_pnl: pnl,
            strategy_name: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_detect_expired_position() {
        let limits = RiskLimits {
            max_hold_time_secs: 30,
            ..Default::default()
        };
        let mut risk_manager = RiskManager::new(limits);

        // Add position that's been held too long
        let position = create_test_position("pos1", 35, 0.0);
        risk_manager.add_position(position);

        let manager_arc = Arc::new(RwLock::new(risk_manager));
        let monitor = PositionMonitor::new(manager_arc, 100);

        let to_close = monitor.get_positions_to_close().await;
        assert_eq!(to_close.len(), 1);
        assert_eq!(to_close[0], "pos1");
    }

    #[tokio::test]
    async fn test_detect_loss_position() {
        let limits = RiskLimits {
            max_loss_per_position: 5.0,
            ..Default::default()
        };
        let mut risk_manager = RiskManager::new(limits);

        // Add position with excessive loss
        let position = create_test_position("pos1", 10, -6.0);
        risk_manager.add_position(position);

        let manager_arc = Arc::new(RwLock::new(risk_manager));
        let monitor = PositionMonitor::new(manager_arc, 100);

        let to_close = monitor.get_positions_to_close().await;
        assert_eq!(to_close.len(), 1);
    }

    #[tokio::test]
    async fn test_no_force_close_needed() {
        let limits = RiskLimits::default();
        let mut risk_manager = RiskManager::new(limits);

        // Add healthy position
        let position = create_test_position("pos1", 10, 2.0);
        risk_manager.add_position(position);

        let manager_arc = Arc::new(RwLock::new(risk_manager));
        let monitor = PositionMonitor::new(manager_arc, 100);

        let to_close = monitor.get_positions_to_close().await;
        assert_eq!(to_close.len(), 0);
    }
}
