use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use mm_core::Asset;

/// Market representation for 15-minute crypto markets
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Market {
    pub condition_id: String,
    pub token_id_yes: String,
    pub token_id_no: String,
    pub asset: Asset,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open_price: f64,
    pub slug: String,
}

impl Market {
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.open_time && now < self.close_time
    }

    pub fn time_remaining(&self) -> chrono::Duration {
        self.close_time - Utc::now()
    }

    pub fn time_elapsed(&self) -> chrono::Duration {
        Utc::now() - self.open_time
    }
}

/// Position side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,   // Bought YES or NO
    Short,  // Sold YES or NO (rare in prediction markets)
}

/// Outcome type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Up,
    Down,
}

impl Outcome {
    pub fn opposite(&self) -> Self {
        match self {
            Outcome::Up => Outcome::Down,
            Outcome::Down => Outcome::Up,
        }
    }
}

/// Position tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub market: Market,
    pub outcome: Outcome,
    pub side: PositionSide,
    pub entry_price: f64,
    pub size: u64,
    pub entry_time: DateTime<Utc>,
    pub unrealized_pnl: f64,
    pub strategy_name: String,
}

impl Position {
    pub fn hold_duration(&self) -> chrono::Duration {
        Utc::now() - self.entry_time
    }

    pub fn hold_duration_secs(&self) -> i64 {
        self.hold_duration().num_seconds()
    }

    pub fn update_pnl(&mut self, current_price: f64) {
        let price_diff = current_price - self.entry_price;
        self.unrealized_pnl = price_diff * self.size as f64;
    }
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

/// Order with TTL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub market: Market,
    pub outcome: Outcome,
    pub side: OrderSide,
    pub price: Decimal,
    pub size: u64,
    pub created_at: DateTime<Utc>,
    pub ttl_secs: u64,
}

impl Order {
    pub fn is_expired(&self) -> bool {
        let elapsed = (Utc::now() - self.created_at).num_seconds() as u64;
        elapsed >= self.ttl_secs
    }

    pub fn time_remaining(&self) -> i64 {
        let elapsed = (Utc::now() - self.created_at).num_seconds();
        self.ttl_secs as i64 - elapsed
    }
}

/// Market status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketStatus {
    Active,
    Closed,
    Settled,
}

/// Market state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    pub market: Market,
    pub best_bid_up: f64,
    pub best_ask_up: f64,
    pub best_bid_down: f64,
    pub best_ask_down: f64,
    pub spread_up: f64,
    pub spread_down: f64,
    pub mid_price_up: f64,
    pub mid_price_down: f64,
    pub volatility: f64,
    pub binance_price: f64,
    pub binance_velocity: f64,
}

impl MarketState {
    pub fn spread_sum(&self) -> f64 {
        self.spread_up + self.spread_down
    }

    pub fn is_wide_spread(&self) -> bool {
        self.spread_sum() > 0.05 // 5% total spread
    }

    pub fn is_high_volatility(&self) -> bool {
        self.volatility > 0.02 // 2% volatility
    }
}

/// Trade fill event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: String,
    pub market: Market,
    pub outcome: Outcome,
    pub side: OrderSide,
    pub price: f64,
    pub size: u64,
    pub timestamp: DateTime<Utc>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_opposite() {
        assert_eq!(Outcome::Up.opposite(), Outcome::Down);
        assert_eq!(Outcome::Down.opposite(), Outcome::Up);
    }

    #[test]
    fn test_order_side_opposite() {
        assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    }
}
