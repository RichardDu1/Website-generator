//! Hourly trading session stats. Reset every hour, pushed to Telegram.
//!
//! Thread-safe via interior Mutex. Counter updates from entry/exit paths;
//! the orchestrator periodically checks `should_flush(now_secs)` and calls
//! `flush()` to get the summary message and reset counters.

use chrono::{DateTime, Utc};
use std::sync::Mutex;

pub struct HourlyStats {
    state: Mutex<State>,
}

struct State {
    hour_start: DateTime<Utc>,
    entries: u32,
    exits_profit_lock: u32,
    exits_force_sell: u32,
    wins: u32,
    losses: u32,
    pnl: f64,
    entry_edge_sum: f64,
}

impl Default for HourlyStats {
    fn default() -> Self {
        Self::new()
    }
}

impl HourlyStats {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                hour_start: Utc::now(),
                entries: 0,
                exits_profit_lock: 0,
                exits_force_sell: 0,
                wins: 0,
                losses: 0,
                pnl: 0.0,
                entry_edge_sum: 0.0,
            }),
        }
    }

    pub fn record_entry(&self, edge: f64) {
        let Ok(mut s) = self.state.lock() else { return };
        s.entries += 1;
        s.entry_edge_sum += edge;
    }

    pub fn record_exit(&self, reason: &str, pnl: f64) {
        let Ok(mut s) = self.state.lock() else { return };
        match reason {
            "profit-lock" => s.exits_profit_lock += 1,
            "force-sell" => s.exits_force_sell += 1,
            _ => {}
        }
        if pnl > 0.0 {
            s.wins += 1;
        } else if pnl < 0.0 {
            s.losses += 1;
        }
        s.pnl += pnl;
    }

    /// True iff wall-clock has crossed an hour boundary since `hour_start`.
    pub fn should_flush(&self, now: DateTime<Utc>) -> bool {
        let Ok(s) = self.state.lock() else { return false };
        (now - s.hour_start).num_seconds() >= 3600
    }

    /// Snapshot current counts as a human-readable summary and reset.
    /// Returns `None` if there's nothing meaningful to report (0 entries and
    /// 0 exits — silent when the bot was idle).
    pub fn flush(&self) -> Option<String> {
        let mut s = self.state.lock().ok()?;
        if s.entries == 0 && s.exits_profit_lock + s.exits_force_sell == 0 {
            // Reset hour_start so we don't repeatedly flush when idle.
            s.hour_start = Utc::now();
            return None;
        }
        let settled = s.wins + s.losses;
        let win_rate = if settled > 0 {
            s.wins as f64 / settled as f64 * 100.0
        } else {
            0.0
        };
        let avg_edge = if s.entries > 0 {
            s.entry_edge_sum / s.entries as f64
        } else {
            0.0
        };
        let msg = format!(
            "📊 Hourly summary\n\
             entries={} (avg_edge={:.4})\n\
             exits: profit-lock={} force-sell={}\n\
             settled={} wins={} losses={} win_rate={:.1}%\n\
             hour_pnl=${:+.2}",
            s.entries,
            avg_edge,
            s.exits_profit_lock,
            s.exits_force_sell,
            settled,
            s.wins,
            s.losses,
            win_rate,
            s.pnl,
        );
        // Reset for next hour.
        *s = State {
            hour_start: Utc::now(),
            entries: 0,
            exits_profit_lock: 0,
            exits_force_sell: 0,
            wins: 0,
            losses: 0,
            pnl: 0.0,
            entry_edge_sum: 0.0,
        };
        Some(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn flush_is_none_when_idle() {
        let h = HourlyStats::new();
        assert!(h.flush().is_none());
    }

    #[test]
    fn flush_includes_counts_and_pnl() {
        let h = HourlyStats::new();
        h.record_entry(0.08);
        h.record_entry(0.12);
        h.record_exit("profit-lock", 1.50);
        h.record_exit("force-sell", -0.80);
        let msg = h.flush().expect("non-empty");
        assert!(msg.contains("entries=2"));
        assert!(msg.contains("profit-lock=1"));
        assert!(msg.contains("force-sell=1"));
        assert!(msg.contains("wins=1"));
        assert!(msg.contains("losses=1"));
        assert!(msg.contains("win_rate=50.0"));
        assert!(msg.contains("avg_edge=0.1000"));
        assert!(msg.contains("hour_pnl=$+0.70"));
    }

    #[test]
    fn flush_resets_state() {
        let h = HourlyStats::new();
        h.record_entry(0.1);
        h.record_exit("profit-lock", 1.0);
        assert!(h.flush().is_some());
        assert!(h.flush().is_none(), "second flush should be idle");
    }

    #[test]
    fn win_rate_rounds_zero_settled() {
        let h = HourlyStats::new();
        h.record_entry(0.05);
        // No exit → settled=0
        let msg = h.flush().expect("non-empty");
        assert!(msg.contains("win_rate=0.0%"));
    }

    #[test]
    fn should_flush_false_within_hour() {
        let h = HourlyStats::new();
        let near = Utc::now() + Duration::seconds(1800); // 30 min
        assert!(!h.should_flush(near));
    }

    #[test]
    fn should_flush_true_after_hour() {
        let h = HourlyStats::new();
        let later = Utc::now() + Duration::seconds(3700);
        assert!(h.should_flush(later));
    }
}
