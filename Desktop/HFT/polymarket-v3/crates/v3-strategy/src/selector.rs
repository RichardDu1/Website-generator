use v3_core::MarketState;
use crate::strategy_trait::Strategy;
use crate::{MomentumStrategy, MarketMakingStrategy, SpreadCaptureStrategy};
use std::sync::Arc;
use tracing::info;

/// Strategy selector - chooses best strategy based on market conditions
pub struct StrategySelector {
    momentum: Arc<MomentumStrategy>,
    market_making: Arc<MarketMakingStrategy>,
    spread_capture: Arc<SpreadCaptureStrategy>,
}

impl StrategySelector {
    pub fn new(
        momentum: Arc<MomentumStrategy>,
        market_making: Arc<MarketMakingStrategy>,
        spread_capture: Arc<SpreadCaptureStrategy>,
    ) -> Self {
        Self {
            momentum,
            market_making,
            spread_capture,
        }
    }

    /// Select the best strategy for current market state
    pub fn select_strategy(&self, market_state: &MarketState) -> Arc<dyn Strategy> {
        let mut best_score = -1.0_f64;
        let mut best: Arc<dyn Strategy> = self.momentum.clone();

        let candidates: [Arc<dyn Strategy>; 3] = [
            self.momentum.clone(),
            self.market_making.clone(),
            self.spread_capture.clone(),
        ];

        for strategy in &candidates {
            if !strategy.is_enabled() {
                continue;
            }
            let score = strategy.suitability_score(market_state);
            if score > best_score {
                best_score = score;
                best = strategy.clone();
            }
        }

        info!("Selected strategy: {} (score: {:.2})", best.name(), best_score);
        best
    }

    pub fn momentum(&self) -> Arc<MomentumStrategy> {
        self.momentum.clone()
    }

    pub fn market_making(&self) -> Arc<MarketMakingStrategy> {
        self.market_making.clone()
    }

    pub fn spread_capture(&self) -> Arc<SpreadCaptureStrategy> {
        self.spread_capture.clone()
    }

    // Aliases for orchestrator compatibility
    pub fn get_momentum_strategy(&self) -> Arc<MomentumStrategy> {
        self.momentum.clone()
    }

    pub fn get_market_making_strategy(&self) -> Arc<MarketMakingStrategy> {
        self.market_making.clone()
    }

    pub fn get_arbitrage_strategy(&self) -> Arc<SpreadCaptureStrategy> {
        self.spread_capture.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v3_core::{MomentumConfig, MarketMakingConfig, SpreadCaptureConfig, MarketState, Market};

    fn create_test_market_state(volatility: f64, velocity: f64) -> MarketState {
        MarketState {
            market: Market::default(),
            best_bid_up: 0.48,
            best_ask_up: 0.52,
            best_bid_down: 0.48,
            best_ask_down: 0.52,
            spread_up: 0.04,
            spread_down: 0.04,
            mid_price_up: 0.50,
            mid_price_down: 0.50,
            volatility,
            binance_price: 100000.0,
            binance_velocity: velocity,
        }
    }

    #[test]
    fn test_strategy_selection() {
        let momentum = Arc::new(MomentumStrategy::new(MomentumConfig::default()));
        let market_making = Arc::new(MarketMakingStrategy::new(MarketMakingConfig::default()));
        let spread_capture = Arc::new(SpreadCaptureStrategy::new(SpreadCaptureConfig::default()));

        let selector = StrategySelector::new(momentum, market_making, spread_capture);

        // Selector should return a valid strategy (imbalance scoring requires SignalSnapshot,
        // which isn't available here — MarketState-based scoring is approximate)
        let state = create_test_market_state(0.03, 1.0);
        let strategy = selector.select_strategy(&state);
        let name = strategy.name();
        assert!(name == "ImbalanceMomentum" || name == "MarketMaking" || name == "SpreadCapture",
            "unexpected strategy: {}", name);

        // Wide spread + low volatility: MarketMaking should score highest
        let mut wide_state = create_test_market_state(0.005, 0.0);
        wide_state.spread_up = 0.08;
        wide_state.spread_down = 0.08;
        let strategy = selector.select_strategy(&wide_state);
        assert_eq!(strategy.name(), "MarketMaking");
    }
}
