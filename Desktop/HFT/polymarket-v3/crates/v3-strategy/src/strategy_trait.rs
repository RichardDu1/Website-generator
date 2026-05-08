use async_trait::async_trait;
use v3_core::{Market, MarketState, Outcome, OrderSide, Position};
use v3_signal::SignalSnapshot;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Strategy trait - all strategies must implement this
#[async_trait]
pub trait Strategy: Send + Sync {
    /// Strategy name
    fn name(&self) -> &str;

    /// Generate trading decision based on current state
    async fn generate_decision(
        &self,
        market: &Market,
        signal: &SignalSnapshot,
        position: Option<&Position>,
    ) -> Result<StrategyDecision>;

    /// Check if position should be exited
    fn should_exit(&self, position: &Position, signal: &SignalSnapshot) -> bool;

    /// Calculate suitability score for this strategy (0-1)
    /// Higher score means strategy is more suitable for current market conditions
    fn suitability_score(&self, market_state: &MarketState) -> f64;

    /// Check if strategy is enabled
    fn is_enabled(&self) -> bool;
}

/// Strategy decision output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyDecision {
    /// Enter a new position
    Enter {
        outcome: Outcome,
        side: OrderSide,
        size: u64,
        price: f64,
        ttl: u64,
        reason: String,
    },
    /// Exit existing position
    Exit {
        reason: ExitReason,
    },
    /// Hold current position or do nothing
    Hold,
}

/// Exit reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    Timeout,
    Reversal,
    ManualClose,
    RiskLimit,
}

impl StrategyDecision {
    pub fn is_entry(&self) -> bool {
        matches!(self, StrategyDecision::Enter { .. })
    }

    pub fn is_exit(&self) -> bool {
        matches!(self, StrategyDecision::Exit { .. })
    }

    pub fn is_hold(&self) -> bool {
        matches!(self, StrategyDecision::Hold)
    }
}
