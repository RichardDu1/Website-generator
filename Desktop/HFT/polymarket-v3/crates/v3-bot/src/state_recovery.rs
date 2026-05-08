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
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;

use crate::gengar_orchestrator::OpenPosition;
use crate::window_manager::WindowKind;
use v3_mm_core::Asset;
use v3_strategy::GengarSide;

/// Raw intermediate row to handle both v1 (no `asset` col) and v2 schemas.
struct RawRow {
    ts: String,
    event: String,
    pair_id: String,
    asset: Asset,
    window: String,
    side: String,
    token_id: String,
    price: f64,
    shares: u64,
    initial_edge: f64,
    order_id: String,
}

/// Read trades.csv and return entries that have no matching exit row.
/// Safe to call with a missing file — returns an empty vec.
///
/// Tolerates two schemas:
///   v1 (13 cols): ts,event,pair_id,window,side,token_id,price,shares,notional,
///                 initial_edge,current_edge,order_id,realized_pnl
///   v2 (14 cols): same + `asset` column inserted after pair_id
///
/// v1 rows default `asset` to BTC (safe: pre-I5 bot was BTC-only).
pub fn recover_open_positions(path: impl AsRef<Path>) -> Result<Vec<OpenPosition>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(BufReader::new(file));

    // Detect schema from header.
    let headers = rdr.headers().context("read headers")?.clone();
    let has_asset = headers.iter().any(|h| h == "asset");

    // pair_id → entry row; remove on matching exit.
    let mut open: HashMap<String, RawRow> = HashMap::new();

    for rec in rdr.records() {
        let rec = rec.context("read trades.csv record")?;
        let raw = match parse_row(&rec, has_asset) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("skip bad trades.csv row: {e:#}");
                continue;
            }
        };
        match raw.event.as_str() {
            "entry" => {
                open.insert(raw.pair_id.clone(), raw);
            }
            e if e.starts_with("exit-") => {
                open.remove(&raw.pair_id);
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

fn parse_row(rec: &csv::StringRecord, has_asset: bool) -> Result<RawRow> {
    let at = |i: usize| rec.get(i).unwrap_or("").to_string();
    // Both schemas start: ts, event, pair_id
    let ts = at(0);
    let event = at(1);
    let pair_id = at(2);

    let (asset_str, window, side, token_id, price, shares, initial_edge, order_id) = if has_asset {
        (at(3), at(4), at(5), at(6), at(7), at(8), at(10), at(12))
    } else {
        // v1: no asset column — shift everything left by 1, default to BTC.
        ("BTC".to_string(), at(3), at(4), at(5), at(6), at(7), at(9), at(11))
    };

    let asset = parse_asset(&asset_str)?;
    let price: f64 = price.parse().with_context(|| format!("bad price {price}"))?;
    let shares: u64 = shares.parse().with_context(|| format!("bad shares {shares}"))?;
    let initial_edge: f64 = initial_edge
        .parse()
        .with_context(|| format!("bad initial_edge {initial_edge}"))?;

    Ok(RawRow { ts, event, pair_id, asset, window, side, token_id, price, shares, initial_edge, order_id })
}

fn parse_asset(s: &str) -> Result<Asset> {
    match s {
        "BTC" => Ok(Asset::BTC),
        "ETH" => Ok(Asset::ETH),
        "SOL" => Ok(Asset::SOL),
        "XRP" => Ok(Asset::XRP),
        other => anyhow::bail!("unknown asset {other}"),
    }
}

fn row_to_position(row: RawRow) -> Result<OpenPosition> {
    let window_kind = parse_window(&row.window)?;
    let side = parse_side(&row.side)?;
    let entered_at = DateTime::<Utc>::from_str(&row.ts)
        .with_context(|| format!("bad ts {}", row.ts))?;
    Ok(OpenPosition {
        pair_id: row.pair_id,
        asset: row.asset,
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

    const HEADER_V1: &str = "ts,event,pair_id,window,side,token_id,price,shares,notional,initial_edge,current_edge,order_id,realized_pnl";
    const HEADER_V2: &str = "ts,event,pair_id,asset,window,side,token_id,price,shares,notional,initial_edge,current_edge,order_id,realized_pnl";

    fn write_trades_v1(path: &Path, rows: &[&str]) {
        let mut f = File::create(path).unwrap();
        writeln!(f, "{HEADER_V1}").unwrap();
        for r in rows { writeln!(f, "{r}").unwrap(); }
    }

    fn write_trades(path: &Path, rows: &[&str]) {
        let mut f = File::create(path).unwrap();
        writeln!(f, "{HEADER_V2}").unwrap();
        for r in rows { writeln!(f, "{r}").unwrap(); }
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
            "2026-01-01T00:00:00Z,entry,p1,BTC,M5,Up,tok123,0.65,10,6.50,0.10,,order-1,",
            "2026-01-01T00:01:00Z,exit-profit-lock,p1,BTC,M5,Up,tok123,0.75,10,7.50,0.10,0.02,order-2,1.0",
        ]);
        assert!(recover_open_positions(&path).unwrap().is_empty());
    }

    #[test]
    fn unmatched_entry_is_recovered() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,BTC,M5,Up,tok123,0.65,10,6.50,0.10,,order-1,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        let p = &out[0];
        assert_eq!(p.pair_id, "p1");
        assert_eq!(p.asset, Asset::BTC);
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
            "2026-01-01T00:00:00Z,entry,p1,BTC,M5,Up,tok1,0.60,10,6.00,0.10,,order-1,",
            "2026-01-01T00:01:00Z,exit-profit-lock,p1,BTC,M5,Up,tok1,0.70,10,7.00,0.10,0.02,order-2,1.0",
            "2026-01-01T00:02:00Z,entry,p2,ETH,M15,Down,tok2,0.55,20,11.00,0.15,,order-3,",
            "2026-01-01T00:03:00Z,exit-preemptive-cancel,p2,ETH,M15,Down,tok2,0.50,20,10.00,0.15,-0.05,order-4,-1.0",
            "2026-01-01T00:04:00Z,entry,p3,SOL,H4,Up,tok3,0.70,5,3.50,0.12,,order-5,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pair_id, "p3");
        assert_eq!(out[0].asset, Asset::SOL);
        assert_eq!(out[0].window_kind, WindowKind::H4);
    }

    #[test]
    fn bad_window_is_skipped_not_fatal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,BTC,BOGUS,Up,tok1,0.60,10,6.00,0.10,,order-1,",
            "2026-01-01T00:01:00Z,entry,p2,BTC,M5,Up,tok2,0.65,10,6.50,0.10,,order-2,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pair_id, "p2");
    }

    /// v1 schema (no asset column) should still recover as BTC.
    #[test]
    fn v1_schema_defaults_to_btc() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trades.csv");
        write_trades_v1(&path, &[
            "2026-01-01T00:00:00Z,entry,p1,M5,Up,tok1,0.60,10,6.00,0.10,,order-1,",
        ]);
        let out = recover_open_positions(&path).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].asset, Asset::BTC);
        assert_eq!(out[0].window_kind, WindowKind::M5);
    }
}
