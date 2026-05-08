//! Gengar-style directional latency arbitrage: pure functions.
//!
//! Workflow:
//!   1. `estimate_true_probability(delta_pct, sec_rem, vol)` — Brownian reach model
//!   2. `kelly_bet_size(prob, market_price, bankroll, ...)` — Quarter-Kelly
//!   3. `evaluate(...)` — 4-gate filter (time window / min delta / min prob / min edge)
//!   4. `should_lock_profit(initial_edge, current_edge, ratio)` — dynamic exit

use serde::{Deserialize, Serialize};
use statrs::function::erf::erf;

/// Gengar strategy parameters (defaults mirror gengar v13 recalibration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GengarConfig {
    pub enabled: bool,
    /// Minimum `prob - market_price` required to enter. Default 0.05 (5%).
    pub min_edge: f64,
    /// Minimum Brownian probability estimate. Default 0.80.
    pub min_prob: f64,
    /// Don't enter earlier than this far from resolution (prevents too-early signal).
    pub entry_window_start_secs: u64,
    /// Don't enter later than this close to resolution.
    pub entry_window_end_secs: u64,
    pub max_price: f64,
    pub min_price: f64,
    /// Minimum |btc_delta_pct| to even consider (Binance vs oracle can disagree below this).
    pub min_btc_delta_pct: f64,
    /// Kelly safety factor (0.25 = Quarter-Kelly).
    pub kelly_fraction: f64,
    /// Polymarket minimum notional. Orders below this are skipped.
    pub min_bet_usdc: f64,
    /// Hard cap per trade.
    pub max_bet_usdc: f64,
    /// Profit-lock exit ratio: if `current_edge < initial_edge * ratio` → close.
    pub profit_lock_ratio: f64,
    /// Used when the per-window VolTracker isn't full yet.
    pub static_vol_fallback: f64,
}

impl Default for GengarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_edge: 0.05,
            min_prob: 0.80,
            entry_window_start_secs: 240,
            entry_window_end_secs: 10,
            max_price: 0.90,
            min_price: 0.50,
            min_btc_delta_pct: 0.06,
            kelly_fraction: 0.25,
            min_bet_usdc: 5.0,
            max_bet_usdc: 25.0,
            profit_lock_ratio: 0.30,
            static_vol_fallback: 0.12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub side: Side,
    pub confidence: f64,
    pub btc_delta_pct: f64,
    pub market_price: f64,
    pub edge: f64,
    pub true_prob: f64,
    pub seconds_remaining: f64,
    pub kelly_size: f64,
}

/// Brownian motion reach probability.
///
/// ```text
/// time_factor   = max(sec_rem, 1) / 300
/// effective_vol = vol * sqrt(time_factor)
/// z             = |delta_pct| / effective_vol
/// prob          = 0.5 * (1 + erf(z / sqrt(2)))
/// ```
pub fn estimate_true_probability(
    btc_delta_pct: f64,
    seconds_remaining: f64,
    vol: f64,
) -> f64 {
    let time_factor = seconds_remaining.max(1.0) / 300.0;
    let effective_vol = vol * time_factor.sqrt();
    if effective_vol == 0.0 {
        return if btc_delta_pct > 0.0 { 0.99 } else { 0.01 };
    }
    let z = btc_delta_pct.abs() / effective_vol;
    let prob = 0.5 * (1.0 + erf(z / 2f64.sqrt()));
    prob.clamp(0.01, 0.99)
}

/// Quarter-Kelly bet size (USDC). Returns 0 when the edge is non-positive,
/// when Kelly-f suggests below `min_bet`, or when Kelly-f is ≤ 0.
pub fn kelly_bet_size(
    true_prob: f64,
    market_price: f64,
    bankroll: f64,
    fraction: f64,
    min_bet: f64,
    max_bet: f64,
) -> f64 {
    if true_prob <= market_price || market_price <= 0.0 || market_price >= 1.0 {
        return 0.0;
    }
    let b = (1.0 - market_price) / market_price;
    let q = 1.0 - true_prob;
    let kelly_f = (b * true_prob - q) / b;
    if kelly_f <= 0.0 {
        return 0.0;
    }
    let bet = bankroll * kelly_f * fraction;
    if bet < min_bet {
        return 0.0;
    }
    bet.min(max_bet)
}

/// 4-gate evaluation. Returns `Some(TradeSignal)` when all gates pass.
pub fn evaluate(
    btc_price: f64,
    opening_price: f64,
    up_market_price: f64,
    down_market_price: f64,
    seconds_remaining: f64,
    bankroll: f64,
    config: &GengarConfig,
    vol: f64,
) -> Option<TradeSignal> {
    // Gate 1: time window
    if seconds_remaining > config.entry_window_start_secs as f64 {
        return None;
    }
    if seconds_remaining < config.entry_window_end_secs as f64 {
        return None;
    }
    if opening_price <= 0.0 {
        return None;
    }

    let btc_delta_pct = (btc_price - opening_price) / opening_price * 100.0;

    // Gate 2: min delta
    if btc_delta_pct.abs() < config.min_btc_delta_pct {
        return None;
    }

    let side = if btc_delta_pct > 0.0 { Side::Up } else { Side::Down };
    let market_price = if btc_delta_pct > 0.0 { up_market_price } else { down_market_price };

    if market_price > config.max_price || market_price < config.min_price {
        return None;
    }

    let true_prob = estimate_true_probability(btc_delta_pct, seconds_remaining, vol);

    // Gate 3: model confidence
    if true_prob < config.min_prob {
        return None;
    }

    // Gate 4: edge
    let edge = true_prob - market_price;
    if edge < config.min_edge {
        return None;
    }

    let kelly_size = kelly_bet_size(
        true_prob,
        market_price,
        bankroll,
        config.kelly_fraction,
        config.min_bet_usdc,
        config.max_bet_usdc,
    );
    if kelly_size <= 0.0 {
        return None;
    }

    Some(TradeSignal {
        side,
        confidence: (edge / 0.10).min(1.0),
        btc_delta_pct,
        market_price,
        edge,
        true_prob,
        seconds_remaining,
        kelly_size,
    })
}

/// Dynamic profit lock: fire when current_edge decays below initial_edge × ratio.
pub fn should_lock_profit(initial_edge: f64, current_edge: f64, lock_ratio: f64) -> bool {
    if initial_edge <= 0.0 {
        return false;
    }
    current_edge < initial_edge * lock_ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_at_zero_delta_is_half() {
        let p = estimate_true_probability(0.0, 300.0, 0.12);
        assert!((p - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn probability_clamped_upper_bound() {
        // Very large delta should saturate at 0.99 (the clamp ceiling)
        let p = estimate_true_probability(100.0, 300.0, 0.12);
        assert_eq!(p, 0.99);
    }

    #[test]
    fn probability_large_delta_large_prob() {
        let p = estimate_true_probability(0.2, 60.0, 0.12);
        assert!(p > 0.75, "got {p}");
    }

    #[test]
    fn kelly_positive_when_edge_positive() {
        let bet = kelly_bet_size(0.85, 0.65, 100.0, 0.25, 5.0, 25.0);
        assert!(bet > 5.0 && bet <= 25.0, "got {bet}");
    }

    #[test]
    fn kelly_zero_when_no_edge() {
        let bet = kelly_bet_size(0.50, 0.55, 100.0, 0.25, 5.0, 25.0);
        assert_eq!(bet, 0.0);
    }

    #[test]
    fn kelly_zero_when_tiny_bankroll() {
        // Small edge + small bankroll → bet < min_bet → 0
        let bet = kelly_bet_size(0.60, 0.55, 10.0, 0.25, 5.0, 25.0);
        assert_eq!(bet, 0.0);
    }

    #[test]
    fn kelly_capped_at_max_bet() {
        let bet = kelly_bet_size(0.95, 0.50, 10_000.0, 0.25, 5.0, 25.0);
        assert_eq!(bet, 25.0);
    }

    #[test]
    fn evaluate_skips_outside_entry_window_too_early() {
        let cfg = GengarConfig::default();
        // sec_rem > entry_window_start (240)
        let r = evaluate(100_150.0, 100_000.0, 0.60, 0.40, 1000.0, 100.0, &cfg, 0.12);
        assert!(r.is_none());
    }

    #[test]
    fn evaluate_skips_outside_entry_window_too_late() {
        let cfg = GengarConfig::default();
        // sec_rem < entry_window_end (10)
        let r = evaluate(100_150.0, 100_000.0, 0.60, 0.40, 5.0, 100.0, &cfg, 0.12);
        assert!(r.is_none());
    }

    #[test]
    fn evaluate_skips_tiny_delta() {
        let cfg = GengarConfig::default();
        // delta = 0.001%, below 0.06%
        let r = evaluate(100_001.0, 100_000.0, 0.60, 0.40, 120.0, 100.0, &cfg, 0.12);
        assert!(r.is_none());
    }

    #[test]
    fn evaluate_skips_when_edge_too_small() {
        let cfg = GengarConfig::default();
        // true_prob ~ 0.904 for (delta=0.1%, sec_rem=120, vol=0.12)
        // If market_price = 0.88, edge = 0.024 < 0.05 → skip
        let r = evaluate(100_100.0, 100_000.0, 0.88, 0.12, 120.0, 100.0, &cfg, 0.12);
        assert!(r.is_none(), "expected skip, got {r:?}");
    }

    #[test]
    fn evaluate_skips_when_price_out_of_range() {
        let cfg = GengarConfig::default();
        // UP price 0.95 > max_price 0.90
        let r = evaluate(100_150.0, 100_000.0, 0.95, 0.05, 120.0, 100.0, &cfg, 0.12);
        assert!(r.is_none());
    }

    #[test]
    fn evaluate_returns_signal_when_all_pass() {
        let cfg = GengarConfig::default();
        // delta=0.15%, sec_rem=120, vol=0.12 → big prob; UP=0.65 → edge >= 0.05
        let r = evaluate(100_150.0, 100_000.0, 0.65, 0.35, 120.0, 100.0, &cfg, 0.12);
        assert!(r.is_some(), "expected signal");
        let sig = r.unwrap();
        assert_eq!(sig.side, Side::Up);
        assert!(sig.edge > 0.05);
        assert!(sig.kelly_size >= 5.0 && sig.kelly_size <= 25.0);
    }

    #[test]
    fn evaluate_returns_down_when_negative_delta() {
        let cfg = GengarConfig::default();
        let r = evaluate(99_850.0, 100_000.0, 0.35, 0.65, 120.0, 100.0, &cfg, 0.12);
        assert!(r.is_some());
        let sig = r.unwrap();
        assert_eq!(sig.side, Side::Down);
    }

    #[test]
    fn lock_profit_when_edge_decayed_below_ratio() {
        // initial=0.20, current=0.05 → 25% < 30%
        assert!(should_lock_profit(0.20, 0.05, 0.30));
    }

    #[test]
    fn lock_profit_not_triggered_when_edge_still_healthy() {
        // initial=0.20, current=0.08 → 40% > 30%
        assert!(!should_lock_profit(0.20, 0.08, 0.30));
    }

    #[test]
    fn lock_profit_ignores_non_positive_initial() {
        assert!(!should_lock_profit(0.0, -0.1, 0.30));
    }
}
