//! Core replay loop.
//!
//! We read signals.csv rows (one per tick), call `gengar::evaluate` to
//! decide entries, and simulate exits using the same pure decision
//! functions the live bot uses.
//!
//! Simplification: we DON'T re-use `BookCache`/`OrderExecutor`/etc. — those
//! are I/O shims. All decisions flow through pure functions in v3-strategy.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

use v3_strategy::{
    evaluate, should_force_close_by_wallclock, should_lock_profit,
    should_preemptive_cancel, GengarConfig, GengarSide,
};

use crate::cli::Args;

/// One row of signals.csv (schema in v3-bot/src/event_logger.rs::SIGNALS_HEADER).
#[derive(Debug, Deserialize)]
pub struct SignalRow {
    pub ts: String,
    pub window: String,
    pub btc_price: f64,
    pub opening_price: f64,
    pub delta_pct: f64,
    pub sec_rem: f64,
    pub vol: f64,
    pub up_ask: f64,
    pub down_ask: f64,
    pub action: String,
    // The remaining columns (prob, edge, kelly_usdc, skip_reason) are
    // re-derived by the backtest; we don't need the originals.
}

/// A simulated position held by the backtest.
#[derive(Debug, Clone)]
#[allow(dead_code)] // opening_price stored for future scenarios
pub struct SimPosition {
    pub window: String,
    pub side: GengarSide,
    pub entry_ts: String,
    pub entry_price: f64,
    pub shares: u64,
    pub initial_edge: f64,
    pub opening_price: f64,
    /// Wall-clock seconds since scenario start when we opened.
    pub entered_at_secs: u64,
}

/// One simulated trade (entry + matching exit).
#[derive(Debug, Clone)]
pub struct SimTrade {
    pub window: String,
    pub side: GengarSide,
    pub entry_ts: String,
    pub entry_price: f64,
    pub exit_ts: String,
    pub exit_price: f64,
    pub shares: u64,
    pub initial_edge: f64,
    pub current_edge_at_exit: f64,
    pub gross_pnl: f64,
    pub fees_paid: f64,
    pub net_pnl: f64,
    pub exit_reason: String,
}

/// Aggregate outcome of a backtest run.
#[derive(Debug, Default)]
pub struct Outcome {
    pub rows_read: u64,
    pub entries_fired: u64,
    pub skips: u64,
    pub skip_reasons: std::collections::HashMap<String, u64>,
    pub trades: Vec<SimTrade>,
}

/// Run the backtest for a single fee setting. Caller iterates modes.
pub fn run(args: &Args) -> Result<Outcome> {
    let cfg = GengarConfig::default();
    let file = File::open(&args.signals_csv)
        .with_context(|| format!("open {}", args.signals_csv.display()))?;
    let mut rdr = csv::Reader::from_reader(BufReader::new(file));

    let mut outcome = Outcome::default();
    // One open position per window.
    let mut open: std::collections::HashMap<String, SimPosition> =
        std::collections::HashMap::new();
    let mut tick_count: u64 = 0;

    for row in rdr.deserialize::<SignalRow>() {
        let row = row.context("parse signals.csv row")?;
        outcome.rows_read += 1;
        tick_count += 1;
        // Signals are assumed roughly 1 tick per ~500ms. We use monotonic
        // tick_count as the wall-clock proxy for max_hold_secs — ticks/2
        // approximates seconds since scenario start.
        let now_secs = tick_count / 2;

        // --- Exit existing position first ---
        if let Some(pos) = open.get(&row.window).cloned() {
            let delta_pct = row.delta_pct;
            let sec_rem = row.sec_rem;
            let current_prob = v3_strategy::estimate_true_probability(
                delta_pct, sec_rem, row.vol,
            );
            let side_matches = (delta_pct > 0.0) == matches!(pos.side, GengarSide::Up);
            let effective_prob = if side_matches {
                current_prob
            } else {
                1.0 - current_prob
            };
            let current_market_price = match pos.side {
                GengarSide::Up => row.up_ask,
                GengarSide::Down => row.down_ask,
            };
            let current_edge = effective_prob - current_market_price;

            let held = now_secs.saturating_sub(pos.entered_at_secs);
            let exit_reason = if should_force_close_by_wallclock(&cfg, held) {
                Some("force-sell")
            } else if should_preemptive_cancel(&cfg, effective_prob) {
                Some("preemptive-cancel")
            } else if should_lock_profit(
                pos.initial_edge,
                current_edge,
                cfg.profit_lock_ratio,
            ) {
                Some("profit-lock")
            } else if sec_rem <= 0.0 {
                Some("window-expired")
            } else {
                None
            };

            if let Some(reason) = exit_reason {
                let gross = (current_market_price - pos.entry_price) * pos.shares as f64;
                let entry_notional = pos.entry_price * pos.shares as f64;
                let exit_notional = current_market_price * pos.shares as f64;
                // Fee charged on both legs' notional.
                let fees = (entry_notional + exit_notional) * (args.fee_bps as f64 / 10_000.0);
                outcome.trades.push(SimTrade {
                    window: pos.window.clone(),
                    side: pos.side,
                    entry_ts: pos.entry_ts.clone(),
                    entry_price: pos.entry_price,
                    exit_ts: row.ts.clone(),
                    exit_price: current_market_price,
                    shares: pos.shares,
                    initial_edge: pos.initial_edge,
                    current_edge_at_exit: current_edge,
                    gross_pnl: gross,
                    fees_paid: fees,
                    net_pnl: gross - fees,
                    exit_reason: reason.to_string(),
                });
                open.remove(&row.window);
            }
        }

        // --- Try entry if no position open for this window ---
        if open.contains_key(&row.window) {
            continue;
        }

        // Only consider entry on rows the live bot also considered (enter or
        // skip — we re-evaluate regardless of original decision).
        if row.action != "enter" && row.action != "skip" {
            continue;
        }

        let Some(sig) = evaluate(
            row.btc_price,
            row.opening_price,
            row.up_ask,
            row.down_ask,
            row.sec_rem,
            args.bankroll,
            &cfg,
            row.vol,
        ) else {
            outcome.skips += 1;
            // Best-effort skip reason from the raw row.
            // (We don't re-derive it here — the live bot already wrote it.)
            *outcome
                .skip_reasons
                .entry(String::from("evaluate_returned_none"))
                .or_insert(0) += 1;
            continue;
        };

        let shares = (sig.kelly_size / sig.market_price).floor() as u64;
        if shares < 5 {
            outcome.skips += 1;
            *outcome
                .skip_reasons
                .entry(String::from("shares_below_min"))
                .or_insert(0) += 1;
            continue;
        }

        let pos = SimPosition {
            window: row.window.clone(),
            side: sig.side,
            entry_ts: row.ts.clone(),
            entry_price: sig.market_price,
            shares,
            initial_edge: sig.edge,
            opening_price: row.opening_price,
            entered_at_secs: now_secs,
        };
        open.insert(row.window.clone(), pos);
        outcome.entries_fired += 1;
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_csv(path: &std::path::Path, rows: &[&str]) {
        let mut f = File::create(path).unwrap();
        writeln!(f, "ts,window,btc_price,opening_price,delta_pct,sec_rem,vol,up_ask,down_ask,action,prob,edge,kelly_usdc,skip_reason").unwrap();
        for r in rows {
            writeln!(f, "{r}").unwrap();
        }
    }

    #[test]
    fn empty_signals_produces_zero_trades() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("signals.csv");
        write_csv(&path, &[]);
        let args = Args {
            signals_csv: path,
            out_dir: dir.path().to_path_buf(),
            fee_bps: 0,
            bankroll: 100.0,
        };
        let out = run(&args).unwrap();
        assert_eq!(out.rows_read, 0);
        assert_eq!(out.entries_fired, 0);
        assert_eq!(out.trades.len(), 0);
    }

    #[test]
    fn single_entry_row_that_passes_all_gates() {
        // delta=0.15% at sec_rem=120, vol=0.12, up_ask=0.65 → strong entry
        let dir = tempdir().unwrap();
        let path = dir.path().join("signals.csv");
        write_csv(
            &path,
            &["2026-01-01T00:00:00Z,M5,100150.0,100000.0,0.15,120.0,0.12,0.65,0.35,enter,,,,"],
        );
        let args = Args {
            signals_csv: path,
            out_dir: dir.path().to_path_buf(),
            fee_bps: 0,
            bankroll: 100.0,
        };
        let out = run(&args).unwrap();
        assert_eq!(out.rows_read, 1);
        assert_eq!(out.entries_fired, 1, "should have entered");
        // No exit row, so trade is still open (not in outcome.trades).
        assert_eq!(out.trades.len(), 0);
    }

    #[test]
    fn entry_then_reversal_closes_preemptive_cancel() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("signals.csv");
        write_csv(
            &path,
            &[
                // Entry: +0.15%, UP=0.65 → enter UP
                "2026-01-01T00:00:00Z,M5,100150.0,100000.0,0.15,120.0,0.12,0.65,0.35,enter,,,,",
                // Reversal: BTC at -0.15%, UP still 0.65. side_matches=false, effective_prob ~0.1
                // → preemptive-cancel fires (below 0.55 threshold)
                "2026-01-01T00:00:30Z,M5,99850.0,100000.0,-0.15,90.0,0.12,0.65,0.35,skip,,,,",
            ],
        );
        let args = Args {
            signals_csv: path,
            out_dir: dir.path().to_path_buf(),
            fee_bps: 0,
            bankroll: 100.0,
        };
        let out = run(&args).unwrap();
        assert_eq!(out.entries_fired, 1);
        assert_eq!(out.trades.len(), 1);
        let t = &out.trades[0];
        assert_eq!(t.exit_reason, "preemptive-cancel");
        // Exit price is the same (0.65 UP) so gross_pnl = 0 … but fees=0 here
        // so net_pnl should also be 0.
        assert_eq!(t.fees_paid, 0.0);
    }

    #[test]
    fn fees_reduce_net_pnl_by_both_legs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("signals.csv");
        write_csv(
            &path,
            &[
                "2026-01-01T00:00:00Z,M5,100150.0,100000.0,0.15,120.0,0.12,0.65,0.35,enter,,,,",
                // Force exit via preemptive cancel with price unchanged
                "2026-01-01T00:00:30Z,M5,99850.0,100000.0,-0.15,90.0,0.12,0.65,0.35,skip,,,,",
            ],
        );
        let args = Args {
            signals_csv: path,
            out_dir: dir.path().to_path_buf(),
            fee_bps: 200,
            bankroll: 100.0,
        };
        let out = run(&args).unwrap();
        assert_eq!(out.trades.len(), 1);
        let t = &out.trades[0];
        // Both legs at 0.65 × shares. Fee = (2 × 0.65 × shares) × 0.02.
        assert!(t.fees_paid > 0.0);
        assert_eq!(t.gross_pnl, 0.0);
        assert!(t.net_pnl < 0.0, "fees should make it negative");
    }
}
