use async_trait::async_trait;
use v3_core::{Market, MarketState, Outcome, OrderSide, Position, SpreadCaptureConfig};
use v3_signal::{SignalSnapshot, SpreadAnalyzer};
use crate::strategy_trait::{Strategy, StrategyDecision, ExitReason};
use anyhow::Result;
use tracing::{info, warn};

/// Spread capture strategy - arbitrage YES+NO pricing inefficiencies
pub struct SpreadCaptureStrategy {
    config: SpreadCaptureConfig,
    analyzer: SpreadAnalyzer,
}

impl SpreadCaptureStrategy {
    pub fn new(config: SpreadCaptureConfig) -> Self {
        Self {
            analyzer: SpreadAnalyzer::new(config.min_spread_edge),
            config,
        }
    }
}

#[async_trait]
impl Strategy for SpreadCaptureStrategy {
    fn name(&self) -> &str {
        "SpreadCapture"
    }

    async fn generate_decision(
        &self,
        _market: &Market,
        signal: &SignalSnapshot,
        position: Option<&Position>,
    ) -> Result<StrategyDecision> {
        // Check timeout for existing position
        if let Some(pos) = position {
            if pos.hold_duration_secs() >= self.config.max_hold_time as i64 {
                return Ok(StrategyDecision::Exit {
                    reason: ExitReason::Timeout,
                });
            }
            return Ok(StrategyDecision::Hold);
        }

        // Detect arbitrage opportunity
        let arb = self.analyzer.detect_arbitrage(
            signal.market_state.best_bid_up,
            signal.market_state.best_ask_up,
            signal.market_state.best_bid_down,
            signal.market_state.best_ask_down,
        );

        if let Some(opportunity) = arb {
            info!(
                "Arbitrage opportunity: {:?} @ {:.4}, edge={:.2}%",
                opportunity.buy_outcome,
                opportunity.buy_price,
                opportunity.edge_pct * 100.0
            );

            // Bug #11 Fix: Validate position size
            let size = self.config.position_size;
            if size <= 0 {
                warn!("Invalid position size: {}, skipping arbitrage", size);
                return Ok(StrategyDecision::Hold);
            }

            return Ok(StrategyDecision::Enter {
                outcome: opportunity.buy_outcome,
                side: OrderSide::Buy,
                size,
                price: opportunity.buy_price,
                ttl: 60, // Longer TTL for arbitrage
                reason: format!("Arbitrage: edge={:.2}%", opportunity.edge_pct * 100.0),
            });
        }

        Ok(StrategyDecision::Hold)
    }

    fn should_exit(&self, position: &Position, _signal: &SignalSnapshot) -> bool {
        position.hold_duration_secs() >= self.config.max_hold_time as i64
    }

    fn suitability_score(&self, market_state: &MarketState) -> f64 {
        // Check if arbitrage exists by detecting it
        let arb = self.analyzer.detect_arbitrage(
            market_state.best_bid_up,
            market_state.best_ask_up,
            market_state.best_bid_down,
            market_state.best_ask_down,
        );

        if arb.is_some() {
            1.0
        } else {
            0.0
        }
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}
