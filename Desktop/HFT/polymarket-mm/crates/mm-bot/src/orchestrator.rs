//! Orchestrator — main event loop that wires everything together

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use mm_core::fair_price::{prob_up, VolEstimator};
use mm_core::inventory::{InventoryManager, PositionVerdict};
use mm_core::quoter::{Quoter, QuoteState, QuoterConfig};
use mm_core::Asset;
use mm_exchange::polymarket::{OrderClient, spawn_clob_ws};
use mm_exchange::types::*;
use mm_risk::limits::{RiskAction, RiskManager};
use crate::trade_log::TradeLogger;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Per-asset state
struct AssetState {
    rtds_price: f64,
    binance_mid: f64,
    vol: VolEstimator,
}

/// Per-market (15-min window) state
struct WindowState {
    info: MarketInfo,
    open_price: f64,
    quotes: QuoteState,
}

pub struct Orchestrator {
    quoter: Quoter,
    inventory: InventoryManager,
    risk: RiskManager,
    order_client: Option<Arc<OrderClient>>,
    trade_log: TradeLogger,
    event_tx: mpsc::Sender<ExchangeEvent>,

    assets: HashMap<Asset, AssetState>,
    windows: HashMap<String, WindowState>,
    assets_filter: Option<HashSet<Asset>>,
    clob_subscribed: HashSet<String>,

    dry_run: bool,
    wind_down_secs: f64,
    stop_quote_secs: f64,
    heartbeat_count: u64,
}

impl Orchestrator {
    pub fn new(
        quoter_config: QuoterConfig,
        max_per_market: f64,
        max_per_asset: f64,
        max_total: f64,
        risk_config: mm_risk::limits::RiskConfig,
        dry_run: bool,
        wind_down_secs: f64,
        stop_quote_secs: f64,
        order_client: Option<OrderClient>,
        assets_filter: Option<HashSet<Asset>>,
        event_tx: mpsc::Sender<ExchangeEvent>,
    ) -> Self {
        let mut assets = HashMap::new();
        for &asset in Asset::all() {
            assets.insert(asset, AssetState {
                rtds_price: 0.0,
                binance_mid: 0.0,
                vol: VolEstimator::new(300.0, 0.15, 2.0),
            });
        }

        Self {
            quoter: Quoter::new(quoter_config),
            inventory: InventoryManager::new(max_per_market, max_per_asset, max_total),
            risk: RiskManager::new(risk_config),
            order_client: order_client.map(Arc::new),
            trade_log: TradeLogger::new(),
            event_tx,
            assets,
            windows: HashMap::new(),
            assets_filter,
            clob_subscribed: HashSet::new(),
            dry_run,
            wind_down_secs,
            stop_quote_secs,
            heartbeat_count: 0,
        }
    }

    /// Main event loop
    pub async fn run(&mut self, mut rx: mpsc::Receiver<ExchangeEvent>) -> Result<()> {
        let mut heartbeat = tokio::time::interval(tokio::time::Duration::from_millis(500));

        info!("Orchestrator: starting event loop (dry_run={})", self.dry_run);

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    self.handle_event(event).await;
                }
                _ = heartbeat.tick() => {
                    self.on_heartbeat().await;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: ExchangeEvent) {
        match event {
            ExchangeEvent::RtdsPrice(p) => self.on_rtds_price(p).await,
            ExchangeEvent::RtdsAlive => { self.risk.touch_rtds(); },
            ExchangeEvent::BinanceTick(t) => self.on_binance_tick(t),
            ExchangeEvent::BookUpdate(b) => self.on_book_update(b),
            ExchangeEvent::OrderFill(f) => self.on_fill(f),
            ExchangeEvent::MarketResolved(r) => self.on_resolved(r),
            ExchangeEvent::MarketReady(m) => self.on_market_ready(m),
            ExchangeEvent::FeedStale { source } => {
                warn!("Feed stale: {source}");
            }
        }
    }

    async fn on_rtds_price(&mut self, p: RtdsPrice) {
        self.risk.touch_rtds();
        if let Some(state) = self.assets.get_mut(&p.asset) {
            state.rtds_price = p.price;
        }
        // Requote all active windows for this asset
        self.try_requote_asset(p.asset).await;
    }

    fn on_binance_tick(&mut self, t: BinanceTick) {
        self.risk.touch_binance();
        if let Some(state) = self.assets.get_mut(&t.asset) {
            let mid = (t.bid + t.ask) / 2.0;
            state.binance_mid = mid;
            state.vol.update(mid);
        }
    }

    fn on_book_update(&mut self, _book: BookSnapshot) {
        // Store latest book for spread analysis (future enhancement)
    }

    fn on_fill(&mut self, fill: Fill) {
        let size = match fill.side {
            FillSide::BuyYes => fill.size as i64,
            FillSide::SellYes => -(fill.size as i64),
        };
        self.inventory.record_fill(
            &fill.condition_id,
            fill.asset,
            size,
            fill.price,
        );

        let side_str = match fill.side {
            FillSide::BuyYes => "BUY",
            FillSide::SellYes => "SELL",
        };
        let inv = self.inventory.position(&fill.condition_id);
        let asset_state = self.assets.get(&fill.asset);
        let rtds_price = asset_state.map_or(0.0, |s| s.rtds_price);
        let binance_mid = asset_state.map_or(0.0, |s| s.binance_mid);
        let vol = asset_state.map_or(0.0, |s| {
            if s.vol.has_data() { s.vol.annualized_vol() } else { 0.50 }
        });
        let window = self.windows.get(&fill.condition_id);
        let remaining_secs = window.map_or(0.0, |w| {
            (w.info.end_time - Utc::now()).num_seconds().max(0) as f64
        });
        let fair_p = window.map_or(0.0, |w| w.quotes.last_fair_p);

        self.trade_log.log_fill(
            fill.asset, &fill.condition_id, side_str,
            fill.price, fill.size, fair_p,
            rtds_price, binance_mid, vol, remaining_secs,
            inv, self.risk.daily_pnl(),
        );

        info!(
            "[FILL] {} {} {:?} price={} size={} inv={}",
            fill.asset,
            fill.condition_id,
            fill.side,
            fill.price,
            fill.size,
            inv,
        );
    }

    fn on_resolved(&mut self, r: MarketResolved) {
        let won = r.winner == Outcome::Yes;
        let pos_before = self.inventory.position(&r.condition_id);
        self.inventory.record_resolution(&r.condition_id, won);

        let pnl = if won {
            pos_before as f64 * 1.0
        } else {
            pos_before as f64 * 0.0
        };
        self.risk.record_pnl(pnl);

        let winner_str = match r.winner {
            Outcome::Yes => "YES",
            Outcome::No => "NO",
        };
        self.trade_log.log_settlement(
            r.asset, &r.condition_id, winner_str,
            pos_before, self.risk.daily_pnl(),
        );

        info!(
            "[SETTLE] {} {} winner={:?} pos_before={} daily_pnl={:.2}",
            r.asset, r.condition_id, r.winner, pos_before, self.risk.daily_pnl()
        );

        self.windows.remove(&r.condition_id);
        self.inventory.cleanup_settled();
    }

    fn on_market_ready(&mut self, market: MarketInfo) {
        // Asset filter: skip markets for assets we're not trading
        if let Some(ref filter) = self.assets_filter {
            if !filter.contains(&market.asset) {
                return;
            }
        }

        let open_price = self.assets.get(&market.asset)
            .map(|s| s.rtds_price)
            .unwrap_or(0.0);

        if open_price > 0.0 {
            info!(
                "[MARKET] {} {} open_price={:.2} ends={}",
                market.asset, market.condition_id, open_price, market.end_time
            );
        } else {
            info!(
                "[MARKET] {} {} open_price=PENDING (awaiting RTDS) ends={}",
                market.asset, market.condition_id, market.end_time
            );
        }

        // Spawn CLOB WS subscription for this market (live mode only, once per condition_id)
        if !self.dry_run && !self.clob_subscribed.contains(&market.condition_id) {
            let asset_ids = vec![
                market.token_id_yes.clone(),
                market.token_id_no.clone(),
            ];
            let mut cid_to_asset = HashMap::new();
            cid_to_asset.insert(market.condition_id.clone(), market.asset);

            let tx = self.event_tx.clone();
            let cid = market.condition_id.clone();
            tokio::spawn(async move {
                if let Err(e) = spawn_clob_ws(tx, asset_ids, cid_to_asset, None).await {
                    error!("[CLOB_WS] failed to spawn for {}: {e}", cid);
                }
            });
            self.clob_subscribed.insert(market.condition_id.clone());
        }

        self.windows.insert(market.condition_id.clone(), WindowState {
            info: market,
            open_price,
            quotes: QuoteState::default(),
        });
    }

    async fn try_requote_asset(&mut self, asset: Asset) {
        let risk_action = self.risk.check();
        if risk_action == RiskAction::CancelAll {
            self.cancel_all_quotes().await;
            return;
        }

        let asset_state = match self.assets.get(&asset) {
            Some(s) => s,
            None => return,
        };

        let rtds_price = asset_state.rtds_price;
        let binance_mid = asset_state.binance_mid;
        let vol = if asset_state.vol.has_data() {
            asset_state.vol.annualized_vol()
        } else {
            0.50
        };

        let spread_mult = if risk_action == RiskAction::WidenSpread { 2.0 } else { 1.0 };

        let cids: Vec<String> = self.windows
            .iter()
            .filter(|(_, w)| w.info.asset == asset)
            .map(|(cid, _)| cid.clone())
            .collect();

        for cid in cids {
            let window = match self.windows.get_mut(&cid) {
                Some(w) => w,
                None => continue,
            };

            // Lazily set open_price on first RTDS tick
            if window.open_price == 0.0 {
                if rtds_price > 0.0 {
                    window.open_price = rtds_price;
                    info!(
                        "[MARKET] {} open_price set to {:.2} (lazy init)",
                        cid, rtds_price
                    );
                } else {
                    continue; // Still no RTDS price, skip this window
                }
            }

            let now = Utc::now();
            let remaining_secs = (window.info.end_time - now).num_seconds().max(0) as f64;

            // Stop quoting near expiry
            if remaining_secs < self.stop_quote_secs {
                if window.quotes.bid_order_id.is_some() || window.quotes.ask_order_id.is_some() {
                    info!("[WIND_DOWN] {} remaining={:.0}s, cancelling", cid, remaining_secs);
                    self.cancel_window_quotes(&cid).await;
                }
                continue;
            }

            let fair_p = prob_up(rtds_price, window.open_price, remaining_secs, vol);

            if !window.quotes.should_requote(fair_p, &self.quoter.config) {
                continue;
            }

            let inv = self.inventory.position(&cid);
            let verdict = self.inventory.can_open(&cid, asset, fair_p);

            let effective_vol = vol * spread_mult;
            let (bid, ask) = self.quoter.generate_quotes(
                fair_p, inv, remaining_secs, effective_vol,
                &window.info.token_id_yes,
            );

            // Apply position verdict
            let bid = match verdict {
                PositionVerdict::Reject => None,
                PositionVerdict::ReduceOnly if inv >= 0 => None,
                _ => bid,
            };
            let ask = match verdict {
                PositionVerdict::Reject => None,
                PositionVerdict::ReduceOnly if inv <= 0 => None,
                _ => ask,
            };

            // Cancel old orders before placing new ones
            if window.quotes.bid_order_id.is_some() || window.quotes.ask_order_id.is_some() {
                self.cancel_window_quotes(&cid).await;
            }

            // Place new orders
            let mut new_bid_id = None;
            let mut new_ask_id = None;

            if let Some(ref b) = bid {
                info!(
                    "[QUOTE] {} BUY  price={} size={} fair={:.3} vol={:.2} rem={:.0}s inv={}",
                    cid, b.price, b.size, fair_p, vol, remaining_secs, inv
                );
                self.trade_log.log_quote(
                    asset, &cid, "BUY", b.price, b.size, fair_p,
                    rtds_price, binance_mid, vol, remaining_secs,
                    inv, self.risk.daily_pnl(),
                );
                if !self.dry_run {
                    if let Some(ref client) = self.order_client {
                        let size_dec = rust_decimal::Decimal::from(b.size);
                        match client.place_limit_order(
                            &b.token_id, OrderSide::Buy, b.price, size_dec,
                        ).await {
                            Ok(id) => new_bid_id = Some(id),
                            Err(e) => error!("[ORDER_ERR] bid {}: {e}", cid),
                        }
                    }
                }
            }

            if let Some(ref a) = ask {
                info!(
                    "[QUOTE] {} SELL price={} size={} fair={:.3} vol={:.2} rem={:.0}s inv={}",
                    cid, a.price, a.size, fair_p, vol, remaining_secs, inv
                );
                self.trade_log.log_quote(
                    asset, &cid, "SELL", a.price, a.size, fair_p,
                    rtds_price, binance_mid, vol, remaining_secs,
                    inv, self.risk.daily_pnl(),
                );
                if !self.dry_run {
                    if let Some(ref client) = self.order_client {
                        let size_dec = rust_decimal::Decimal::from(a.size);
                        match client.place_limit_order(
                            &a.token_id, OrderSide::Sell, a.price, size_dec,
                        ).await {
                            Ok(id) => new_ask_id = Some(id),
                            Err(e) => error!("[ORDER_ERR] ask {}: {e}", cid),
                        }
                    }
                }
            }

            // Update quote state
            let window = self.windows.get_mut(&cid).unwrap();
            window.quotes.last_fair_p = fair_p;
            window.quotes.bid_order_id = new_bid_id;
            window.quotes.ask_order_id = new_ask_id;
            window.quotes.bid_price = bid.as_ref().map(|b| b.price);
            window.quotes.ask_price = ask.as_ref().map(|a| a.price);
        }
    }

    /// Cancel all quotes across all windows
    async fn cancel_all_quotes(&mut self) {
        let had_quotes = self.windows.values().any(|w| {
            w.quotes.bid_order_id.is_some() || w.quotes.ask_order_id.is_some()
        });
        if !had_quotes { return; }

        if !self.dry_run {
            if let Some(ref client) = self.order_client {
                if let Err(e) = client.cancel_all().await {
                    error!("[CANCEL_ALL_ERR] {e}");
                }
            }
        }
        for (_, window) in self.windows.iter_mut() {
            window.quotes = QuoteState::default();
        }
        info!("[CANCEL_ALL] risk triggered");
    }

    /// Cancel quotes for a specific window
    async fn cancel_window_quotes(&mut self, cid: &str) {
        if let Some(window) = self.windows.get_mut(cid) {
            if !self.dry_run {
                if let Some(ref client) = self.order_client {
                    if let Some(ref id) = window.quotes.bid_order_id {
                        let _ = client.cancel_order(id).await;
                    }
                    if let Some(ref id) = window.quotes.ask_order_id {
                        let _ = client.cancel_order(id).await;
                    }
                }
            }
            window.quotes = QuoteState::default();
        }
    }

    async fn on_heartbeat(&mut self) {
        self.heartbeat_count += 1;

        // Only check risk when we have active windows
        if !self.windows.is_empty() {
            let risk_action = self.risk.check();
            if risk_action == RiskAction::CancelAll {
                self.cancel_all_quotes().await;
            }
        }

        // Order reconciliation every ~15s (30 heartbeats × 500ms), live mode only
        if !self.dry_run && self.heartbeat_count % 30 == 0 {
            self.reconcile_orders().await;
        }

        // Periodic snapshot every ~30s (60 heartbeats × 500ms)
        if self.heartbeat_count % 60 == 0 {
            let daily_pnl = self.risk.daily_pnl();
            for &asset in Asset::all() {
                if let Some(s) = self.assets.get(&asset) {
                    if s.rtds_price > 0.0 {
                        let vol = if s.vol.has_data() { s.vol.annualized_vol() } else { 0.50 };
                        self.trade_log.log_snapshot(
                            asset, s.rtds_price, s.binance_mid, vol, daily_pnl,
                        );
                    }
                }
            }
        }

        // Check for expired windows
        let now = Utc::now();
        let expired: Vec<String> = self.windows
            .iter()
            .filter(|(_, w)| w.info.end_time <= now)
            .map(|(cid, _)| cid.clone())
            .collect();

        for cid in expired {
            // Cancel any remaining quotes and remove expired windows
            self.cancel_window_quotes(&cid).await;
            info!("[EXPIRED] window {} past end_time, removing", cid);
            self.windows.remove(&cid);
        }
    }

    /// Check tracked orders against exchange — detect fills for orders that disappeared
    async fn reconcile_orders(&mut self) {
        let client = match self.order_client {
            Some(ref c) => c.clone(),
            None => return,
        };

        // Collect all tracked order IDs with their window context
        let mut tracked: Vec<(String, String, bool)> = Vec::new(); // (order_id, cid, is_bid)
        for (cid, window) in &self.windows {
            if let Some(ref id) = window.quotes.bid_order_id {
                tracked.push((id.clone(), cid.clone(), true));
            }
            if let Some(ref id) = window.quotes.ask_order_id {
                tracked.push((id.clone(), cid.clone(), false));
            }
        }

        if tracked.is_empty() { return; }

        for (order_id, cid, is_bid) in tracked {
            match client.get_order_status(&order_id).await {
                Ok((is_live, size_matched)) => {
                    if !is_live {
                        let matched_u64 = size_matched.to_string().parse::<u64>().unwrap_or(0);
                        if matched_u64 > 0 {
                            // Order was filled — synthesize fill event
                            let window = match self.windows.get(&cid) {
                                Some(w) => w,
                                None => continue,
                            };
                            let side = if is_bid { FillSide::BuyYes } else { FillSide::SellYes };
                            let price = if is_bid {
                                window.quotes.bid_price.unwrap_or_default()
                            } else {
                                window.quotes.ask_price.unwrap_or_default()
                            };
                            let fill = Fill {
                                order_id: order_id.clone(),
                                condition_id: cid.clone(),
                                asset: window.info.asset,
                                side,
                                price,
                                size: matched_u64,
                                ts: Utc::now(),
                            };
                            info!(
                                "[RECONCILE] detected fill: {} {:?} price={} size={}",
                                cid, side, price, matched_u64
                            );
                            self.on_fill(fill);
                        }
                        // Clear the order from quote state
                        if let Some(w) = self.windows.get_mut(&cid) {
                            if is_bid { w.quotes.bid_order_id = None; }
                            else { w.quotes.ask_order_id = None; }
                        }
                    }
                }
                Err(e) => {
                    warn!("[RECONCILE] failed to check order {}: {e}", order_id);
                }
            }
        }
    }
}