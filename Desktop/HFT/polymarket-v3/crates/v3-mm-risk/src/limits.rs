//! Risk management — position limits, kill switch, feed staleness

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tracing::{error, info, warn};

pub struct RiskManager {
    config: RiskConfig,
    daily_realized_pnl: f64,
    killed: AtomicBool,
    last_rtds_ts: Instant,
    last_binance_ts: Instant,
    last_action: RiskAction,
}

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_daily_loss: f64,
    pub rtds_stale_timeout: Duration,
    pub binance_stale_timeout: Duration,
    pub max_open_orders: usize,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_daily_loss: 200.0,
            rtds_stale_timeout: Duration::from_millis(2000),
            binance_stale_timeout: Duration::from_millis(5000),
            max_open_orders: 40,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskAction {
    /// All clear, proceed normally
    Normal,
    /// Widen spreads (Binance feed unreliable)
    WidenSpread,
    /// Cancel all quotes immediately (RTDS feed lost or daily loss hit)
    CancelAll,
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            daily_realized_pnl: 0.0,
            killed: AtomicBool::new(false),
            last_rtds_ts: now,
            last_binance_ts: now,
            last_action: RiskAction::Normal,
        }
    }

    /// Record realized P&L from a settlement
    pub fn record_pnl(&mut self, pnl: f64) {
        self.daily_realized_pnl += pnl;
        if self.daily_realized_pnl <= -self.config.max_daily_loss {
            error!(
                "KILL SWITCH: daily loss {:.2} exceeds limit {:.2}",
                self.daily_realized_pnl, self.config.max_daily_loss
            );
            self.killed.store(true, Ordering::SeqCst);
        }
    }

    /// Update RTDS feed timestamp
    pub fn touch_rtds(&mut self) {
        self.last_rtds_ts = Instant::now();
    }

    /// Update Binance feed timestamp
    pub fn touch_binance(&mut self) {
        self.last_binance_ts = Instant::now();
    }

    /// Check overall risk state (only logs on state transitions)
    pub fn check(&mut self) -> RiskAction {
        if self.killed.load(Ordering::SeqCst) {
            return RiskAction::CancelAll;
        }

        let now = Instant::now();
        let rtds_stale = now.duration_since(self.last_rtds_ts) > self.config.rtds_stale_timeout;
        let binance_stale = now.duration_since(self.last_binance_ts) > self.config.binance_stale_timeout;

        let action = if rtds_stale {
            RiskAction::CancelAll
        } else if binance_stale {
            RiskAction::WidenSpread
        } else {
            RiskAction::Normal
        };

        // Only log on state change
        if action != self.last_action {
            match action {
                RiskAction::CancelAll => warn!("Risk: RTDS feed stale, cancelling all quotes"),
                RiskAction::WidenSpread => warn!("Risk: Binance feed stale, widening spreads"),
                RiskAction::Normal => info!("Risk: feeds recovered, resuming normal"),
            }
            self.last_action = action;
        }

        action
    }

    pub fn is_killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }

    pub fn daily_pnl(&self) -> f64 {
        self.daily_realized_pnl
    }

    pub fn rtds_age(&self) -> Duration {
        self.last_rtds_ts.elapsed()
    }

    pub fn binance_age(&self) -> Duration {
        self.last_binance_ts.elapsed()
    }

    pub fn last_action(&self) -> RiskAction {
        self.last_action
    }

    pub fn max_daily_loss(&self) -> f64 {
        self.config.max_daily_loss
    }

    /// Reset daily counters (call at UTC midnight)
    pub fn reset_daily(&mut self) {
        self.daily_realized_pnl = 0.0;
        self.killed.store(false, Ordering::SeqCst);
    }
}
