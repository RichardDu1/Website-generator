//! Polymarket CLOB — order management via polymarket-client-sdk
//!
//! REST: place/cancel orders using the official SDK
//! WS: subscribe to orderbook updates, fills, market resolutions

use anyhow::{Context, Result};
use chrono::Utc;
use futures_util::StreamExt;
use mm_core::Asset;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::types::{
    BookLevel, BookSnapshot, ExchangeEvent, MarketResolved, Outcome,
};

const CLOB_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

// ── Order Client (wraps polymarket-client-sdk) ──

use std::str::FromStr;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer as _;
use polymarket_client_sdk::clob::types::{OrderType, Side as SdkSide, SignableOrder, SignedOrder};
use polymarket_client_sdk::clob::types::response::{CancelOrdersResponse, PostOrderResponse};
use polymarket_client_sdk::clob::{Client as ClobClient, Config as ClobConfig};
use polymarket_client_sdk::types::U256;
use polymarket_client_sdk::POLYGON;

/// High-level order client for our market maker
pub struct OrderClient {
    client: ClobClient<polymarket_client_sdk::auth::state::Authenticated<polymarket_client_sdk::auth::Normal>>,
    signer: PrivateKeySigner,
}

impl OrderClient {
    /// Create and authenticate an order client.
    /// `private_key` is the hex-encoded private key (with or without 0x prefix).
    pub async fn new(host: &str, private_key: &str) -> Result<Self> {
        let mut signer = PrivateKeySigner::from_str(private_key)
            .context("Invalid private key")?;
        signer.set_chain_id(Some(POLYGON));

        let unauth = ClobClient::new(host, ClobConfig::default())
            .map_err(|e| anyhow::anyhow!("CLOB client init: {e}"))?;

        let client = unauth
            .authentication_builder(&signer)
            .authenticate()
            .await
            .map_err(|e| anyhow::anyhow!("CLOB auth: {e}"))?;

        info!("Polymarket CLOB: authenticated as {}", signer.address());
        Ok(Self { client, signer })
    }

    /// Place a Post-Only limit order (maker only).
    pub async fn place_limit_order(
        &self,
        token_id: &str,
        side: crate::types::OrderSide,
        price: Decimal,
        size: Decimal,
    ) -> Result<String> {
        let token_u256 = U256::from_str_radix(token_id, 10)
            .or_else(|_| U256::from_str_radix(token_id.trim_start_matches("0x"), 16))
            .context("Invalid token_id")?;

        let sdk_side = match side {
            crate::types::OrderSide::Buy => SdkSide::Buy,
            crate::types::OrderSide::Sell => SdkSide::Sell,
        };

        let signable: SignableOrder = self
            .client
            .limit_order()
            .token_id(token_u256)
            .side(sdk_side)
            .price(price)
            .size(size)
            .post_only(true)
            .order_type(OrderType::GTC)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Build order: {e}"))?;

        let signed: SignedOrder = self
            .client
            .sign(&self.signer, signable)
            .await
            .map_err(|e| anyhow::anyhow!("Sign order: {e}"))?;

        let resp: PostOrderResponse = self
            .client
            .post_order(signed)
            .await
            .map_err(|e| anyhow::anyhow!("Post order: {e}"))?;

        info!(
            "[ORDER] {} {:?} price={} size={} id={}",
            token_id, side, price, size, resp.order_id
        );
        Ok(resp.order_id)
    }

    /// Cancel a specific order by ID.
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let _: CancelOrdersResponse = self.client
            .cancel_order(order_id)
            .await
            .map_err(|e| anyhow::anyhow!("Cancel order: {e}"))?;
        info!("[CANCEL] {}", order_id);
        Ok(())
    }

    /// Cancel all open orders.
    pub async fn cancel_all(&self) -> Result<()> {
        let _: CancelOrdersResponse = self.client
            .cancel_all_orders()
            .await
            .map_err(|e| anyhow::anyhow!("Cancel all: {e}"))?;
        info!("[CANCEL_ALL]");
        Ok(())
    }

    /// Check if an order is still live. Returns (is_live, size_matched).
    pub async fn get_order_status(&self, order_id: &str) -> Result<(bool, Decimal)> {
        let resp = self.client
            .order(order_id)
            .await
            .map_err(|e| anyhow::anyhow!("Get order: {e}"))?;
        use polymarket_client_sdk::clob::types::OrderStatusType;
        let is_live = matches!(resp.status, OrderStatusType::Live | OrderStatusType::Delayed);
        Ok((is_live, resp.size_matched))
    }
}

// ── WebSocket (orderbook + resolutions) ──

#[derive(Debug, Deserialize)]
struct WsBookMsg {
    asset_id: Option<String>,
    #[allow(dead_code)]
    market: Option<String>,
    bids: Option<Vec<WsLevel>>,
    asks: Option<Vec<WsLevel>>,
    #[allow(dead_code)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WsLevel {
    price: String,
    size: String,
}

#[derive(Debug, Deserialize)]
struct WsResolvedMsg {
    market: Option<String>,
    winner: Option<String>,
}

/// Run one CLOB WS session. Returns Err when disconnected.
async fn run_clob_session(
    tx: &mpsc::Sender<ExchangeEvent>,
    url: &str,
    asset_ids: &[String],
    cid_to_asset: &HashMap<String, Asset>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(url)
        .await
        .context("CLOB WS: failed to connect")?;

    info!("CLOB WS: connected");
    let (mut write, mut read) = ws_stream.split();

    // Subscribe
    let sub_msg = serde_json::json!({
        "type": "market",
        "assets_ids": asset_ids,
        "initial_dump": true,
        "custom_feature_enabled": true
    });
    use futures_util::SinkExt;
    write
        .send(Message::Text(sub_msg.to_string()))
        .await
        .context("CLOB WS: subscribe failed")?;

    info!("CLOB WS: subscribed to {} assets", asset_ids.len());

    // Ping task
    let (ping_stop_tx, mut ping_stop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if write.send(Message::Text("PING".to_string())).await.is_err() {
                        break;
                    }
                }
                _ = &mut ping_stop_rx => break,
            }
        }
    });
    // Read loop
    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if text == "PONG" { continue; }
                if let Ok(book) = serde_json::from_str::<WsBookMsg>(&text) {
                    if let Some(ref token_id) = book.asset_id {
                        let bids = book.bids.unwrap_or_default().iter().filter_map(|l| {
                            Some(BookLevel { price: l.price.parse().ok()?, size: l.size.parse().ok()? })
                        }).collect();
                        let asks = book.asks.unwrap_or_default().iter().filter_map(|l| {
                            Some(BookLevel { price: l.price.parse().ok()?, size: l.size.parse().ok()? })
                        }).collect();
                        let event = ExchangeEvent::BookUpdate(BookSnapshot {
                            token_id: token_id.clone(), bids, asks, ts: Utc::now(),
                        });
                        if tx.send(event).await.is_err() {
                            let _ = ping_stop_tx.send(());
                            return Err(anyhow::anyhow!("channel closed"));
                        }
                    }
                }
                if text.contains("winner") {
                    if let Ok(r) = serde_json::from_str::<WsResolvedMsg>(&text) {
                        if let (Some(cid), Some(winner)) = (&r.market, &r.winner) {
                            let outcome = if winner == "YES" { Outcome::Yes } else { Outcome::No };
                            let asset = cid_to_asset.get(cid.as_str()).copied().unwrap_or(Asset::BTC);
                            let event = ExchangeEvent::MarketResolved(MarketResolved {
                                condition_id: cid.clone(),
                                asset,
                                winner: outcome,
                                ts: Utc::now(),
                            });
                            if tx.send(event).await.is_err() {
                                let _ = ping_stop_tx.send(());
                                return Err(anyhow::anyhow!("channel closed"));
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => { warn!("CLOB WS: server closed"); break; }
            Err(e) => { error!("CLOB WS: read error: {e}"); break; }
            _ => {}
        }
    }
    let _ = ping_stop_tx.send(());
    Err(anyhow::anyhow!("disconnected"))
}

/// Spawn Polymarket CLOB WebSocket task with auto-reconnect.
pub async fn spawn_clob_ws(
    tx: mpsc::Sender<ExchangeEvent>,
    asset_ids: Vec<String>,
    cid_to_asset: HashMap<String, Asset>,
    ws_url: Option<&str>,
) -> Result<()> {
    let url = ws_url.unwrap_or(CLOB_WS_URL).to_string();
    info!("CLOB WS: connecting to {url}");

    tokio::spawn(async move {
        loop {
            match run_clob_session(&tx, &url, &asset_ids, &cid_to_asset).await {
                Err(e) if e.to_string().contains("channel closed") => {
                    error!("CLOB WS: channel closed, stopping");
                    break;
                }
                Err(e) => warn!("CLOB WS: session ended: {e}"),
                Ok(_) => {}
            }
            warn!("CLOB WS: reconnecting in 2s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    Ok(())
}
