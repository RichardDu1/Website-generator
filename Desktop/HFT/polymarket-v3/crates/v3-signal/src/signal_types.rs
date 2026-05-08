use serde::{Deserialize, Serialize};
use v3_core::{Market, MarketState};

/// Complete signal snapshot for decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSnapshot {
    pub market: Market,
    pub market_state: MarketState,
    pub binance_price: f64,
    pub binance_velocity: f64,
    pub binance_acceleration: f64,
    pub momentum_signal: MomentumSignalType,
    pub volatility: f64,
    pub has_arbitrage: bool,

    /// Order book imbalance ratio: (bid_vol - ask_vol) / (bid_vol + ask_vol)
    /// Range [-1.0, 1.0]. Positive = more bids (buy pressure), negative = more asks (sell pressure).
    pub order_book_imbalance: f64,

    /// Sum of top-3 bid levels in USDC (depth on buy side)
    pub bid_depth_3: f64,

    /// Sum of top-3 ask levels in USDC (depth on sell side)
    pub ask_depth_3: f64,

    /// Spread capture margin: 1.0 - (best_yes_ask + best_no_ask)
    /// Positive value means risk-neutral arbitrage profit is available.
    pub spread_capture_margin: f64,

    /// True when Binance velocity spike detected — MM orders should be cancelled immediately.
    pub binance_cancel_signal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MomentumSignalType {
    StrongUp,
    WeakUp,
    Neutral,
    WeakDown,
    StrongDown,
}

impl MomentumSignalType {
    pub fn is_bullish(&self) -> bool {
        matches!(self, MomentumSignalType::StrongUp | MomentumSignalType::WeakUp)
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, MomentumSignalType::StrongDown | MomentumSignalType::WeakDown)
    }

    pub fn is_strong(&self) -> bool {
        matches!(self, MomentumSignalType::StrongUp | MomentumSignalType::StrongDown)
    }
}

impl From<crate::MomentumSignal> for MomentumSignalType {
    fn from(signal: crate::MomentumSignal) -> Self {
        match signal {
            crate::MomentumSignal::StrongUp => MomentumSignalType::StrongUp,
            crate::MomentumSignal::WeakUp => MomentumSignalType::WeakUp,
            crate::MomentumSignal::Neutral => MomentumSignalType::Neutral,
            crate::MomentumSignal::WeakDown => MomentumSignalType::WeakDown,
            crate::MomentumSignal::StrongDown => MomentumSignalType::StrongDown,
        }
    }
}
