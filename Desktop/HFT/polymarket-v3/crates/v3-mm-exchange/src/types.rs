use chrono::{DateTime, Utc};
use v3_mm_core::Asset;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ── RTDS (Chainlink price) ──

#[derive(Debug, Clone)]
pub struct RtdsPrice {
    pub asset: Asset,
    pub price: f64,
    pub ts: DateTime<Utc>,
}

// ── Binance ──

#[derive(Debug, Clone)]
pub struct BinanceTick {
    pub asset: Asset,
    pub bid: f64,
    pub ask: f64,
    pub ts: DateTime<Utc>,
}

// ── Polymarket CLOB ──

#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub condition_id: String,
    pub token_id_yes: String,
    pub token_id_no: String,
    pub asset: Asset,
    pub end_time: DateTime<Utc>,
    pub question: String,
}

#[derive(Debug, Clone)]
pub struct BookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone)]
pub struct BookSnapshot {
    pub token_id: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: String,
    pub condition_id: String,
    pub asset: Asset,
    pub side: FillSide,
    pub price: Decimal,
    pub size: u64,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FillSide {
    BuyYes,
    SellYes,
    BuyNo,
    SellNo,
}

#[derive(Debug, Clone)]
pub struct MarketResolved {
    pub condition_id: String,
    pub asset: Asset,
    pub winner: Outcome,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Yes,
    No,
}

// ── Order side for our order client ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

// ── Events sent to orchestrator ──

#[derive(Debug, Clone)]
pub enum ExchangeEvent {
    RtdsPrice(RtdsPrice),
    RtdsAlive,
    BinanceTick(BinanceTick),
    BookUpdate(BookSnapshot),
    OrderFill(Fill),
    MarketResolved(MarketResolved),
    MarketReady(MarketInfo),
    FeedStale { source: &'static str },
    RedeemComplete { condition_id: String, success: bool },
}
