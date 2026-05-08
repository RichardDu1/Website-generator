//! CLI arg parsing — deliberately minimal; no clap to keep deps lean.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Args {
    pub signals_csv: PathBuf,
    pub out_dir: PathBuf,
    /// Taker fee in basis points. Set per-run by main.rs; any user-provided
    /// --fee-bps becomes the default but we iterate over {0, 200}.
    pub fee_bps: u32,
    /// Starting bankroll used for Kelly sizing.
    pub bankroll: f64,
}

impl Args {
    pub fn parse() -> Result<Self> {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        if raw.is_empty() || raw.iter().any(|a| a == "-h" || a == "--help") {
            print_usage();
            std::process::exit(if raw.is_empty() { 2 } else { 0 });
        }

        let mut signals_csv: Option<PathBuf> = None;
        let mut out_dir = PathBuf::from("backtest_out");
        let mut fee_bps: u32 = 0;
        let mut bankroll: f64 = 100.0;

        let mut it = raw.into_iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--out-dir" => {
                    out_dir = PathBuf::from(
                        it.next().context("--out-dir needs a value")?,
                    );
                }
                "--fee-bps" => {
                    fee_bps = it
                        .next()
                        .context("--fee-bps needs a value")?
                        .parse()
                        .context("--fee-bps must be u32")?;
                }
                "--bankroll" => {
                    bankroll = it
                        .next()
                        .context("--bankroll needs a value")?
                        .parse()
                        .context("--bankroll must be f64")?;
                }
                s if s.starts_with('-') => bail!("unknown flag: {s}"),
                s => {
                    if signals_csv.is_some() {
                        bail!("multiple positional args; only one signals.csv expected");
                    }
                    signals_csv = Some(PathBuf::from(s));
                }
            }
        }

        let signals_csv = signals_csv.context("missing signals.csv path")?;
        if !signals_csv.exists() {
            bail!(
                "signals.csv not found at {}\n\
                 tip: run the bot in dry_run first — logs/signals.csv will be created.",
                signals_csv.display()
            );
        }

        Ok(Self {
            signals_csv,
            out_dir,
            fee_bps,
            bankroll,
        })
    }
}

fn print_usage() {
    eprintln!(
        "\
v3-backtest — replay signals.csv through Gengar decisions.

USAGE:
    v3-backtest <signals.csv> [--out-dir DIR] [--bankroll USDC]

OPTIONS:
    <signals.csv>      path to a signals.csv produced by the live bot
    --out-dir DIR      output directory for backtest_trades.csv and
                       backtest_equity.csv (default: ./backtest_out)
    --bankroll USDC    starting bankroll for Kelly sizing (default: 100)
    -h, --help         show this help

NOTE: runs both fee modes automatically (0 bps maker, 200 bps taker).
      output files are suffixed with _0bps / _200bps.
"
    );
}
