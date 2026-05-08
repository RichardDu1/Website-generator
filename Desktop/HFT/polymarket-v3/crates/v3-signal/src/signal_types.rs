use serde::{Deserialize, Serialize};

/// Which window kind the signal belongs to (kept as lightweight tag here to
/// avoid v3-signal → v3-bot circular dep; the real `WindowKind` with methods
/// lives in v3-bot::window_manager).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    M5,
    M15,
    H1,
}

/// Gengar-style per-tick signal snapshot. All fields are consumed by
/// `v3_strategy::gengar::evaluate`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalSnapshot {
    pub window_kind: Option<WindowKind>,
    /// Latest Binance BTC spot price.
    pub binance_price: f64,
    /// The BTC price captured at the window's open (anchors delta).
    pub opening_price: f64,
    /// (binance_price - opening_price) / opening_price * 100
    pub btc_delta_pct: f64,
    /// Seconds until the window resolves.
    pub seconds_remaining: f64,
    /// Realized volatility from the window's VolTracker (falls back to
    /// `static_vol_fallback` when the tracker is below capacity).
    pub realized_vol: f64,
    /// Best ask for UP token (what we'd pay to buy UP).
    pub up_market_price: f64,
    /// Best ask for DOWN token.
    pub down_market_price: f64,
}
