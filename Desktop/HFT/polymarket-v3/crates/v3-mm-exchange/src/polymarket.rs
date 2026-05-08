//! Polymarket CLOB — order management via polymarket-client-sdk
//!
//! REST: place/cancel orders using the official SDK
//! WS: subscribe to orderbook updates, fills, market resolutions

use anyhow::{Context, Result};
use chrono::Utc;
use futures_util::StreamExt;
use v3_mm_core::Asset;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::types::{
    BookLevel, BookSnapshot, ExchangeEvent, MarketResolved, Outcome,
};

// ── On-chain constants ──
const POLYGON_RPC: &str = "https://polygon-bor-rpc.publicnode.com";
/// ConditionalTokens (CTF) contract on Polygon
const CTF_CONTRACT: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
/// NegRisk Adapter on Polygon
const NEG_RISK_ADAPTER: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";
/// USDC.e on Polygon
const USDC_E: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";

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
    wallet_addr: alloy::primitives::Address,
    http: reqwest::Client,
}

impl OrderClient {
    /// Create and authenticate an order client.
    /// `private_key` is the hex-encoded private key (with or without 0x prefix).
    pub async fn new(host: &str, private_key: &str) -> Result<Self> {
        let mut signer = PrivateKeySigner::from_str(private_key)
            .context("Invalid private key")?;
        signer.set_chain_id(Some(POLYGON));

        let wallet_addr = signer.address();

        let unauth = ClobClient::new(host, ClobConfig::default())
            .map_err(|e| anyhow::anyhow!("CLOB client init: {e}"))?;

        let client = unauth
            .authentication_builder(&signer)
            .authenticate()
            .await
            .map_err(|e| anyhow::anyhow!("CLOB auth: {e}"))?;

        info!("Polymarket CLOB: authenticated as {}", wallet_addr);
        Ok(Self {
            client,
            signer,
            wallet_addr,
            http: reqwest::Client::new(),
        })
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


    /// Place a GTC limit order (NOT post-only — may fill immediately as taker).
    /// Used as fallback when post_only is CROSS rejected, to guarantee fill.
    pub async fn place_gtc_order(
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
            "[ORDER_GTC] {} {:?} price={} size={} id={}",
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

    /// Raw eth_call helper
    async fn eth_call(&self, to: &str, data: &[u8]) -> Result<Vec<u8>> {
        let resp: serde_json::Value = self.http.post(POLYGON_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "eth_call",
                "params": [{"to": to, "data": format!("0x{}", hex::encode(data))}, "latest"]
            }))
            .send().await?.json().await?;
        let hex_str = resp["result"].as_str()
            .ok_or_else(|| anyhow::anyhow!("eth_call error: {}", resp))?;
        Ok(hex::decode(hex_str.trim_start_matches("0x"))?)
    }

    /// Query on-chain USDC.e balance (6 decimals) → returns whole dollars (f64)
    pub async fn usdc_balance(&self) -> Result<f64> {
        use alloy::primitives::U256 as AlloyU256;
        // ERC-20 balanceOf(address) selector = 0x70a08231
        let mut data = vec![0x70, 0xa0, 0x82, 0x31];
        // ABI-encode the address (left-padded to 32 bytes)
        let mut padded = [0u8; 32];
        padded[12..32].copy_from_slice(self.wallet_addr.as_slice());
        data.extend_from_slice(&padded);

        let result = self.eth_call(USDC_E, &data).await?;
        if result.len() < 32 { return Ok(0.0); }
        let val = AlloyU256::from_be_slice(&result[..32]);
        // USDC.e has 6 decimals
        let raw = val.to::<u64>();
        Ok(raw as f64 / 1_000_000.0)
    }
}

// ── REST book fetch (for pre-flight crossing guard) ──

#[derive(Debug, Deserialize)]
struct PriceResponse {
    price: Option<String>,
}

async fn fetch_price(http: &reqwest::Client, token_id: &str, side: &str) -> f64 {
    let resp = match http
        .get("https://clob.polymarket.com/price")
        .query(&[("token_id", token_id), ("side", side)])
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return 0.0,
    };
    match resp.json::<PriceResponse>().await {
        Ok(p) => p.price.and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

/// Fetch best bid/ask for a token from the CLOB REST /price endpoint.
/// Returns (best_bid, best_ask). Values are 0.0 if unavailable.
pub async fn fetch_book_top(http: &reqwest::Client, token_id: &str) -> (f64, f64) {
    let (bid, ask) = tokio::join!(
        fetch_price(http, token_id, "buy"),
        fetch_price(http, token_id, "sell"),
    );
    (bid, ask)
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

/// Run one CLOB WS session. Returns Ok(true) if downstream channel closed (stop reconnecting),
/// Ok(false) if disconnected from server (should reconnect), Err on fatal setup failure.
async fn run_clob_session(
    tx: &mpsc::Sender<ExchangeEvent>,
    url: &str,
    asset_ids: &[String],
    cid_to_asset: &HashMap<String, Asset>,
) -> Result<bool> {
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
                            return Ok(true); // downstream closed — stop reconnecting
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
                                return Ok(true); // downstream closed — stop reconnecting
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
    Ok(false) // disconnected from server — reconnect
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
                Ok(true) => {
                    error!("CLOB WS: downstream channel closed, stopping");
                    break;
                }
                Ok(false) => warn!("CLOB WS: session ended, reconnecting"),
                Err(e) => warn!("CLOB WS: session error: {e:#}"),
            }
            warn!("CLOB WS: reconnecting in 2s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    Ok(())
}

// ── On-chain Redeemer (CTF redeemPositions + merge) ──

use alloy::primitives::{Address, FixedBytes, U256 as AlloyU256, Bytes};
use alloy::sol;
use alloy::sol_types::SolCall;

sol! {
    interface IConditionalTokens {
        function redeemPositions(
            address collateralToken,
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256[] calldata indexSets
        ) external;

        function balanceOf(address owner, uint256 id) external view returns (uint256 balance);

        function mergePositions(
            address collateralToken,
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256[] calldata partition,
            uint256 amount
        ) external;

        function payoutDenominator(bytes32 conditionId) external view returns (uint256 denom);
    }
}

pub struct Redeemer {
    http: reqwest::Client,
    signer: PrivateKeySigner,
    wallet_addr: Address,
}

impl Redeemer {
    pub fn wallet_address(&self) -> Address { self.wallet_addr }

    pub fn new(private_key: &str) -> Result<Self> {
        let mut signer = PrivateKeySigner::from_str(private_key)
            .context("Invalid private key for Redeemer")?;
        signer.set_chain_id(Some(POLYGON));
        let wallet_addr = signer.address();
        Ok(Self { http: reqwest::Client::new(), signer, wallet_addr })
    }

    /// Raw eth_call helper
    async fn eth_call(&self, to: &str, data: &[u8]) -> Result<Vec<u8>> {
        let resp: serde_json::Value = self.http.post(POLYGON_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "eth_call",
                "params": [{"to": to, "data": format!("0x{}", hex::encode(data))}, "latest"]
            }))
            .send().await?.json().await?;
        let hex_str = resp["result"].as_str()
            .ok_or_else(|| anyhow::anyhow!("eth_call error: {}", resp))?;
        Ok(hex::decode(hex_str.trim_start_matches("0x"))?)
    }

    /// Get nonce and gas price via JSON-RPC
    async fn get_nonce_and_gas(&self) -> Result<(u64, u128)> {
        let addr_hex = format!("{:?}", self.wallet_addr);
        let resp: serde_json::Value = self.http.post(POLYGON_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "eth_getTransactionCount",
                "params": [addr_hex, "latest"]
            }))
            .send().await?.json().await?;
        let nonce_hex = resp["result"].as_str().unwrap_or("0x0");
        let nonce = u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)?;

        let resp2: serde_json::Value = self.http.post(POLYGON_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "eth_gasPrice", "params": []
            }))
            .send().await?.json().await?;
        let gas_hex = resp2["result"].as_str().unwrap_or("0x0");
        let gas_price = u128::from_str_radix(gas_hex.trim_start_matches("0x"), 16)?;

        Ok((nonce, gas_price))
    }

    /// Sign and send a legacy TX, return tx hash
    async fn send_tx(&self, to: Address, data: Vec<u8>) -> Result<String> {
        use alloy::consensus::TxLegacy;
        use alloy::network::TxSignerSync;
        use alloy::rlp::Encodable;

        let (nonce, gas_price) = self.get_nonce_and_gas().await?;

        let mut tx = TxLegacy {
            chain_id: Some(POLYGON as u64),
            nonce,
            gas_price: gas_price * 2,
            gas_limit: 300_000,
            to: alloy::primitives::TxKind::Call(to),
            value: AlloyU256::ZERO,
            input: Bytes::from(data),
        };

        let sig = self.signer.sign_transaction_sync(&mut tx)
            .map_err(|e| anyhow::anyhow!("sign tx: {e}"))?;

        // Manually RLP-encode signed legacy tx: rlp([nonce, gasPrice, gasLimit, to, value, data, v, r, s])
        let chain_id = POLYGON as u64;
        let v = if sig.v() { 1u64 } else { 0u64 } + chain_id * 2 + 35;

        let mut rlp_content = Vec::new();
        nonce.encode(&mut rlp_content);
        (gas_price * 2).encode(&mut rlp_content);
        300_000u64.encode(&mut rlp_content);
        to.encode(&mut rlp_content);
        AlloyU256::ZERO.encode(&mut rlp_content);
        tx.input.as_ref().encode(&mut rlp_content);
        v.encode(&mut rlp_content);
        sig.r().encode(&mut rlp_content);
        sig.s().encode(&mut rlp_content);

        let mut encoded = Vec::new();
        alloy::rlp::Header { list: true, payload_length: rlp_content.len() }.encode(&mut encoded);
        encoded.extend_from_slice(&rlp_content);

        let raw_hex = format!("0x{}", hex::encode(&encoded));

        let resp: serde_json::Value = self.http.post(POLYGON_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "eth_sendRawTransaction",
                "params": [raw_hex]
            }))
            .send().await?.json().await?;

        let hash = match resp["result"].as_str() {
            Some(h) => h.to_string(),
            None => return Err(anyhow::anyhow!("send tx failed: {}", resp)),
        };

        // Poll for receipt confirmation (up to 30s, every 3s)
        info!("[TX] submitted {}, waiting for confirmation...", hash);
        for attempt in 1..=10 {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            let receipt_resp: serde_json::Value = self.http.post(POLYGON_RPC)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0", "id": 1, "method": "eth_getTransactionReceipt",
                    "params": [&hash]
                }))
                .send().await?
                .json().await?;

            let receipt = &receipt_resp["result"];
            if receipt.is_null() {
                // Not mined yet
                if attempt == 10 {
                    warn!("[TX] {} not mined after 30s", hash);
                    return Err(anyhow::anyhow!("tx {} not confirmed after 30s", hash));
                }
                continue;
            }

            // Check status: 0x1 = success, 0x0 = reverted
            let status = receipt["status"].as_str().unwrap_or("0x0");
            if status == "0x1" {
                info!("[TX] {} confirmed (attempt {})", hash, attempt);
                return Ok(hash);
            } else {
                warn!("[TX] {} reverted on-chain", hash);
                return Err(anyhow::anyhow!("tx {} reverted (status={})", hash, status));
            }
        }

        unreachable!()
    }

    /// Query on-chain USDC.e balance (6 decimals) → returns whole dollars (f64)
    pub async fn usdc_balance(&self) -> Result<f64> {
        // ERC-20 balanceOf(address) selector = 0x70a08231
        let mut data = vec![0x70, 0xa0, 0x82, 0x31];
        // ABI-encode the address (left-padded to 32 bytes)
        let mut padded = [0u8; 32];
        padded[12..32].copy_from_slice(self.wallet_addr.as_slice());
        data.extend_from_slice(&padded);

        let result = self.eth_call(USDC_E, &data).await?;
        if result.len() < 32 { return Ok(0.0); }
        let val = AlloyU256::from_be_slice(&result[..32]);
        // USDC.e has 6 decimals
        let raw = val.to::<u64>();
        Ok(raw as f64 / 1_000_000.0)
    }

    /// Check CTF token balance for a given token_id
    pub async fn token_balance(&self, token_id: &str) -> Result<u64> {
        let tid = AlloyU256::from_str_radix(token_id, 10)
            .or_else(|_| AlloyU256::from_str_radix(token_id.trim_start_matches("0x"), 16))
            .context("bad token_id")?;
        let call = IConditionalTokens::balanceOfCall {
            owner: self.wallet_addr, id: tid,
        };
        let result = self.eth_call(CTF_CONTRACT, &call.abi_encode()).await?;
        if result.len() < 32 { return Ok(0); }
        let val = AlloyU256::from_be_slice(&result[..32]);
        Ok(val.to::<u64>())
    }

    /// Check if a condition has been resolved (payoutDenominator > 0)
    pub async fn is_resolved(&self, condition_id: &str) -> Result<bool> {
        let cid: FixedBytes<32> = condition_id.parse().context("bad condition_id")?;
        let call = IConditionalTokens::payoutDenominatorCall { conditionId: cid };
        let result = self.eth_call(CTF_CONTRACT, &call.abi_encode()).await?;
        if result.len() < 32 { return Ok(false); }
        let val = AlloyU256::from_be_slice(&result[..32]);
        Ok(val > AlloyU256::ZERO)
    }

    /// Redeem winning positions for a resolved condition
    pub async fn redeem(&self, condition_id: &str) -> Result<String> {
        let ctf_addr: Address = CTF_CONTRACT.parse().unwrap();
        let collateral: Address = USDC_E.parse().unwrap();
        let cid: FixedBytes<32> = condition_id.parse().context("bad condition_id")?;
        let call = IConditionalTokens::redeemPositionsCall {
            collateralToken: collateral,
            parentCollectionId: FixedBytes::<32>::ZERO,
            conditionId: cid,
            indexSets: vec![AlloyU256::from(1), AlloyU256::from(2)],
        };
        let hash = self.send_tx(ctf_addr, call.abi_encode()).await?;
        info!("[REDEEM] condition={} tx={}", condition_id, hash);
        Ok(hash)
    }

    /// Merge equal YES+NO positions back into USDC.e collateral
    pub async fn merge(&self, condition_id: &str, token_id_yes: &str, token_id_no: &str) -> Result<Option<String>> {
        let yes_bal = self.token_balance(token_id_yes).await?;
        let no_bal = self.token_balance(token_id_no).await?;
        let merge_amount = yes_bal.min(no_bal);
        if merge_amount == 0 { return Ok(None); }

        let ctf_addr: Address = CTF_CONTRACT.parse().unwrap();
        let collateral: Address = USDC_E.parse().unwrap();
        let cid: FixedBytes<32> = condition_id.parse().context("bad condition_id")?;
        let call = IConditionalTokens::mergePositionsCall {
            collateralToken: collateral,
            parentCollectionId: FixedBytes::<32>::ZERO,
            conditionId: cid,
            partition: vec![AlloyU256::from(1), AlloyU256::from(2)],
            amount: AlloyU256::from(merge_amount),
        };
        let hash = self.send_tx(ctf_addr, call.abi_encode()).await?;
        info!("[MERGE] condition={} amount={} tx={}", condition_id, merge_amount, hash);
        Ok(Some(hash))
    }
}
