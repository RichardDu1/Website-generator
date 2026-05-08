use std::collections::HashMap;

use crate::Asset;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Tracks positions across all active markets
pub struct InventoryManager {
    positions: HashMap<String, MarketPosition>, // keyed by market condition_id
    max_per_market: f64,
    max_per_asset: f64,
    max_total: f64,
}

#[derive(Debug, Clone, Default)]
pub struct MarketPosition {
    pub asset: Asset,
    pub condition_id: String,
    pub net_shares: i64,     // + = long YES, - = short YES (long NO)
    pub total_cost: f64,     // cumulative cost basis
    pub realized_pnl: f64,
}

impl Default for Asset {
    fn default() -> Self {
        Asset::BTC
    }
}

impl InventoryManager {
    pub fn new(max_per_market: f64, max_per_asset: f64, max_total: f64) -> Self {
        Self {
            positions: HashMap::new(),
            max_per_market,
            max_per_asset,
            max_total,
        }
    }

    /// Record a fill. `price` is the fill price (0..1), `size` is shares.
    /// For a BUY YES: size positive. For a SELL YES: size negative.
    pub fn record_fill(
        &mut self,
        condition_id: &str,
        asset: Asset,
        size: i64,
        price: Decimal,
    ) {
        let pos = self.positions.entry(condition_id.to_string()).or_insert_with(|| {
            MarketPosition {
                asset,
                condition_id: condition_id.to_string(),
                ..Default::default()
            }
        });
        let price_f = price.to_f64().unwrap_or(0.0);
        pos.net_shares += size;
        pos.total_cost += (size as f64) * price_f;
    }

    /// Record market resolution. `won` = true if YES won.
    pub fn record_resolution(&mut self, condition_id: &str, won: bool) {
        if let Some(pos) = self.positions.get_mut(condition_id) {
            let settlement = if won { 1.0 } else { 0.0 };
            let pnl = (pos.net_shares as f64) * settlement - pos.total_cost;
            pos.realized_pnl += pnl;
            pos.net_shares = 0;
            pos.total_cost = 0.0;
        }
    }

    pub fn position(&self, condition_id: &str) -> i64 {
        self.positions.get(condition_id).map_or(0, |p| p.net_shares)
    }

    /// Total exposure for a single market (absolute value in dollars)
    pub fn market_exposure(&self, condition_id: &str, mid_price: f64) -> f64 {
        let shares = self.position(condition_id).abs() as f64;
        shares * mid_price
    }

    /// Total exposure for one asset across all its active markets
    pub fn asset_exposure(&self, asset: Asset, mid_price: f64) -> f64 {
        self.positions
            .values()
            .filter(|p| p.asset == asset && p.net_shares != 0)
            .map(|p| (p.net_shares.abs() as f64) * mid_price)
            .sum()
    }

    /// Total exposure across all assets
    pub fn total_exposure(&self) -> f64 {
        self.positions
            .values()
            .filter(|p| p.net_shares != 0)
            .map(|p| (p.net_shares.abs() as f64) * 0.5) // approximate at mid=0.5
            .sum()
    }

    /// Check if we can take a new position in this market
    pub fn can_open(&self, condition_id: &str, asset: Asset, mid_price: f64) -> PositionVerdict {
        let market_exp = self.market_exposure(condition_id, mid_price);
        if market_exp >= self.max_per_market {
            return PositionVerdict::ReduceOnly;
        }
        let asset_exp = self.asset_exposure(asset, mid_price);
        if asset_exp >= self.max_per_asset {
            return PositionVerdict::Reject;
        }
        if self.total_exposure() >= self.max_total {
            return PositionVerdict::Reject;
        }
        PositionVerdict::Allow
    }

    /// Total realized P&L across all markets
    pub fn total_realized_pnl(&self) -> f64 {
        self.positions.values().map(|p| p.realized_pnl).sum()
    }

    /// Remove settled markets (zero position + has realized pnl)
    pub fn cleanup_settled(&mut self) {
        self.positions.retain(|_, p| p.net_shares != 0);
    }

    pub fn max_total(&self) -> f64 {
        self.max_total
    }

    /// Get the total cost basis for a market position (for P&L calculation at settlement)
    pub fn cost_basis(&self, condition_id: &str) -> f64 {
        self.positions.get(condition_id).map_or(0.0, |p| p.total_cost)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionVerdict {
    Allow,
    ReduceOnly,
    Reject,
}

