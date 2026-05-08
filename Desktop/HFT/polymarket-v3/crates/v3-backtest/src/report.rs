//! Summary printing + CSV output.

use anyhow::{Context, Result};
use std::fs;
use std::io::Write;

use crate::backtest::{Outcome, SimTrade};
use crate::cli::Args;
use v3_strategy::GengarSide;

pub fn print_summary(outcome: &Outcome, fee_bps: u32) {
    let trades = &outcome.trades;
    let n = trades.len();
    println!("rows read         : {}", outcome.rows_read);
    println!("entries fired     : {}", outcome.entries_fired);
    println!("skipped           : {}", outcome.skips);
    if !outcome.skip_reasons.is_empty() {
        let mut by_reason: Vec<_> = outcome.skip_reasons.iter().collect();
        by_reason.sort_by(|a, b| b.1.cmp(a.1));
        println!("skip reasons:");
        for (reason, n) in by_reason.iter().take(10) {
            println!("  {reason:<32} {n}");
        }
    }
    println!("closed trades     : {n}");
    if n == 0 {
        println!("(no trades closed; need more signals.csv rows spanning entry+exit)");
        return;
    }

    let gross: f64 = trades.iter().map(|t| t.gross_pnl).sum();
    let fees: f64 = trades.iter().map(|t| t.fees_paid).sum();
    let net: f64 = trades.iter().map(|t| t.net_pnl).sum();
    let wins = trades.iter().filter(|t| t.net_pnl > 0.0).count();
    let losses = trades.iter().filter(|t| t.net_pnl < 0.0).count();
    let flats = n - wins - losses;
    let settled = wins + losses;
    let win_rate = if settled > 0 {
        wins as f64 / settled as f64 * 100.0
    } else {
        0.0
    };
    let avg = net / n as f64;
    let best = trades.iter().map(|t| t.net_pnl).fold(f64::NEG_INFINITY, f64::max);
    let worst = trades.iter().map(|t| t.net_pnl).fold(f64::INFINITY, f64::min);

    // Max drawdown: walk cumulative PnL, track peak-to-trough gap.
    let mut cum = 0.0;
    let mut peak = 0.0;
    let mut max_dd = 0.0;
    for t in trades {
        cum += t.net_pnl;
        if cum > peak {
            peak = cum;
        }
        let dd = peak - cum;
        if dd > max_dd {
            max_dd = dd;
        }
    }

    // Exit reason counts.
    let mut by_reason: std::collections::HashMap<&str, u64> = Default::default();
    for t in trades {
        *by_reason.entry(t.exit_reason.as_str()).or_insert(0) += 1;
    }

    println!("gross pnl         : {:+.4}", gross);
    println!("fees paid         : {:.4} ({fee_bps} bps both legs)", fees);
    println!("net pnl           : {:+.4}", net);
    println!("avg trade         : {:+.4}", avg);
    println!("best / worst      : {:+.4} / {:+.4}", best, worst);
    println!("wins / losses     : {wins} / {losses} (flat={flats})");
    println!("win rate          : {:.1}%", win_rate);
    println!("max drawdown      : {:.4}", max_dd);
    println!("exit reasons:");
    let mut reasons: Vec<_> = by_reason.iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(a.1));
    for (reason, n) in reasons {
        println!("  {reason:<20} {n}");
    }
}

pub fn write_csvs(outcome: &Outcome, args: &Args) -> Result<()> {
    fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("create {}", args.out_dir.display()))?;

    // backtest_trades_{fee}bps.csv
    let fee_suffix = format!("{}bps", args.fee_bps);
    let trades_path = args
        .out_dir
        .join(format!("backtest_trades_{fee_suffix}.csv"));
    let mut f = fs::File::create(&trades_path)
        .with_context(|| format!("create {}", trades_path.display()))?;
    writeln!(
        f,
        "window,side,entry_ts,entry_price,exit_ts,exit_price,shares,initial_edge,current_edge_at_exit,gross_pnl,fees_paid,net_pnl,exit_reason"
    )?;
    for t in &outcome.trades {
        writeln!(
            f,
            "{},{:?},{},{:.4},{},{:.4},{},{:.6},{:.6},{:.4},{:.4},{:.4},{}",
            t.window,
            match t.side { GengarSide::Up => "Up", GengarSide::Down => "Down" },
            t.entry_ts,
            t.entry_price,
            t.exit_ts,
            t.exit_price,
            t.shares,
            t.initial_edge,
            t.current_edge_at_exit,
            t.gross_pnl,
            t.fees_paid,
            t.net_pnl,
            t.exit_reason,
        )?;
    }

    // backtest_equity_{fee}bps.csv — cumulative net PnL per trade
    let eq_path = args.out_dir.join(format!("backtest_equity_{fee_suffix}.csv"));
    let mut f = fs::File::create(&eq_path)
        .with_context(|| format!("create {}", eq_path.display()))?;
    writeln!(f, "trade_idx,exit_ts,trade_pnl,cumulative_pnl")?;
    let mut cum = 0.0;
    for (i, t) in outcome.trades.iter().enumerate() {
        cum += t.net_pnl;
        writeln!(f, "{},{},{:.4},{:.4}", i + 1, t.exit_ts, t.net_pnl, cum)?;
    }

    println!("trades  -> {}", trades_path.display());
    println!("equity  -> {}", eq_path.display());
    Ok(())
}

// We never actually read this type from outside; kept for symmetry.
#[allow(dead_code)]
fn _compile_check(_t: &SimTrade) {}
