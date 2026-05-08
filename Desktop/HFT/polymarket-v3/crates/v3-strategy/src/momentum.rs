use async_trait::async_trait;
use v3_core::{Market, MarketState, MomentumConfig, Outcome, OrderSide, Position};
use v3_signal::SignalSnapshot;
use crate::strategy_trait::{Strategy, StrategyDecision, ExitReason};
use anyhow::Result;
use tracing::{debug, info, warn};

/// Momentum strategy — uses order book imbalance as primary entry signal.
/// Based on distinct-baguette's actual approach: enter when bid/ask depth
/// imbalance exceeds threshold, then exit quickly (6-30s TTL).
pub struct MomentumStrategy {
    config: MomentumConfig,
}

impl MomentumStrategy {
    pub fn new(config: MomentumConfig) -> Self {
        Self { config }
    }

    /// Check if order book imbalance is strong enough to enter.
    /// Imbalance ratio is in [-1, 1]. Requires |imbalance| >= 0.25.
    fn is_strong_imbalance(&self, signal: &SignalSnapshot) -> bool {
        signal.order_book_imbalance.abs() >= 0.25
    }

    /// Determine entry direction from order book imbalance.
    /// Positive imbalance (more bids) → buy UP (price likely to rise).
    /// Negative imbalance (more asks) → buy DOWN (price likely to fall).
    fn imbalance_outcome(&self, signal: &SignalSnapshot) -> Option<Outcome> {
        let imbalance = signal.order_book_imbalance;
        let mid_up = signal.market_state.mid_price_up;
        let mid_down = signal.market_state.mid_price_down;

        if imbalance >= 0.25 {
            if mid_up >= 0.01 && mid_up <= 0.99 {
                info!("📊 Imbalance UP entry: imbalance={:.3}, mid_up={:.4}", imbalance, mid_up);
                Some(Outcome::Up)
            } else {
                None
            }
        } else if imbalance <= -0.25 {
            if mid_down >= 0.01 && mid_down <= 0.99 {
                info!("📊 Imbalance DOWN entry: imbalance={:.3}, mid_down={:.4}", imbalance, mid_down);
                Some(Outcome::Down)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Calculate entry price (aggressive taker — use ask price).
    fn calculate_entry_price(&self, signal: &SignalSnapshot, outcome: Outcome) -> f64 {
        let price = match outcome {
            Outcome::Up => signal.market_state.best_ask_up,
            Outcome::Down => signal.market_state.best_ask_down,
        };
        (price * 100.0).round() / 100.0
    }

    /// Calculate position size in shares.
    /// distinct-baguette avg $4-7 per trade. We target $5 base.
    fn calculate_position_size(&self, price: f64, imbalance_strength: f64) -> u64 {
        let base_usd = 5.0_f64;
        let max_usd = 10.0_f64;
        // Scale up slightly with imbalance strength
        let strength = (imbalance_strength.abs() - 0.25) / 0.75; // 0.0 at threshold, 1.0 at max
        let size_usd = base_usd + (max_usd - base_usd) * strength.clamp(0.0, 1.0);
        let shares = (size_usd / price).max(1.0);
        shares as u64
    }

    fn is_timeout(&self, position: &Position) -> bool {
        position.hold_duration_secs() >= self.config.max_hold_time as i64
    }

    fn is_stop_loss(&self, position: &Position) -> bool {
        let cost = position.entry_price * position.size as f64;
        if cost == 0.0 { return false; }
        let loss_pct = position.unrealized_pnl / cost;
        loss_pct <= -self.config.stop_loss_pct
    }

    fn is_imbalance_reversed(&self, position: &Position, signal: &SignalSnapshot) -> bool {
        let imbalance = signal.order_book_imbalance;
        match position.outcome {
            Outcome::Up => imbalance <= -0.15, // was bullish, now bearish
            Outcome::Down => imbalance >= 0.15, // was bearish, now bullish
        }
    }
}

#[async_trait]
impl Strategy for MomentumStrategy {
    fn name(&self) -> &str {
        "ImbalanceMomentum"
    }

    async fn generate_decision(
        &self,
        _market: &Market,
        signal: &SignalSnapshot,
        position: Option<&Position>,
    ) -> Result<StrategyDecision> {
        // --- EXIT LOGIC ---
        if let Some(pos) = position {
            if self.is_timeout(pos) {
                info!("⏰ Momentum timeout exit: held {}s", pos.hold_duration_secs());
                return Ok(StrategyDecision::Exit { reason: ExitReason::Timeout });
            }
            if self.is_stop_loss(pos) {
                info!("🛑 Momentum stop-loss exit: pnl={:.4}", pos.unrealized_pnl);
                return Ok(StrategyDecision::Exit { reason: ExitReason::StopLoss });
            }
            if self.is_imbalance_reversed(pos, signal) {
                info!("🔄 Imbalance reversed, exiting: imbalance={:.3}", signal.order_book_imbalance);
                return Ok(StrategyDecision::Exit { reason: ExitReason::Reversal });
            }
            return Ok(StrategyDecision::Hold);
        }

        // --- ENTRY LOGIC ---
        if !self.config.enabled {
            return Ok(StrategyDecision::Hold);
        }

        // Skip if Binance cancel signal is active (adverse conditions)
        if signal.binance_cancel_signal {
            debug!("Skipping momentum entry: Binance cancel signal active");
            return Ok(StrategyDecision::Hold);
        }

        if !self.is_strong_imbalance(signal) {
            debug!("Imbalance too weak: {:.3}", signal.order_book_imbalance);
            return Ok(StrategyDecision::Hold);
        }

        let outcome = match self.imbalance_outcome(signal) {
            Some(o) => o,
            None => return Ok(StrategyDecision::Hold),
        };

        let price = self.calculate_entry_price(signal, outcome);
        if price <= 0.01 || price >= 0.99 {
            debug!("Invalid entry price: {:.4}", price);
            return Ok(StrategyDecision::Hold);
        }

        let size = self.calculate_position_size(price, signal.order_book_imbalance);
        if size == 0 {
            warn!("Zero position size, skipping");
            return Ok(StrategyDecision::Hold);
        }

        info!(
            "✅ Imbalance entry: {:?} @ {:.4}, size={}, imbalance={:.3}, bid_depth={:.2}, ask_depth={:.2}",
            outcome, price, size, signal.order_book_imbalance, signal.bid_depth_3, signal.ask_depth_3
        );

        Ok(StrategyDecision::Enter {
            outcome,
            side: OrderSide::Buy,
            size,
            price,
            ttl: self.config.max_hold_time.min(30), // Max 30s TTL
            reason: format!("imbalance={:.3}", signal.order_book_imbalance),
        })
    }

    fn should_exit(&self, position: &Position, signal: &SignalSnapshot) -> bool {
        self.is_timeout(position)
            || self.is_stop_loss(position)
            || self.is_imbalance_reversed(position, signal)
    }

    fn suitability_score(&self, _market_state: &MarketState) -> f64 {
        0.5
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v3_core::{Market, MarketState, MomentumConfig};
    use v3_signal::signal_types::MomentumSignalType;

    fn create_test_market() -> Market {
        Market::default()
    }

    fn create_test_signal(imbalance: f64) -> SignalSnapshot {
        SignalSnapshot {
            market: Market::default(),
            market_state: MarketState {
                market: Market::default(),
                best_bid_up: 0.48,
                best_ask_up: 0.52,
                best_bid_down: 0.48,
                best_ask_down: 0.52,
                spread_up: 0.04,
                spread_down: 0.04,
                mid_price_up: 0.50,
                mid_price_down: 0.50,
                volatility: 0.02,
                binance_price: 100000.0,
                binance_velocity: 0.0,
            },
            binance_price: 100000.0,
            binance_velocity: 0.0,
            binance_acceleration: 0.0,
            momentum_signal: MomentumSignalType::Neutral,
            volatility: 0.02,
            has_arbitrage: false,
            order_book_imbalance: imbalance,
            bid_depth_3: 100.0,
            ask_depth_3: 50.0,
            spread_capture_margin: 0.0,
            binance_cancel_signal: false,
        }
    }

    #[tokio::test]
    async fn test_imbalance_entry_up() {
        let config = MomentumConfig::default();
        let strategy = MomentumStrategy::new(config);
        let signal = create_test_signal(0.5); // Strong buy imbalance
        let decision = strategy.generate_decision(&create_test_market(), &signal, None).await.unwrap();
        assert!(decision.is_entry());
    }

    #[tokio::test]
    async fn test_imbalance_entry_down() {
        let config = MomentumConfig::default();
        let strategy = MomentumStrategy::new(config);
        let signal = create_test_signal(-0.5); // Strong sell imbalance
        let decision = strategy.generate_decision(&create_test_market(), &signal, None).await.unwrap();
        assert!(decision.is_entry());
    }

    #[tokio::test]
    async fn test_no_entry_weak_imbalance() {
        let config = MomentumConfig::default();
        let strategy = MomentumStrategy::new(config);
        let signal = create_test_signal(0.1); // Weak imbalance
        let decision = strategy.generate_decision(&create_test_market(), &signal, None).await.unwrap();
        assert!(decision.is_hold());
    }

    #[tokio::test]
    async fn test_no_entry_cancel_signal() {
        let config = MomentumConfig::default();
        let strategy = MomentumStrategy::new(config);
        let mut signal = create_test_signal(0.5);
        signal.binance_cancel_signal = true; // Adverse Binance move
        let decision = strategy.generate_decision(&create_test_market(), &signal, None).await.unwrap();
        assert!(decision.is_hold());
    }
}
