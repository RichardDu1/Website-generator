use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use anyhow::{Result, bail};

use v3_core::types::Position;
use v3_core::Asset;
use crate::persistence::DailyPnlState;

/// Risk limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Maximum position size per market (USDC)
    pub max_position_size: u64,

    /// Maximum total exposure across all positions (USDC)
    pub max_total_exposure: u64,

    /// Maximum daily loss (USDC)
    pub max_daily_loss: f64,

    /// Maximum position hold time (seconds)
    pub max_hold_time_secs: u64,

    /// Maximum number of concurrent positions
    pub max_concurrent_positions: usize,

    /// Maximum loss per position (USDC)
    pub max_loss_per_position: f64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: 10,
            max_total_exposure: 50,
            max_daily_loss: 20.0,
            max_hold_time_secs: 30,
            max_concurrent_positions: 10,
            max_loss_per_position: 5.0,
        }
    }
}

/// Risk manager
pub struct RiskManager {
    limits: RiskLimits,
    daily_pnl: f64,
    daily_reset_time: DateTime<Utc>,
    positions: HashMap<String, Position>,
    state_file: PathBuf,
}

impl RiskManager {
    pub fn new(limits: RiskLimits) -> Self {
        let state_file = PathBuf::from("state/daily_pnl.json");

        // Load persisted state
        let state = DailyPnlState::load_or_new(&state_file);

        Self {
            limits,
            daily_pnl: state.daily_pnl,
            daily_reset_time: state.last_updated,
            positions: HashMap::new(),
            state_file,
        }
    }

    /// Save current state to disk
    fn save_state(&self) {
        let state = DailyPnlState::new(self.daily_pnl);
        if let Err(e) = state.save(&self.state_file) {
            warn!("Failed to save PnL state: {}", e);
        }
    }

    /// Check if we can open a new position
    pub fn can_open_position(&self, size: u64, _asset: Asset) -> Result<()> {
        // Check position size
        if size > self.limits.max_position_size {
            bail!(
                "Position size {} exceeds limit {}",
                size,
                self.limits.max_position_size
            );
        }

        // Check concurrent positions
        if self.positions.len() >= self.limits.max_concurrent_positions {
            bail!(
                "Already at max concurrent positions: {}",
                self.limits.max_concurrent_positions
            );
        }

        // Check total exposure
        let current_exposure: u64 = self.positions.values().map(|p| p.size).sum();
        if current_exposure + size > self.limits.max_total_exposure {
            bail!(
                "Total exposure {} + {} would exceed limit {}",
                current_exposure,
                size,
                self.limits.max_total_exposure
            );
        }

        // Check daily loss
        if self.daily_pnl < -self.limits.max_daily_loss {
            bail!(
                "Daily loss {:.2} exceeds limit {:.2}",
                -self.daily_pnl,
                self.limits.max_daily_loss
            );
        }

        Ok(())
    }

    /// Add a position
    pub fn add_position(&mut self, position: Position) {
        debug!("Adding position: {}", position.id);
        self.positions.insert(position.id.clone(), position);
    }

    /// Remove a position
    pub fn remove_position(&mut self, position_id: &str) -> Option<Position> {
        debug!("Removing position: {}", position_id);
        self.positions.remove(position_id)
    }

    /// Update position PnL
    pub fn update_position_pnl(&mut self, position_id: &str, unrealized_pnl: f64) {
        if let Some(position) = self.positions.get_mut(position_id) {
            position.unrealized_pnl = unrealized_pnl;
        }
    }

    /// Record realized PnL
    pub fn record_realized_pnl(&mut self, pnl: f64) {
        self.daily_pnl += pnl;
        debug!("Recorded PnL: {:.2}, Daily total: {:.2}", pnl, self.daily_pnl);

        // Persist to disk
        self.save_state();
    }

    /// Check if position should be force-closed
    pub fn should_force_close(&self, position: &Position) -> bool {
        // Check hold time
        let hold_time = (Utc::now() - position.entry_time).num_seconds() as u64;
        if hold_time >= self.limits.max_hold_time_secs {
            warn!(
                "Position {} exceeded max hold time: {}s >= {}s",
                position.id, hold_time, self.limits.max_hold_time_secs
            );
            return true;
        }

        // Check loss limit
        if position.unrealized_pnl < -self.limits.max_loss_per_position {
            warn!(
                "Position {} exceeded max loss: {:.2} < -{:.2}",
                position.id, position.unrealized_pnl, self.limits.max_loss_per_position
            );
            return true;
        }

        false
    }

    /// Check if trading should be halted
    pub fn should_halt_trading(&self) -> bool {
        if self.daily_pnl < -self.limits.max_daily_loss {
            warn!(
                "Daily loss {:.2} exceeds limit {:.2}, halting trading",
                -self.daily_pnl, self.limits.max_daily_loss
            );
            return true;
        }
        false
    }

    /// Reset daily counters (call at start of new day)
    pub fn reset_daily(&mut self) {
        let now = Utc::now();
        if now.date_naive() != self.daily_reset_time.date_naive() {
            debug!("Resetting daily counters");
            self.daily_pnl = 0.0;
            self.daily_reset_time = now;

            // Persist reset state
            self.save_state();
        }
    }

    /// Get current positions
    pub fn positions(&self) -> &HashMap<String, Position> {
        &self.positions
    }

    /// Get daily PnL
    pub fn daily_pnl(&self) -> f64 {
        self.daily_pnl
    }

    /// Get total exposure
    pub fn total_exposure(&self) -> u64 {
        self.positions.values().map(|p| p.size).sum()
    }

    /// Get risk limits (for dashboard)
    pub fn limits(&self) -> &RiskLimits {
        &self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v3_core::types::{Market, Outcome, PositionSide};

    fn create_test_position(id: &str, size: u64) -> Position {
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
            size,
            entry_time: Utc::now(),
            unrealized_pnl: 0.0,
            strategy_name: "test".to_string(),
        }
    }

    #[test]
    fn test_can_open_position() {
        let limits = RiskLimits::default();
        let manager = RiskManager::new(limits);

        assert!(manager.can_open_position(5, Asset::BTC).is_ok());
        assert!(manager.can_open_position(15, Asset::BTC).is_err()); // Exceeds max size
    }

    #[test]
    fn test_max_concurrent_positions() {
        let limits = RiskLimits {
            max_concurrent_positions: 2,
            ..Default::default()
        };
        let mut manager = RiskManager::new(limits);

        manager.add_position(create_test_position("pos1", 5));
        manager.add_position(create_test_position("pos2", 5));

        assert!(manager.can_open_position(5, Asset::BTC).is_err());
    }

    #[test]
    fn test_total_exposure() {
        let mut manager = RiskManager::new(RiskLimits::default());

        manager.add_position(create_test_position("pos1", 10));
        manager.add_position(create_test_position("pos2", 15));

        assert_eq!(manager.total_exposure(), 25);
    }

    #[test]
    fn test_daily_loss_limit() {
        let limits = RiskLimits {
            max_daily_loss: 10.0,
            ..Default::default()
        };
        let mut manager = RiskManager::new(limits);

        manager.record_realized_pnl(-5.0);
        assert!(manager.can_open_position(5, Asset::BTC).is_ok());

        manager.record_realized_pnl(-6.0);
        assert!(manager.should_halt_trading());
    }

    #[test]
    fn test_force_close_on_loss() {
        let limits = RiskLimits {
            max_loss_per_position: 3.0,
            ..Default::default()
        };
        let manager = RiskManager::new(limits);

        let mut position = create_test_position("pos1", 5);
        position.unrealized_pnl = -4.0;

        assert!(manager.should_force_close(&position));
    }
}
