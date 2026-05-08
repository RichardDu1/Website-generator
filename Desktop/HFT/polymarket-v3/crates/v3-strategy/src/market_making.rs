use async_trait::async_trait;
use v3_core::{Market, MarketMakingConfig, MarketState, Outcome, OrderSide, Position};
use v3_signal::SignalSnapshot;
use crate::strategy_trait::{Strategy, StrategyDecision, ExitReason};
use anyhow::Result;
use tracing::{info, warn};

/// Market making strategy - two-sided quoting with inventory management
pub struct MarketMakingStrategy {
    config: MarketMakingConfig,
    current_inventory: i64,
}

impl MarketMakingStrategy {
    pub fn new(config: MarketMakingConfig) -> Self {
        Self {
            config,
            current_inventory: 0,
        }
    }

    fn calculate_quote_prices(&self, signal: &SignalSnapshot) -> (f64, f64) {
        let mid = signal.market_state.mid_price_up;
        let half_spread = self.config.base_spread / 2.0;

        // Adjust for inventory
        let inventory_adjustment = self.current_inventory as f64 * self.config.inventory_skew;

        let bid = (mid - half_spread + inventory_adjustment).clamp(0.01, 0.99);
        let ask = (mid + half_spread + inventory_adjustment).clamp(0.01, 0.99);

        // Round to 2 decimal places (CLOB tick size requirement)
        let bid = (bid * 100.0).round() / 100.0;
        let ask = (ask * 100.0).round() / 100.0;

        (bid, ask)
    }
}

#[async_trait]
impl Strategy for MarketMakingStrategy {
    fn name(&self) -> &str {
        "MarketMaking"
    }

    async fn generate_decision(
        &self,
        _market: &Market,
        signal: &SignalSnapshot,
        position: Option<&Position>,
    ) -> Result<StrategyDecision> {
        // Adversarial cancel: if Binance velocity spike detected, exit immediately
        // to avoid adverse selection on resting limit orders.
        if signal.binance_cancel_signal {
            if position.is_some() {
                info!("⚡ Adversarial cancel: Binance velocity spike, exiting MM position");
                return Ok(StrategyDecision::Exit { reason: ExitReason::Reversal });
            }
            return Ok(StrategyDecision::Hold);
        }

        // Check inventory limits
        if self.current_inventory.abs() >= self.config.max_inventory {
            if let Some(_pos) = position {
                return Ok(StrategyDecision::Exit {
                    reason: ExitReason::RiskLimit,
                });
            }
            return Ok(StrategyDecision::Hold);
        }

        // If we have position, check timeout
        if let Some(pos) = position {
            if pos.hold_duration_secs() >= self.config.order_ttl as i64 {
                return Ok(StrategyDecision::Exit {
                    reason: ExitReason::Timeout,
                });
            }
            return Ok(StrategyDecision::Hold);
        }

        // Generate quotes
        let (bid, ask) = self.calculate_quote_prices(signal);

        // Determine side based on inventory and spread opportunity
        let (outcome, price) = if self.current_inventory > 0 {
            // Long UP inventory → reduce by selling UP at ask
            (Outcome::Up, ask)
        } else if self.current_inventory < 0 {
            // Long DOWN inventory → reduce by selling DOWN at ask
            (Outcome::Down, ask)
        } else {
            // No inventory: quote the wider spread side at bid
            let spread_up = signal.market_state.spread_up;
            let spread_down = signal.market_state.spread_down;
            if spread_up >= spread_down {
                (Outcome::Up, bid)
            } else {
                (Outcome::Down, bid)
            }
        };

        info!("Market making quote: {:?} @ {:.4}", outcome, price);

        // Bug #11 Fix: Validate position size
        let size = self.config.size_per_side;
        if size <= 0 {
            warn!("Invalid position size: {}, skipping", size);
            return Ok(StrategyDecision::Hold);
        }

        Ok(StrategyDecision::Enter {
            outcome,
            side: OrderSide::Buy,
            size,
            price,
            ttl: self.config.order_ttl,
            reason: "Market making".to_string(),
        })
    }

    fn should_exit(&self, position: &Position, _signal: &SignalSnapshot) -> bool {
        position.hold_duration_secs() >= self.config.order_ttl as i64
    }

    fn suitability_score(&self, market_state: &MarketState) -> f64 {
        let mut score: f64 = 0.5; // Base score

        // Wide spreads are good for MM
        if market_state.spread_sum() > 0.05 {
            score += 0.3;
        }

        // Low volatility is good
        if market_state.volatility < 0.01 {
            score += 0.2;
        }

        score.min(1.0)
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}
