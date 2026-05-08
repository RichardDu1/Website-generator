//! Dashboard state types for V3 bot

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Full bot state snapshot
#[derive(Debug, Clone, Serialize)]
pub struct DashboardState {
    pub timestamp: String,
    pub uptime_secs: u64,
    pub dry_run: bool,

    // Risk
    pub daily_pnl: f64,
    pub max_daily_loss: f64,
    pub circuit_breaker_active: bool,
    pub trading_enabled: bool,

    // Positions
    pub positions: Vec<PositionSnapshot>,
    pub total_exposure: f64,
    pub position_count: usize,

    // Orders
    pub orders: Vec<OrderSnapshot>,
    pub active_order_count: usize,

    // System status
    pub scanner_connected: bool,
    pub binance_connected: bool,
    pub clob_connected: bool,
    pub tick_interval_ms: u64,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            uptime_secs: 0,
            dry_run: true,
            daily_pnl: 0.0,
            max_daily_loss: 100.0,
            circuit_breaker_active: false,
            trading_enabled: true,
            positions: vec![],
            total_exposure: 0.0,
            position_count: 0,
            orders: vec![],
            active_order_count: 0,
            scanner_connected: false,
            binance_connected: false,
            clob_connected: false,
            tick_interval_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionSnapshot {
    pub market_id: String,
    pub outcome: String,
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub hold_time_secs: f64,
    pub entry_time: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderSnapshot {
    pub order_id: String,
    pub market_id: String,
    pub side: String,
    pub price: f64,
    pub size: u64,
    pub ttl_secs: u64,
    pub age_secs: f64,
    pub created_at: String,
}

/// Ring buffer of recent events
#[derive(Debug, Clone, Serialize, Default)]
pub struct EventLog {
    pub events: Vec<DashboardEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardEvent {
    pub ts: String,
    pub kind: String,
    pub message: String,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn add(&mut self, kind: String, message: String) {
        let ts = Utc::now().format("%H:%M:%S").to_string();
        self.events.insert(0, DashboardEvent { ts, kind, message });

        // Keep only last 50 events
        if self.events.len() > 50 {
            self.events.truncate(50);
        }
    }
}
