//! Recover open positions from a trades.csv file on bot restart.
//!
//! When the bot restarts after a crash / deploy / Ctrl-C, any positions
//! it had open are still sitting on the exchange but the in-memory map
//! is empty. Without recovery the bot would:
//!   - Miss profit-lock / preemptive-cancel / wall-clock exits
//!   - Accept a new Gengar entry that conflicts with the existing one
//!
//! This module reads trades.csv, pairs each `entry` row with the matching
//! `exit-*` row by pair_id, and returns the unmatched entries as
//! recoverable OpenPosition values.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;

use crate::gengar_orchestrator::OpenPosition;
use crate::window_manager::WindowKind;
use v3_strategy::GengarSide;

#[derive(Debug, Deserialize)]
struct TradeRow {
    ts: String,
    event: String,
    pair_id: String,
    window: String,
    side: String,
    token_id: String,
    price: f64,
    shares: u64,
    #[serde(default)]
    #[allow(dead_code)]
    notional: String,
    initial_edge: f64,
    #[serde(default)]
    #[allow(dead_code)]
    current_edge: String,
    order_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    realized_pnl: String,
}

/// Read trades.csv and return entries that have no matching exit row.
/// Safe to call with a missing file — returns an empty vec.
pub fn recover_open_positions(path: impl AsRef<Path>) -> Result<Vec<OpenPosition>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut rdr = csv::Reader::from_reader(BufReader::new(file));

    // pair_id → entry row; remove on matching exit.
    let mut open: HashMap<String, TradeRow> = HashMap::new();

    for row in rdr.deserialize::<TradeRow>() {
        let row = row.context("parse trades.csv row")?;
        match row.event.as_str() {
            "entry" => {
                open.insert(row.pair_id.clone(), row);
            }
            e if e.starts_with("exit-") => {
                open.remove(&row.pair_id);
            }
            _ => {}
        }
    }

    let mut result = Vec::with_capacity(open.len());
    for (_, row) in open {
        match row_to_position(row) {
            Ok(p) => result.push(p),
            Err(e) => tracing::warn!("skip unrecoverable entry row: {e:#}"),
        }
    }
    Ok(result)
}

fn row_to_position(row: TradeRow) -> Result<OpenPosition> {
    let window_kind = parse_window(&row.window)?;
    let side = parse_side(&row.side)?;
    let entered_at = DateTime::<Utc>::from_str(&row.ts)
        .with_context(|| format!("bad ts {}", row.ts))?;
    Ok(OpenPosition {
        pair_id: row.pair_id,
        window_kind,
        side,
        token_id: row.token_id,
        entry_price: row.price,
        shares: row.shares,
        initial_edge: row.initial_edge,
        order_id: row.order_id,
        entered_at,
    })
}

fn parse_window(s: &str) -> Result<WindowKind> {
    match s {
        "M5" => Ok(WindowKind::M5),
        "M15" => Ok(WindowKind::M15),
        "H4" => Ok(WindowKind::H4),
        other => anyhow::bail!("unknown window {other}"),
    }
}

fn parse_side(s: &str) -> Result<GengarSide> {
    match s {
        "Up" => Ok(GengarSide::Up),
        "Down" => Ok(GengarSide::Down),
        other => anyhow::bail!("unknown side {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    const HEADER: &str = "ts,event,pair_id,window,side,token_id,price,shares,notional,initial_edge,current_edge,order_id,realized_pnl";

    fn write_trades(path: &Path, rows: &[&str]) {
        let mut f = File::create(path).unwrap();
        writeln!(f, "{HEADER}").unwrap();
        for r in rows {
            writeln!(f, "{r}").unwrap();
        }
    }

    #[test]
    fn missing_file_returns_empty() {
        let out = recover_open_positions("/does/not/exist.csv").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn empty_trades_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[]);
        assert!(recover_open_positions(&path).unwrap().is_empty());
    }

    #[test]
    fn fully_closed_trade_returns_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,M5,Up,tok123,0.65,10,6.50,0.10,,order-1,",
            "2026-01-01T00:01:00Z,exit-profit-lock,p1,M5,Up,tok123,0.75,10,7.50,0.10,0.02,order-2,1.0",
        ]);
        assert!(recover_open_positions(&path).unwrap().is_empty());
    }

    #[test]
    fn unmatched_entry_is_recovered() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,M5,Up,tok123,0.65,10,6.50,0.10,,order-1,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        let p = &out[0];
        assert_eq!(p.pair_id, "p1");
        assert_eq!(p.window_kind, WindowKind::M5);
        assert!(matches!(p.side, GengarSide::Up));
        assert_eq!(p.token_id, "tok123");
        assert_eq!(p.entry_price, 0.65);
        assert_eq!(p.shares, 10);
        assert_eq!(p.initial_edge, 0.10);
        assert_eq!(p.order_id, "order-1");
    }

    #[test]
    fn multiple_closed_plus_one_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,M5,Up,tok1,0.60,10,6.00,0.10,,order-1,",
            "2026-01-01T00:01:00Z,exit-profit-lock,p1,M5,Up,tok1,0.70,10,7.00,0.10,0.02,order-2,1.0",
            "2026-01-01T00:02:00Z,entry,p2,M15,Down,tok2,0.55,20,11.00,0.15,,order-3,",
            "2026-01-01T00:03:00Z,exit-preemptive-cancel,p2,M15,Down,tok2,0.50,20,10.00,0.15,-0.05,order-4,-1.0",
            "2026-01-01T00:04:00Z,entry,p3,H4,Up,tok3,0.70,5,3.50,0.12,,order-5,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pair_id, "p3");
        assert_eq!(out[0].window_kind, WindowKind::H4);
    }

    #[test]
    fn bad_window_is_skipped_not_fatal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,BOGUS,Up,tok1,0.60,10,6.00,0.10,,order-1,",
            "2026-01-01T00:01:00Z,entry,p2,M5,Up,tok2,0.65,10,6.50,0.10,,order-2,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pair_id, "p2");
    }
}
