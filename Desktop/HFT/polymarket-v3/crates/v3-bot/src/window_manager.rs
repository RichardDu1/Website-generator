//! Polymarket 5m / 15m / 1h BTC UP/DOWN window math.
//!
//! Polymarket uses deterministic slugs: `btc-updown-{5m|15m|1h}-{window_ts}`
//! where `window_ts = now - (now % period)`. This module encodes that math
//! so we don't have to keep string-formatting slugs by hand.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    M5,
    M15,
    H1,
}

impl WindowKind {
    pub fn period_secs(&self) -> u64 {
        match self {
            WindowKind::M5 => 300,
            WindowKind::M15 => 900,
            WindowKind::H1 => 3600,
        }
    }

    pub fn slug_prefix(&self) -> &'static str {
        match self {
            WindowKind::M5 => "5m",
            WindowKind::M15 => "15m",
            WindowKind::H1 => "1h",
        }
    }

    /// Start-of-window timestamp for any `now_secs` (UTC epoch seconds).
    pub fn current_window_ts(&self, now_secs: i64) -> u64 {
        let p = self.period_secs() as i64;
        (now_secs - now_secs.rem_euclid(p)) as u64
    }

    /// Polymarket Gamma slug for a specific window ts.
    pub fn slug(&self, window_ts: u64) -> String {
        format!("btc-updown-{}-{}", self.slug_prefix(), window_ts)
    }

    /// Remaining seconds in the window. Can be negative if `now_secs` has
    /// already crossed the window end; callers should clamp to 0 if needed.
    pub fn seconds_remaining(&self, window_ts: u64, now_secs: i64) -> i64 {
        (window_ts as i64 + self.period_secs() as i64) - now_secs
    }

    pub fn all() -> [WindowKind; 3] {
        [WindowKind::M5, WindowKind::M15, WindowKind::H1]
    }
}

/// Per-window state owned by `WindowManager`.
#[derive(Debug, Clone)]
pub struct WindowState {
    pub kind: WindowKind,
    pub window_ts: u64,
    pub slug: String,
    pub condition_id: String,
    pub token_id_up: String,
    pub token_id_down: String,
    /// Binance spot at the moment we first observed this window.
    pub opening_price: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_seconds_correct() {
        assert_eq!(WindowKind::M5.period_secs(), 300);
        assert_eq!(WindowKind::M15.period_secs(), 900);
        assert_eq!(WindowKind::H1.period_secs(), 3600);
    }

    #[test]
    fn window_ts_aligns_to_period() {
        assert_eq!(WindowKind::M5.current_window_ts(1_700_000_123), 1_700_000_100);
        assert_eq!(WindowKind::M15.current_window_ts(1_700_000_999), 1_700_000_100);
        assert_eq!(WindowKind::H1.current_window_ts(1_700_000_100), 1_699_999_200);
    }

    #[test]
    fn slug_format() {
        assert_eq!(
            WindowKind::M5.slug(1_700_000_100),
            "btc-updown-5m-1700000100"
        );
        assert_eq!(
            WindowKind::M15.slug(1_700_000_100),
            "btc-updown-15m-1700000100"
        );
        assert_eq!(
            WindowKind::H1.slug(1_700_000_100),
            "btc-updown-1h-1700000100"
        );
    }

    #[test]
    fn seconds_remaining_countdown() {
        let wk = WindowKind::M5;
        let ts = 1_700_000_100_u64;
        let now = ts as i64 + 100;
        assert_eq!(wk.seconds_remaining(ts, now), 200);
    }

    #[test]
    fn seconds_remaining_negative_past_end() {
        let wk = WindowKind::M15;
        let ts = 1_700_000_000_u64;
        let now = ts as i64 + 1000; // 100s past 900s window end
        assert_eq!(wk.seconds_remaining(ts, now), -100);
    }

    #[test]
    fn all_enumerates_three_kinds() {
        assert_eq!(WindowKind::all().len(), 3);
    }
}
