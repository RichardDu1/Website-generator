//! Binance WebSocket — auxiliary price feed for volatility estimation
//!
//! Subscribes to bookTicker streams for BTC/ETH/SOL/XRP USDT pairs.

use anyhow::{Context, Result};
use chrono::Utc;
use futures_util::StreamExt;
use v3_mm_core::Asset;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::types::{BinanceTick, ExchangeEvent};

const BINANCE_WS_BASE: &str = "wss://stream.binance.com:9443/ws";

#[derive(Debug, Deserialize)]
struct BookTickerMsg {
    s: String,       // symbol e.g. "BTCUSDT"
    b: String,       // best bid price
    #[serde(rename = "B")]
    b_qty: String,   // best bid qty
    a: String,       // best ask price
    #[serde(rename = "A")]
    a_qty: String,   // best ask qty
}

fn symbol_to_asset(s: &str) -> Option<Asset> {
    match s {
        "BTCUSDT" => Some(Asset::BTC),
        "ETHUSDT" => Some(Asset::ETH),
        "SOLUSDT" => Some(Asset::SOL),
        "XRPUSDT" => Some(Asset::XRP),
        _ => None,
    }
}

/// Run one Binance session. Returns Err when disconnected.
async fn run_binance_session(tx: &mpsc::Sender<ExchangeEvent>, url: &str) -> Result<()> {
    let (ws_stream, _) = connect_async(url)
        .await
        .context("Binance: failed to connect")?;
    info!("Binance: connected");

    let (_write, mut read) = ws_stream.split();

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if let Ok(msg) = serde_json::from_str::<BookTickerMsg>(&text) {
                    if let Some(asset) = symbol_to_asset(&msg.s) {
                        let bid: f64 = msg.b.parse().unwrap_or(0.0);
                        let ask: f64 = msg.a.parse().unwrap_or(0.0);
                        if bid > 0.0 && ask > 0.0 {
                            let event = ExchangeEvent::BinanceTick(BinanceTick {
                                asset, bid, ask, ts: Utc::now(),
                            });
                            if tx.send(event).await.is_err() {
                                return Err(anyhow::anyhow!("channel closed"));
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => { warn!("Binance: server closed"); break; }
            Err(e) => { error!("Binance: read error: {e}"); break; }
            _ => {}
        }
    }
    Err(anyhow::anyhow!("disconnected"))
}

/// Spawn Binance WebSocket task with auto-reconnect.
pub async fn spawn_binance_feed(
    tx: mpsc::Sender<ExchangeEvent>,
    symbols: &[&str],
    ws_base: Option<&str>,
) -> Result<()> {
    let streams: Vec<String> = symbols
        .iter()
        .map(|s| format!("{}@bookTicker", s.to_lowercase()))
        .collect();
    let url = format!(
        "{}/{}",
        ws_base.unwrap_or(BINANCE_WS_BASE),
        streams.join("/")
    );

    info!("Binance: connecting to {url}");

    tokio::spawn(async move {
        loop {
            match run_binance_session(&tx, &url).await {
                Err(e) if e.to_string().contains("channel closed") => {
                    error!("Binance: channel closed, stopping");
                    break;
                }
                Err(e) => warn!("Binance: session ended: {e:#}"),
                Ok(_) => {}
            }
            warn!("Binance: reconnecting in 2s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    Ok(())
}
