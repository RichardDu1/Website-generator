//! v3-backtest — replay signals.csv through Gengar decisions
//!
//! Reads the signals.csv emitted by the live bot's EventLogger and replays
//! each tick through `gengar::evaluate` + the exit decisions. Simulates
//! position entries/exits to estimate historical PnL.
//!
//! Not perfect: the actual fill is assumed to happen at the logged ask
//! price (matching what the live bot quoted). Real fills may have differed,
//! but for Gengar the entry order is a limit at that price so the bot never
//! pays more — only worse case is the order never fills.
//!
//! Usage:
//!   v3-backtest <signals.csv> [--out-dir DIR] [--fee-bps N]

mod backtest;
mod cli;
mod report;

use anyhow::Result;
use cli::Args;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse()?;

    // Always run both fee modes (per user decision).
    for fee_bps in [0_u32, 200_u32] {
        println!("\n=== Fee mode: {fee_bps} bps ===");
        let mut cfg = args.clone();
        cfg.fee_bps = fee_bps;
        let outcome = backtest::run(&cfg)?;
        report::print_summary(&outcome, fee_bps);
        report::write_csvs(&outcome, &cfg)?;
    }

    Ok(())
}
