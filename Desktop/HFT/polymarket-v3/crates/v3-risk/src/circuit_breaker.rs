use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{error, info, warn};

use crate::limits::RiskManager;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,  // Normal operation
    Open,    // Trading halted
}

/// Circuit breaker for emergency trading halt
pub struct CircuitBreaker {
    risk_manager: Arc<RwLock<RiskManager>>,
    state: CircuitState,
    tripped_at: Option<DateTime<Utc>>,
    manual_halt: bool,
}

impl CircuitBreaker {
    pub fn new(risk_manager: Arc<RwLock<RiskManager>>) -> Self {
        Self {
            risk_manager,
            state: CircuitState::Closed,
            tripped_at: None,
            manual_halt: false,
        }
    }

    /// Check if trading should be allowed
    pub async fn should_allow_trading(&mut self) -> bool {
        // Manual halt overrides everything
        if self.manual_halt {
            return false;
        }

        // Check risk manager conditions
        let manager = self.risk_manager.read().await;
        let should_halt = manager.should_halt_trading();
        drop(manager);

        if should_halt && self.state == CircuitState::Closed {
            self.trip();
        } else if !should_halt && self.state == CircuitState::Open && !self.manual_halt {
            self.reset();
        }

        self.state == CircuitState::Closed
    }

    /// Trip the circuit breaker (halt trading)
    fn trip(&mut self) {
        if self.state == CircuitState::Closed {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Trading halted");
            self.state = CircuitState::Open;
            self.tripped_at = Some(Utc::now());
        }
    }

    /// Reset the circuit breaker (resume trading)
    fn reset(&mut self) {
        if self.state == CircuitState::Open && !self.manual_halt {
            info!("✅ Circuit breaker reset - Trading resumed");
            self.state = CircuitState::Closed;
            self.tripped_at = None;
        }
    }

    /// Manually halt trading
    pub fn manual_halt(&mut self) {
        warn!("⚠️  Manual trading halt activated");
        self.manual_halt = true;
        self.state = CircuitState::Open;
        self.tripped_at = Some(Utc::now());
    }

    /// Manually resume trading
    pub fn manual_resume(&mut self) {
        info!("✅ Manual trading halt lifted");
        self.manual_halt = false;
        self.state = CircuitState::Closed;
        self.tripped_at = None;
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Get time when circuit was tripped
    pub fn tripped_at(&self) -> Option<DateTime<Utc>> {
        self.tripped_at
    }

    /// Check if manually halted
    pub fn is_manual_halt(&self) -> bool {
        self.manual_halt
    }

    /// Check if circuit breaker is active (for dashboard)
    pub fn is_active(&self) -> bool {
        self.state == CircuitState::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::RiskLimits;

    #[tokio::test]
    async fn test_circuit_breaker_normal() {
        let limits = RiskLimits::default();
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(limits)));
        let mut breaker = CircuitBreaker::new(risk_manager);

        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.should_allow_trading().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_trips_on_loss() {
        let limits = RiskLimits {
            max_daily_loss: 10.0,
            ..Default::default()
        };
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(limits)));
        let mut breaker = CircuitBreaker::new(risk_manager.clone());

        // Record excessive loss
        risk_manager.write().await.record_realized_pnl(-15.0);

        assert!(!breaker.should_allow_trading().await);
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(breaker.tripped_at().is_some());
    }

    #[tokio::test]
    async fn test_manual_halt() {
        let limits = RiskLimits::default();
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(limits)));
        let mut breaker = CircuitBreaker::new(risk_manager);

        breaker.manual_halt();

        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(breaker.is_manual_halt());
        assert!(!breaker.should_allow_trading().await);
    }

    #[tokio::test]
    async fn test_manual_resume() {
        let limits = RiskLimits::default();
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(limits)));
        let mut breaker = CircuitBreaker::new(risk_manager);

        breaker.manual_halt();
        assert_eq!(breaker.state(), CircuitState::Open);

        breaker.manual_resume();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(!breaker.is_manual_halt());
        assert!(breaker.should_allow_trading().await);
    }

    #[tokio::test]
    async fn test_auto_reset_after_recovery() {
        let limits = RiskLimits {
            max_daily_loss: 10.0,
            ..Default::default()
        };
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(limits)));
        let mut breaker = CircuitBreaker::new(risk_manager.clone());

        // Trip breaker
        risk_manager.write().await.record_realized_pnl(-15.0);
        assert!(!breaker.should_allow_trading().await);

        // Manually reset PnL to simulate recovery (reset_daily only works on date change)
        {
            let mut manager = risk_manager.write().await;
            manager.record_realized_pnl(15.0); // Bring back to 0
        }

        // Should auto-reset
        assert!(breaker.should_allow_trading().await);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }
}
