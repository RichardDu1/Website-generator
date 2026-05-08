//! Polymarket RTDS WebSocket — Chainlink Data Streams price feed
//!
//! Connects to wss://ws-live-data.polymarket.com and streams
//! real-time BTC/ETH/SOL/XRP Chainlink prices (the settlement source).
//!
//! Protocol:
//!   1. Connect
//!   2. Send subscribe: {"action":"subscribe","subscriptions":[{topic,type,filters}]}
//!   3. Receive: {"topic":"crypto_prices_chainlink","type":"update","timestamp":..,"payload":{symbol,value,timestamp}}

use anyhow::{Context, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use v3_mm_core::Asset;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::types::{ExchangeEvent, RtdsPrice};

const RTDS_WS_URL: &str = "wss://ws-live-data.polymarket.com";
const PING_INTERVAL_SECS: u64 = 5;
/// If no message (data or PONG) is received within this many seconds,
/// treat the connection as dead and reconnect. Polymarket RTDS typically
/// sends crypto_prices updates every 1-2s plus our PONG every 5s.
const READ_TIMEOUT_SECS: u64 = 15;

/// Top-level RTDS message envelope
#[derive(Debug, Deserialize)]
struct RtdsEnvelope {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    payload: Option<RtdsPayload>,
}

/// Price payload inside the envelope
#[derive(Debug, Deserialize)]
struct RtdsPayload {
    symbol: Option<String>,
    value: Option<f64>,
}

/// Parse asset string from RTDS to our Asset enum
fn parse_asset(s: &str) -> Option<Asset> {
    match s.to_lowercase().as_str() {
        "btc/usd" | "btcusd" | "btc" => Some(Asset::BTC),
        "eth/usd" | "ethusd" | "eth" => Some(Asset::ETH),
        "sol/usd" | "solusd" | "sol" => Some(Asset::SOL),
        "xrp/usd" | "xrpusd" | "xrp" => Some(Asset::XRP),
        // Binance-style symbols from crypto_prices topic
        "btcusdt" => Some(Asset::BTC),
        "ethusdt" => Some(Asset::ETH),
        "solusdt" => Some(Asset::SOL),
        "xrpusdt" => Some(Asset::XRP),
        _ => None,
    }
}

/// Build the subscription message for Chainlink crypto prices
fn build_subscribe_msg() -> String {
    serde_json::json!({
        "action": "subscribe",
        "subscriptions": [
            {
                "topic": "crypto_prices_chainlink",
                "type": "*"
            },
            {
                "topic": "crypto_prices",
                "type": "*"
            }
        ]
    })
    .to_string()
}

/// Run one RTDS session. Returns Err when disconnected.
async fn run_rtds_session(tx: &mpsc::Sender<ExchangeEvent>, url: &str) -> Result<()> {
    let (ws_stream, _) = connect_async(url)
        .await
        .context("RTDS: failed to connect")?;
    info!("RTDS: connected");

    let (mut write, mut read) = ws_stream.split();

    let sub_msg = build_subscribe_msg();
    write
        .send(Message::Text(sub_msg))
        .await
        .context("RTDS: failed to send subscribe")?;
    info!("RTDS: subscribed to crypto_prices_chainlink + crypto_prices");

    let (ping_stop_tx, mut ping_stop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(PING_INTERVAL_SECS),
        );
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if write.send(Message::Text("PING".into())).await.is_err() {
                        break;
                    }
                }
                _ = &mut ping_stop_rx => break,
            }
        }
    });

    let read_timeout = tokio::time::Duration::from_secs(READ_TIMEOUT_SECS);
    loop {
        let msg_result = tokio::time::timeout(read_timeout, read.next()).await;
        match msg_result {
            Err(_) => {
                // Read timeout — no message received in READ_TIMEOUT_SECS.
                // The connection is silently dead; force reconnect.
                warn!("RTDS: read timeout ({}s), reconnecting", READ_TIMEOUT_SECS);
                let _ = ping_stop_tx.send(());
                return Err(anyhow::anyhow!("read timeout"));
            }
            Ok(None) => {
                // Stream ended
                break;
            }
            Ok(Some(msg_result)) => match msg_result {
                Ok(Message::Text(text)) => {
                    if text == "PONG" {
                        if tx.send(ExchangeEvent::RtdsAlive).await.is_err() {
                            let _ = ping_stop_tx.send(());
                            return Err(anyhow::anyhow!("channel closed"));
                        }
                        continue;
                    }
                    if let Ok(env) = serde_json::from_str::<RtdsEnvelope>(&text) {
                        if let Some(payload) = env.payload {
                            if let (Some(sym), Some(value)) = (&payload.symbol, payload.value) {
                                if let Some(asset) = parse_asset(sym) {
                                    let event = ExchangeEvent::RtdsPrice(RtdsPrice {
                                        asset, price: value, ts: Utc::now(),
                                    });
                                    if tx.send(event).await.is_err() {
                                        let _ = ping_stop_tx.send(());
                                        return Err(anyhow::anyhow!("channel closed"));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => { warn!("RTDS: server closed"); break; }
                Err(e) => { error!("RTDS: read error: {e}"); break; }
                _ => {}
            },
        }
    }
    let _ = ping_stop_tx.send(());
    Err(anyhow::anyhow!("disconnected"))
}

/// Spawn RTDS WebSocket task with auto-reconnect.
pub async fn spawn_rtds_feed(
    tx: mpsc::Sender<ExchangeEvent>,
    ws_url: Option<&str>,
) -> Result<()> {
    let url = ws_url.unwrap_or(RTDS_WS_URL).to_string();
    info!("RTDS: connecting to {url}");

    tokio::spawn(async move {
        loop {
            match run_rtds_session(&tx, &url).await {
                Err(e) if e.to_string().contains("channel closed") => {
                    error!("RTDS: channel closed, stopping");
                    break;
                }
                Err(e) => warn!("RTDS: session ended: {e}"),
                Ok(_) => {}
            }
            warn!("RTDS: reconnecting in 2s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    Ok(())
}
