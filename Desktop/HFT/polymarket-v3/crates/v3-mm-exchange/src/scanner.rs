//! Market Scanner — discovers active 15-minute crypto markets via Gamma API
//!
//! Computes current + next 15-min window timestamps, then queries
//! GET https://gamma-api.polymarket.com/events?slug={asset}-updown-15m-{ts}
//! for each asset (BTC/ETH/SOL/XRP) every ~30 seconds.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use v3_mm_core::Asset;
use serde::Deserialize;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::types::{ExchangeEvent, MarketInfo};

const GAMMA_EVENTS_URL: &str = "https://gamma-api.polymarket.com/events";
const SCAN_INTERVAL_SECS: u64 = 30;
const WINDOW_SECS: i64 = 900; // 15 minutes

/// Asset slug prefixes for 15-minute markets
const ASSET_SLUGS: &[(Asset, &str)] = &[
    (Asset::BTC, "btc-updown-15m"),
    (Asset::ETH, "eth-updown-15m"),
    (Asset::SOL, "sol-updown-15m"),
    (Asset::XRP, "xrp-updown-15m"),
];

#[derive(Debug, Deserialize)]
struct GammaEvent {
    markets: Option<Vec<GammaMarket>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GammaMarket {
    condition_id: Option<String>,
    question: Option<String>,
    end_date: Option<String>,
    clob_token_ids: Option<String>, // JSON-encoded array string like "[\"id1\", \"id2\"]"
}

/// Fetch a single event by exact slug, extract markets for the given asset
async fn fetch_event_by_slug(
    http: &reqwest::Client,
    asset: Asset,
    slug: &str,
) -> Result<Vec<MarketInfo>> {
    let resp = http
        .get(GAMMA_EVENTS_URL)
        .query(&[("slug", slug), ("limit", "1")])
        .send()
        .await
        .context("Gamma API request failed")?;

    let events: Vec<GammaEvent> = resp
        .json()
        .await
        .context("Gamma API parse failed")?;

    let mut results = Vec::new();
    let now = Utc::now();

    for event in events {
        let markets = match event.markets {
            Some(m) => m,
            None => continue,
        };
        for m in markets {
            let condition_id = match &m.condition_id {
                Some(c) => c.clone(),
                None => continue,
            };

            // clob_token_ids comes as a JSON string like "[\"id1\", \"id2\"]"
            let token_ids: Vec<String> = match &m.clob_token_ids {
                Some(s) => match serde_json::from_str(s) {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                None => continue,
            };
            if token_ids.len() < 2 {
                continue;
            }

            let end_time = match &m.end_date {
                Some(s) => match s.parse::<DateTime<Utc>>() {
                    Ok(dt) => dt,
                    Err(_) => continue,
                },
                None => continue,
            };

            if end_time <= now {
                continue;
            }

            results.push(MarketInfo {
                condition_id,
                token_id_yes: token_ids[0].clone(),
                token_id_no: token_ids[1].clone(),
                asset,
                end_time,
                question: m.question.unwrap_or_default(),
            });
        }
    }

    Ok(results)
}

/// Compute window timestamps: current window start and next window start
fn window_timestamps() -> (i64, i64) {
    let now = Utc::now().timestamp();
    let current = now / WINDOW_SECS * WINDOW_SECS;
    let next = current + WINDOW_SECS;
    (current, next)
}

/// Fetch all active 15-minute markets across all assets
async fn fetch_all_markets(http: &reqwest::Client) -> Vec<MarketInfo> {
    let (current_ts, next_ts) = window_timestamps();
    let mut all = Vec::new();

    for &(asset, prefix) in ASSET_SLUGS {
        // Query current and next window for each asset
        for ts in &[current_ts, next_ts] {
            let slug = format!("{}-{}", prefix, ts);
            match fetch_event_by_slug(http, asset, &slug).await {
                Ok(markets) => all.extend(markets),
                Err(e) => debug!("[SCANNER] {} slug={}: {e}", asset, slug),
            }
        }
    }
    all
}

/// Spawn a background task that polls for new 15-minute markets.
/// Sends MarketReady events for newly discovered windows.
pub async fn spawn_market_scanner(tx: mpsc::Sender<ExchangeEvent>) -> Result<()> {
    let http = reqwest::Client::new();

    tokio::spawn(async move {
        let mut seen: HashSet<String> = HashSet::new();
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(SCAN_INTERVAL_SECS),
        );

        loop {
            interval.tick().await;

            let markets = fetch_all_markets(&http).await;
            let new_count = markets.iter()
                .filter(|m| !seen.contains(&m.condition_id))
                .count();

            if new_count > 0 {
                info!("[SCANNER] found {} active markets, {} new", markets.len(), new_count);
            } else {
                debug!("[SCANNER] {} active markets, 0 new", markets.len());
            }

            for market in markets {
                if seen.contains(&market.condition_id) {
                    continue;
                }
                info!(
                    "[SCANNER] new: {} {} \"{}\" ends={}",
                    market.asset, market.condition_id, market.question, market.end_time
                );
                seen.insert(market.condition_id.clone());
                if tx.send(ExchangeEvent::MarketReady(market)).await.is_err() {
                    error!("[SCANNER] channel closed");
                    return;
                }
            }

            if seen.len() > 500 {
                seen.clear();
            }
        }
    });

    info!("[SCANNER] market scanner started (interval={}s)", SCAN_INTERVAL_SECS);
    Ok(())
}
