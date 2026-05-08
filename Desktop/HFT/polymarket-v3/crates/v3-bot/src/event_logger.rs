//! Per-event CSV logging for Gengar: `signals.csv` + `trades.csv`.
//!
//! Written as the bot runs. Use the files for post-hoc calibration:
//! - signals.csv: every `evaluate()` call, incl. skipped (reason field)
//! - trades.csv: entry and exit fills with realized PnL
//!
//! Thread-safe: callers hold an `Arc<EventLogger>` and each log method
//! acquires a `Mutex` briefly to append a line and flush.

use anyhow::{Context, Result};
use chrono::Utc;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::window_manager::WindowKind;

pub struct EventLogger {
    signals: Mutex<File>,
    trades: Mutex<File>,
}

const SIGNALS_HEADER: &str = "ts,window,btc_price,opening_price,delta_pct,sec_rem,vol,up_ask,down_ask,action,prob,edge,kelly_usdc,skip_reason";
const TRADES_HEADER: &str = "ts,event,pair_id,asset,window,side,token_id,price,shares,notional,initial_edge,current_edge,order_id,realized_pnl";

impl EventLogger {
    /// Create (or reopen) the two log files under `dir`.
    /// If the files already exist we append; otherwise we write the header row.
    pub fn new(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create log dir {}", dir.display()))?;
        let signals = open_with_header(&dir.join("signals.csv"), SIGNALS_HEADER)?;
        let trades = open_with_header(&dir.join("trades.csv"), TRADES_HEADER)?;
        Ok(Self {
            signals: Mutex::new(signals),
            trades: Mutex::new(trades),
        })
    }

    /// Log a signal evaluation result. `action` is "enter" or "skip";
    /// `skip_reason` is `None` for enter, `Some(&str)` for skip.
    #[allow(clippy::too_many_arguments)]
    pub fn log_signal(
        &self,
        window: WindowKind,
        btc_price: f64,
        opening_price: f64,
        delta_pct: f64,
        sec_rem: f64,
        vol: f64,
        up_ask: f64,
        down_ask: f64,
        action: &str,
        prob: Option<f64>,
        edge: Option<f64>,
        kelly_usdc: Option<f64>,
        skip_reason: Option<&str>,
    ) {
        let ts = Utc::now().to_rfc3339();
        let line = format!(
            "{ts},{window:?},{btc_price:.4},{opening_price:.4},{delta_pct:.6},{sec_rem:.1},{vol:.6},{up_ask:.4},{down_ask:.4},{action},{},{},{},{}\n",
            prob.map(|v| format!("{v:.6}")).unwrap_or_default(),
            edge.map(|v| format!("{v:.6}")).unwrap_or_default(),
            kelly_usdc.map(|v| format!("{v:.4}")).unwrap_or_default(),
            skip_reason.unwrap_or(""),
        );
        if let Ok(mut f) = self.signals.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }

    /// Log an entry fill.
    #[allow(clippy::too_many_arguments)]
    pub fn log_entry(
        &self,
        pair_id: &str,
        asset: &str,
        window: WindowKind,
        side: &str,
        token_id: &str,
        price: f64,
        shares: u64,
        initial_edge: f64,
        order_id: &str,
    ) {
        let ts = Utc::now().to_rfc3339();
        let notional = price * shares as f64;
        let line = format!(
            "{ts},entry,{pair_id},{asset},{window:?},{side},{token_id},{price:.4},{shares},{notional:.4},{initial_edge:.6},,{order_id},\n"
        );
        if let Ok(mut f) = self.trades.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }

    /// Log an exit (profit-lock, force-sell, window-end release).
    #[allow(clippy::too_many_arguments)]
    pub fn log_exit(
        &self,
        pair_id: &str,
        asset: &str,
        window: WindowKind,
        side: &str,
        token_id: &str,
        price: f64,
        shares: u64,
        initial_edge: f64,
        current_edge: Option<f64>,
        order_id: &str,
        realized_pnl: Option<f64>,
        reason: &str,
    ) {
        let ts = Utc::now().to_rfc3339();
        let notional = price * shares as f64;
        let line = format!(
            "{ts},exit-{reason},{pair_id},{asset},{window:?},{side},{token_id},{price:.4},{shares},{notional:.4},{initial_edge:.6},{},{order_id},{}\n",
            current_edge.map(|v| format!("{v:.6}")).unwrap_or_default(),
            realized_pnl.map(|v| format!("{v:.4}")).unwrap_or_default(),
        );
        if let Ok(mut f) = self.trades.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
}

fn open_with_header(path: &PathBuf, header: &str) -> Result<File> {
    let exists = path.exists() && std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    if !exists {
        writeln!(f, "{header}")?;
        f.flush()?;
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn headers_written_on_new_files() {
        let dir = tempdir().unwrap();
        let _ = EventLogger::new(dir.path()).unwrap();
        let mut s = String::new();
        File::open(dir.path().join("signals.csv"))
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert!(s.starts_with("ts,window,btc_price,"));
        let mut t = String::new();
        File::open(dir.path().join("trades.csv"))
            .unwrap()
            .read_to_string(&mut t)
            .unwrap();
        assert!(t.starts_with("ts,event,pair_id,"));
    }

    #[test]
    fn append_does_not_duplicate_header() {
        let dir = tempdir().unwrap();
        let log1 = EventLogger::new(dir.path()).unwrap();
        log1.log_signal(
            WindowKind::M5,
            100_000.0,
            99_900.0,
            0.1,
            120.0,
            0.12,
            0.55,
            0.45,
            "skip",
            None,
            None,
            None,
            Some("delta_too_small"),
        );
        drop(log1);

        let _log2 = EventLogger::new(dir.path()).unwrap();
        let mut s = String::new();
        File::open(dir.path().join("signals.csv"))
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        // Header + 1 data row + (possibly) trailing newline — check header appears exactly once.
        assert_eq!(s.matches("ts,window,btc_price").count(), 1, "got:\n{s}");
        assert!(s.contains("M5"));
        assert!(s.contains("delta_too_small"));
    }

    #[test]
    fn log_entry_and_exit_round_trip() {
        let dir = tempdir().unwrap();
        let log = EventLogger::new(dir.path()).unwrap();
        log.log_entry(
            "pair1",
            "BTC",
            WindowKind::M15,
            "Up",
            "0xdeadbeef",
            0.65,
            10,
            0.10,
            "order-1",
        );
        log.log_exit(
            "pair1",
            "BTC",
            WindowKind::M15,
            "Up",
            "0xdeadbeef",
            0.72,
            10,
            0.10,
            Some(0.02),
            "order-2",
            Some(0.70),
            "profit-lock",
        );

        let mut t = String::new();
        File::open(dir.path().join("trades.csv"))
            .unwrap()
            .read_to_string(&mut t)
            .unwrap();
        let lines: Vec<&str> = t.trim().split('\n').collect();
        assert_eq!(lines.len(), 3); // header + entry + exit
        assert!(lines[1].contains(",entry,"));
        assert!(lines[1].contains("pair1"));
        assert!(lines[2].contains(",exit-profit-lock,"));
        assert!(lines[2].contains("0.70"));
    }
}
