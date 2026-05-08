//! Gengar orchestrator — the only live strategy in the project.
//!
//! Responsibilities per tick (`tick_interval_ms`, default 500ms):
//!   1. Refresh the 3 active windows (5m / 15m / 1h) from Gamma API when slugs roll.
//!   2. Anchor the window's `opening_price` on the first Binance tick observed.
//!   3. Close-out bookkeeping when a window rolls: feed the closing btc_delta_pct
//!      into that window's VolTracker.
//!   4. Per open window without a position: evaluate Gengar → maybe place a
//!      non-post-only GTC BUY (`execute_gengar_order`).
//!   5. Per open position: compute current edge, dynamic profit-lock, or window-end
//!      close-out.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use v3_execution::{OrderExecutor, OrderTracker, TtlManager};
use v3_mm_core::Asset;
use v3_signal::BinanceMonitor as V3BinanceMonitor;
use v3_strategy::{gengar, GengarConfig, VolTracker};

use crate::binance_integration::BinanceFeed;
use crate::clob_integration::{BookCache, ClobFeed};
use crate::config::BotConfig;
use crate::event_logger::EventLogger;
use crate::hourly_stats::HourlyStats;
use crate::notifier::Notifier;
use crate::window_manager::{WindowKind, WindowManager, WindowState};

/// An open Gengar position (single leg). We allow at most one per (asset, window).
#[derive(Debug, Clone)]
pub struct OpenPosition {
    pub pair_id: String,
    pub asset: v3_mm_core::Asset,
    pub window_kind: WindowKind,
    /// Which side we bought (UP or DOWN).
    pub side: gengar::Side,
    pub token_id: String,
    pub entry_price: f64,
    pub shares: u64,
    pub initial_edge: f64,
    pub order_id: String,
    pub entered_at: DateTime<Utc>,
}

pub struct GengarOrchestrator {
    config: BotConfig,

    window_manager: Arc<WindowManager>,
    binance_feed: Arc<RwLock<BinanceFeed>>,
    clob_feed: Arc<RwLock<ClobFeed>>,
    book_cache: Arc<RwLock<BookCache>>,
    executor: Arc<OrderExecutor>,

    vol_trackers: Arc<RwLock<HashMap<(v3_mm_core::Asset, WindowKind), VolTracker>>>,
    /// Tracks the last closing price per (asset, WindowKind) so we can record
    /// delta when a window rolls over.
    last_window_ts: Arc<RwLock<HashMap<(v3_mm_core::Asset, WindowKind), u64>>>,

    /// Kelly bankroll (USDC). Updated via balance snapshots.
    bankroll_usdc: Arc<RwLock<f64>>,

    positions: Arc<RwLock<HashMap<String, OpenPosition>>>,

    /// Latest Binance spot per asset (cached from ticks between polls).
    latest_prices: Arc<RwLock<HashMap<v3_mm_core::Asset, f64>>>,

    gengar_cfg: GengarConfig,

    last_refresh_secs: Arc<RwLock<i64>>,
    last_bankroll_refresh_secs: Arc<RwLock<i64>>,

    event_log: Arc<EventLogger>,
    notifier: Notifier,
    hourly_stats: Arc<HourlyStats>,
}

impl GengarOrchestrator {
    pub async fn new(config: BotConfig, initial_bankroll_usdc: f64) -> Result<Self> {
        info!(
            "GengarOrchestrator init (dry_run={}, bankroll=${:.2})",
            config.dry_run, initial_bankroll_usdc
        );

        let window_manager = Arc::new(WindowManager::new());
        let binance_feed = Arc::new(RwLock::new(BinanceFeed::new().await?));
        let clob_feed = Arc::new(RwLock::new(
            ClobFeed::new(vec![], HashMap::new()).await?,
        ));
        let book_cache = Arc::new(RwLock::new(BookCache::new()));

        // Executor plumbing (TTL/CancelMonitor/OrderTracker still needed for API
        // compatibility even though Gengar doesn't use TTL cancellation).
        let ttl_manager = Arc::new(RwLock::new(TtlManager::new()));
        let order_tracker = Arc::new(RwLock::new(OrderTracker::new()));
        // CancelMonitor needs a BinanceMonitor, but Gengar never starts its
        // run-loop (we don't call cancel_monitor.run()). We only construct one
        // to satisfy `OrderExecutor::new`. A dummy monitor for BTC is fine.
        let dummy_binance = Arc::new(RwLock::new(V3BinanceMonitor::new(
            Asset::BTC, 0.5, 0.1,
        )));
        let cancel_monitor = Arc::new(v3_execution::CancelMonitor::new(
            dummy_binance,
            order_tracker.clone(),
            0.5,
            1000,
        ));
        let private_key = std::env::var("PRIVATE_KEY").ok();
        let executor = Arc::new(
            OrderExecutor::new(
                private_key,
                ttl_manager.clone(),
                cancel_monitor,
                order_tracker,
                config.dry_run,
            )
            .await?,
        );

        let vol_trackers = {
            let mut m = HashMap::new();
            for asset in &config.strategies.gengar.tracked_assets {
                for k in WindowKind::all() {
                    m.insert((*asset, k), VolTracker::new(12));
                }
            }
            Arc::new(RwLock::new(m))
        };

        // CSV event logger under ./logs/. Directory auto-created.
        let event_log = Arc::new(
            EventLogger::new("logs").context("init EventLogger")?,
        );
        let notifier = Notifier::from_env();
        if notifier.is_enabled() {
            info!("📣 Telegram notifier enabled");
        }

        // Recover any positions left open by a prior run. Silent no-op if
        // logs/trades.csv is missing or has no unmatched entries.
        let recovered = crate::state_recovery::recover_open_positions("logs/trades.csv")
            .context("recover_open_positions")?;
        if !recovered.is_empty() {
            info!(
                "🔁 Recovered {} open position(s) from logs/trades.csv",
                recovered.len()
            );
            for p in &recovered {
                info!(
                    "  ↪ {} {:?} {:?} @ {:.4} x {} (initial_edge={:.4})",
                    p.pair_id, p.window_kind, p.side, p.entry_price, p.shares, p.initial_edge
                );
            }
            let summary: Vec<String> = recovered.iter()
                .map(|p| format!("{:?} {:?} @ {:.4} x {}", p.window_kind, p.side, p.entry_price, p.shares))
                .collect();
            notifier
                .notify(&format!(
                    "🔁 Recovered {} open position(s) on restart:\n{}",
                    recovered.len(),
                    summary.join("\n"),
                ))
                .await;
        }
        let positions_map: HashMap<String, OpenPosition> =
            recovered.into_iter().map(|p| (p.pair_id.clone(), p)).collect();

        Ok(Self {
            gengar_cfg: config.strategies.gengar.clone(),
            config,
            window_manager,
            binance_feed,
            clob_feed,
            book_cache,
            executor,
            vol_trackers,
            last_window_ts: Arc::new(RwLock::new(HashMap::new())),
            bankroll_usdc: Arc::new(RwLock::new(initial_bankroll_usdc)),
            positions: Arc::new(RwLock::new(positions_map)),
            latest_prices: Arc::new(RwLock::new(HashMap::new())),
            last_refresh_secs: Arc::new(RwLock::new(0)),
            last_bankroll_refresh_secs: Arc::new(RwLock::new(0)),
            event_log,
            notifier,
            hourly_stats: Arc::new(HourlyStats::new()),
        })
    }

    /// Main entry. Runs the Gengar tick loop forever (until aborted).
    pub async fn run(&self) -> Result<()> {
        info!(
            "▶ GengarOrchestrator running; tick_interval_ms={}",
            self.config.tick_interval_ms
        );
        let bk = *self.bankroll_usdc.read().await;
        self.notifier
            .notify(&format!(
                "🚀 Gengar bot started\nmode={}\nbankroll=${:.2}\ntick={}ms",
                if self.config.dry_run { "DRY-RUN" } else { "LIVE" },
                bk,
                self.config.tick_interval_ms,
            ))
            .await;
        let interval = std::time::Duration::from_millis(self.config.tick_interval_ms);
        loop {
            if let Err(e) = self.tick().await {
                warn!("tick error: {:#}", e);
            }
            tokio::time::sleep(interval).await;
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("GengarOrchestrator shutdown");
        Ok(())
    }

    /// One tick: (1) drain Binance, (2) drain CLOB, (3) refresh windows,
    /// (4) roll-over bookkeeping, (5) per-window evaluate, (6) manage positions.
    async fn tick(&self) -> Result<()> {
        // --- 1. Drain Binance ticks, keep latest price per asset ---
        {
            let mut feed = self.binance_feed.write().await;
            let mut cache = self.latest_prices.write().await;
            while let Some(tick) = feed.next_tick().await {
                // Only track assets in config.tracked_assets.
                if !self.gengar_cfg.tracked_assets.contains(&tick.asset) {
                    continue;
                }
                let mid = (tick.bid + tick.ask) / 2.0;
                if mid > 0.0 {
                    cache.insert(tick.asset, mid);
                }
            }
        }

        // --- 2. Drain CLOB book updates into cache ---
        {
            let mut feed = self.clob_feed.write().await;
            let updates = feed.drain_book_updates().await;
            let mut cache = self.book_cache.write().await;
            for snap in updates {
                cache.update(snap);
            }
        }

        // --- 3. Refresh windows every 10s ---
        //
        // When a window rolls, `refresh_all` returns the OLD WindowState so
        // we can compute its closing delta and feed the per-window VolTracker.
        let now = chrono::Utc::now().timestamp();
        let mut last_refresh = self.last_refresh_secs.write().await;
        if now - *last_refresh >= 10 {
            *last_refresh = now;
            drop(last_refresh);
            let rolled = self
                .window_manager
                .refresh_all(&self.gengar_cfg.tracked_assets)
                .await?;
            self.record_closing_deltas(&rolled).await;
            self.ensure_clob_subscriptions().await?;
        } else {
            drop(last_refresh);
        }

        // --- 3b. Refresh bankroll from on-chain USDC every 60s (live only) ---
        if !self.config.dry_run {
            let mut last_bk = self.last_bankroll_refresh_secs.write().await;
            if now - *last_bk >= 60 {
                *last_bk = now;
                drop(last_bk);
                self.refresh_bankroll().await;
            } else {
                drop(last_bk);
            }
        }

        // --- 4. Detect window roll-over + anchor opening price ---
        self.handle_rollovers_and_openings().await;

        // --- 5. Per-(asset, window) evaluate ---
        for &asset in &self.gengar_cfg.tracked_assets {
            for kind in WindowKind::all() {
                if let Err(e) = self.tick_window(asset, kind).await {
                    warn!("tick_window {:?} {:?}: {:#}", asset, kind, e);
                }
            }
        }

        // --- 6. Manage open positions (profit lock, window-end cleanup) ---
        self.manage_positions().await?;

        // --- 7. Hourly Telegram summary (flush once per wall-clock hour) ---
        if self.hourly_stats.should_flush(chrono::Utc::now()) {
            if let Some(msg) = self.hourly_stats.flush() {
                info!("{}", msg);
                self.notifier.notify(&msg).await;
            }
        }

        Ok(())
    }

    /// Refresh bankroll from on-chain USDC.e balance. Live mode only.
    ///
    /// Locked-in capital (already in open Gengar positions) is subtracted from
    /// the on-chain balance before writing. This gives Kelly a view of
    /// *deployable* capital, not total wallet value. Without this subtraction
    /// we'd double-size: new trades would size against the full balance, but
    /// the balance already reflects spend on open positions too.
    async fn refresh_bankroll(&self) {
        let raw = match self.executor.usdc_balance().await {
            Ok(Some(b)) => b,
            Ok(None) => return, // dry_run path
            Err(e) => {
                warn!("usdc_balance refresh failed: {:#}", e);
                return;
            }
        };
        // Subtract locked capital (sum of open positions' notional).
        let locked: f64 = self
            .positions
            .read()
            .await
            .values()
            .map(|p| p.entry_price * p.shares as f64)
            .sum();
        let deployable = (raw - locked).max(0.0);
        let mut bk = self.bankroll_usdc.write().await;
        let prev = *bk;
        *bk = deployable;
        info!(
            "💰 Bankroll refresh: wallet=${:.2} − locked=${:.2} = deployable=${:.2} (was ${:.2})",
            raw, locked, deployable, prev
        );
    }

    async fn ensure_clob_subscriptions(&self) -> Result<()> {
        let tokens = self.window_manager.all_tokens().await;
        if tokens.is_empty() {
            return Ok(());
        }
        let mut feed = self.clob_feed.write().await;
        feed.ensure_subscribed(tokens, HashMap::new()).await?;
        Ok(())
    }

    /// Given the list of `(asset, kind, old_state)` that just rolled over,
    /// compute each one's closing `btc_delta_pct` using the per-asset Binance
    /// spot as an approximation for the close price and push it into the
    /// per-(asset, kind) VolTracker.
    ///
    /// Approximation note: Polymarket settles on the Binance 1m bar at window
    /// end. Between rollover detection and this call we may be a few hundred
    /// ms late, so `latest_prices` is close-but-not-exact. Good enough for a
    /// rolling vol estimate; the static fallback (0.12) covers any gap.
    async fn record_closing_deltas(
        &self,
        rolled: &[(v3_mm_core::Asset, WindowKind, WindowState)],
    ) {
        if rolled.is_empty() {
            return;
        }
        let prices = self.latest_prices.read().await.clone();
        let mut trackers = self.vol_trackers.write().await;
        for (asset, kind, old) in rolled {
            let Some(open) = old.opening_price else { continue };
            if open <= 0.0 {
                continue;
            }
            let Some(&latest) = prices.get(asset) else { continue };
            if latest <= 0.0 {
                continue;
            }
            let closing_delta_pct = (latest - open) / open * 100.0;
            if let Some(t) = trackers.get_mut(&(*asset, *kind)) {
                t.record_window_delta(closing_delta_pct);
                info!(
                    "↻ {:?} {:?} closed {:.4}% (opened={:.2} closed_at≈{:.2}); vol_buffer={}/{}",
                    asset,
                    kind,
                    closing_delta_pct,
                    open,
                    latest,
                    t.len(),
                    t.capacity()
                );
            }
        }
    }

    /// Anchor the opening price for any (asset, window) that doesn't have one yet.
    /// Called every tick (cheap no-op when already anchored).
    async fn handle_rollovers_and_openings(&self) {
        let prices = self.latest_prices.read().await.clone();
        if prices.is_empty() {
            return;
        }
        for &asset in &self.gengar_cfg.tracked_assets {
            let Some(&latest) = prices.get(&asset) else { continue };
            if latest <= 0.0 {
                continue;
            }
            for kind in WindowKind::all() {
                let Some(ws) = self.window_manager.get(asset, kind).await else {
                    continue;
                };
                // Track window_ts transitions purely for info-level logging.
                let mut last_ts_map = self.last_window_ts.write().await;
                let prev = last_ts_map.get(&(asset, kind)).copied();
                if let Some(prev_ts) = prev {
                    if prev_ts != ws.window_ts {
                        info!(
                            "↻ Window {:?} {:?} rolled: {} → {}",
                            asset, kind, prev_ts, ws.window_ts
                        );
                    }
                }
                last_ts_map.insert((asset, kind), ws.window_ts);
                drop(last_ts_map);

                if ws.opening_price.is_none() {
                    self.window_manager.set_opening_price(asset, kind, latest).await;
                    info!("⚓ Anchored opening price {:?} {:?} = {:.2}", asset, kind, latest);
                }
            }
        }
    }

    async fn tick_window(&self, asset: v3_mm_core::Asset, kind: WindowKind) -> Result<()> {
        // Skip if we already hold a position in this (asset, window).
        let has_pos = self
            .positions
            .read()
            .await
            .values()
            .any(|p| p.asset == asset && p.window_kind == kind);
        if has_pos {
            return Ok(());
        }

        let Some(ws) = self.window_manager.get(asset, kind).await else {
            return Ok(());
        };
        let Some(opening) = ws.opening_price else {
            return Ok(());
        };
        let binance_price = match self.latest_prices.read().await.get(&asset) {
            Some(&p) if p > 0.0 => p,
            _ => return Ok(()),
        };
        let now = chrono::Utc::now().timestamp();
        let sec_rem = kind.seconds_remaining(ws.window_ts, now) as f64;
        if sec_rem <= 0.0 {
            return Ok(());
        }

        let (up_ask, down_ask) = {
            let book = self.book_cache.read().await;
            let up = book.get_top(&ws.token_id_up).map(|(_, a)| a);
            let down = book.get_top(&ws.token_id_down).map(|(_, a)| a);
            (up, down)
        };
        // Without both books we can't evaluate.
        let (Some(up_ask), Some(down_ask)) = (up_ask, down_ask) else {
            return Ok(());
        };

        let vol = {
            let trackers = self.vol_trackers.read().await;
            trackers
                .get(&(asset, kind))
                .map(|t| t.realized_vol(self.gengar_cfg.static_vol_fallback))
                .unwrap_or(self.gengar_cfg.static_vol_fallback)
        };

        let bankroll = *self.bankroll_usdc.read().await;

        let delta_pct_preview = (binance_price - opening) / opening * 100.0;
        let Some(sig) = gengar::evaluate(
            binance_price,
            opening,
            up_ask,
            down_ask,
            sec_rem,
            bankroll,
            &self.gengar_cfg,
            vol,
        ) else {
            // Log skip reason to signals.csv so we can post-hoc analyze which
            // gate rejected. Cheap to derive here.
            let skip_reason = classify_skip(
                &self.gengar_cfg,
                delta_pct_preview,
                sec_rem,
                up_ask,
                down_ask,
            );
            self.event_log.log_signal(
                kind,
                binance_price,
                opening,
                delta_pct_preview,
                sec_rem,
                vol,
                up_ask,
                down_ask,
                "skip",
                None,
                None,
                None,
                Some(skip_reason),
            );
            return Ok(());
        };

        let token = match sig.side {
            gengar::Side::Up => &ws.token_id_up,
            gengar::Side::Down => &ws.token_id_down,
        };

        // Convert Kelly USDC → shares at the chosen side's ask.
        let shares = (sig.kelly_size / sig.market_price).floor() as u64;
        if shares < 5 {
            info!(
                "Gengar skip: shares {} < 5 (kelly=${:.2} @ {:.4})",
                shares, sig.kelly_size, sig.market_price
            );
            self.event_log.log_signal(
                kind,
                binance_price,
                opening,
                sig.btc_delta_pct,
                sec_rem,
                vol,
                up_ask,
                down_ask,
                "skip",
                Some(sig.true_prob),
                Some(sig.edge),
                Some(sig.kelly_size),
                Some("shares_below_min"),
            );
            return Ok(());
        }

        let order_id = {
            let book_top = self.book_cache.read().await.get_top(token);
            self.executor
                .execute_gengar_order(token, sig.market_price, shares, book_top)
                .await?
        };

        // Take the last 8 chars of order_id so we get uuid-like entropy even
        // when the id has a static prefix ("dry-run-gengar-<uuid>" in dry_run).
        // Without this, every pair_id collapses to "M5-dry-run-" and state
        // recovery can't tell entries apart.
        let oid_tail = &order_id[order_id.len().saturating_sub(8)..];
        let pair_id = format!("{:?}-{:?}-{}", asset, kind, oid_tail);
        let pos = OpenPosition {
            pair_id: pair_id.clone(),
            asset,
            window_kind: kind,
            side: sig.side,
            token_id: token.clone(),
            entry_price: sig.market_price,
            shares,
            initial_edge: sig.edge,
            order_id: order_id.clone(),
            entered_at: Utc::now(),
        };
        info!(
            "✅ Gengar entry {:?} {:?} @ {:.4} x {} shares, edge={:.4} vol={:.3} sec_rem={:.0}",
            kind, sig.side, sig.market_price, shares, sig.edge, vol, sec_rem
        );
        self.event_log.log_signal(
            kind,
            binance_price,
            opening,
            sig.btc_delta_pct,
            sec_rem,
            vol,
            up_ask,
            down_ask,
            "enter",
            Some(sig.true_prob),
            Some(sig.edge),
            Some(sig.kelly_size),
            None,
        );
        self.event_log.log_entry(
            &pair_id,
            &format!("{:?}", asset),
            kind,
            match sig.side {
                gengar::Side::Up => "Up",
                gengar::Side::Down => "Down",
            },
            token,
            sig.market_price,
            shares,
            sig.edge,
            &order_id,
        );
        self.hourly_stats.record_entry(sig.edge);
        self.notifier
            .notify(&format!(
                "✅ ENTRY {:?} {:?}\nprice=${:.4} x {} = ${:.2}\nedge={:.4} prob={:.3}\nvol={:.3} sec_rem={:.0}",
                kind,
                sig.side,
                sig.market_price,
                shares,
                sig.market_price * shares as f64,
                sig.edge,
                sig.true_prob,
                vol,
                sec_rem,
            ))
            .await;
        self.positions.write().await.insert(pair_id, pos);
        Ok(())
    }

    async fn manage_positions(&self) -> Result<()> {
        let positions: Vec<OpenPosition> =
            self.positions.read().await.values().cloned().collect();

        for pos in positions {
            // --- Wall-clock hard ceiling ---
            let held_secs = (chrono::Utc::now() - pos.entered_at).num_seconds().max(0) as u64;
            if gengar::should_force_close_by_wallclock(&self.gengar_cfg, held_secs) {
                warn!(
                    "⏱️ Wall-clock max_hold exceeded for {}: held {}s >= limit {}s — force-sell",
                    pos.pair_id, held_secs, self.gengar_cfg.max_position_hold_secs
                );
                self.force_sell_position(&pos).await;
                continue;
            }

            let Some(ws) = self.window_manager.get(pos.asset, pos.window_kind).await else {
                // window gone → release
                info!("🏁 Window gone, release {}", pos.pair_id);
                self.positions.write().await.remove(&pos.pair_id);
                continue;
            };
            let now = chrono::Utc::now().timestamp();
            let sec_rem = pos.window_kind.seconds_remaining(ws.window_ts, now) as f64;
            if sec_rem <= 0.0 {
                info!("🏁 Window expired, release {}", pos.pair_id);
                self.positions.write().await.remove(&pos.pair_id);
                continue;
            }

            let binance_price = match self.latest_prices.read().await.get(&pos.asset) {
                Some(&p) if p > 0.0 => p,
                _ => continue,
            };
            let Some(opening) = ws.opening_price else {
                continue;
            };
            if binance_price <= 0.0 || opening <= 0.0 {
                continue;
            }
            let delta_pct = (binance_price - opening) / opening * 100.0;
            let vol = {
                let trackers = self.vol_trackers.read().await;
                trackers
                    .get(&(pos.asset, pos.window_kind))
                    .map(|t| t.realized_vol(self.gengar_cfg.static_vol_fallback))
                    .unwrap_or(self.gengar_cfg.static_vol_fallback)
            };
            // Compute current edge using *our side's* market ask.
            let current_ask = {
                let book = self.book_cache.read().await;
                book.get_top(&pos.token_id).map(|(_, a)| a)
            };
            let Some(current_market_price) = current_ask else {
                continue;
            };
            let current_prob =
                gengar::estimate_true_probability(delta_pct, sec_rem, vol);
            // prob is the absolute probability of touching; but for a position
            // on DOWN the effective prob is (1 - p_up). estimate_true_probability
            // already uses |delta_pct|, so `prob` is the prob of whatever
            // direction delta is pointing. If our side matches the sign → prob.
            // Otherwise → 1 - prob.
            let side_matches = (delta_pct > 0.0) == matches!(pos.side, gengar::Side::Up);
            let effective_prob = if side_matches {
                current_prob
            } else {
                1.0 - current_prob
            };
            let current_edge = effective_prob - current_market_price;

            // --- Preemptive cancel (Binance adverse move) ---
            //
            // Fires when the model's probability has collapsed — usually
            // because BTC has moved strongly against us. Bails faster than
            // profit-lock, which needs edge to decay proportionally.
            if gengar::should_preemptive_cancel(&self.gengar_cfg, effective_prob) {
                self.sell_with_log(&pos, current_edge, "preemptive-cancel").await;
                continue;
            }

            if gengar::should_lock_profit(
                pos.initial_edge,
                current_edge,
                self.gengar_cfg.profit_lock_ratio,
            ) {
                self.sell_with_log(&pos, current_edge, "profit-lock").await;
                continue;
            }
        }

        Ok(())
    }

    /// Shared sell path used by profit-lock and preemptive-cancel exits.
    /// Quotes best_bid - 1 tick (or 90% of entry as last resort), submits
    /// via the executor, logs both to event_log and Telegram, and drops the
    /// position from the map.
    async fn sell_with_log(
        &self,
        pos: &OpenPosition,
        current_edge: f64,
        reason: &'static str,
    ) {
        let book_top = {
            let book = self.book_cache.read().await;
            book.get_top(&pos.token_id)
        };
        let sell_price = match book_top {
            Some((b, _)) => ((b - 0.01).max(0.01) * 100.0).round() / 100.0,
            None => (pos.entry_price * 0.90 * 100.0).round() / 100.0,
        };
        match self
            .executor
            .execute_gengar_sell(&pos.token_id, sell_price, pos.shares, book_top)
            .await
        {
            Ok(order_id) => {
                let pnl = (sell_price - pos.entry_price) * pos.shares as f64;
                info!(
                    "🚪 Exit ({}) {} @ {:.4}: initial_edge={:.4} current={:.4} pnl={:+.2}",
                    reason, pos.pair_id, sell_price, pos.initial_edge, current_edge, pnl
                );
                self.event_log.log_exit(
                    &pos.pair_id,
                    &format!("{:?}", pos.asset),
                    pos.window_kind,
                    match pos.side {
                        gengar::Side::Up => "Up",
                        gengar::Side::Down => "Down",
                    },
                    &pos.token_id,
                    sell_price,
                    pos.shares,
                    pos.initial_edge,
                    Some(current_edge),
                    &order_id,
                    Some(pnl),
                    reason,
                );
                self.hourly_stats.record_exit(reason, pnl);
                let emoji = match reason {
                    "profit-lock" => "🔒",
                    "preemptive-cancel" => "⚡",
                    _ => "🚪",
                };
                self.notifier
                    .notify(&format!(
                        "{} EXIT {} {:?}\npnl=${:+.2} (entry {:.4} → {:.4})\ninitial_edge={:.4} current={:.4}",
                        emoji,
                        reason,
                        pos.window_kind,
                        pnl,
                        pos.entry_price,
                        sell_price,
                        pos.initial_edge,
                        current_edge,
                    ))
                    .await;
                self.positions.write().await.remove(&pos.pair_id);
            }
            Err(e) => warn!("sell ({}) failed for {}: {:#}", reason, pos.pair_id, e),
        }
    }

    /// Best-effort close: try to sell at best_bid - 1 tick, else at 90% of
    /// entry price as a last-ditch price floor. If the sell itself errors we
    /// still drop the position from our map and log so we don't loop forever
    /// on a dead market.
    async fn force_sell_position(&self, pos: &OpenPosition) {
        let book_top = {
            let book = self.book_cache.read().await;
            book.get_top(&pos.token_id)
        };
        let sell_price = match book_top {
            Some((b, _)) => ((b - 0.01).max(0.01) * 100.0).round() / 100.0,
            None => (pos.entry_price * 0.90 * 100.0).round() / 100.0,
        };
        match self
            .executor
            .execute_gengar_sell(&pos.token_id, sell_price, pos.shares, book_top)
            .await
        {
            Ok(order_id) => {
                info!(
                    "💨 Force-sell {} @ {:.4} → order={}",
                    pos.pair_id, sell_price, order_id
                );
                let pnl = (sell_price - pos.entry_price) * pos.shares as f64;
                self.event_log.log_exit(
                    &pos.pair_id,
                    &format!("{:?}", pos.asset),
                    pos.window_kind,
                    match pos.side {
                        gengar::Side::Up => "Up",
                        gengar::Side::Down => "Down",
                    },
                    &pos.token_id,
                    sell_price,
                    pos.shares,
                    pos.initial_edge,
                    None,
                    &order_id,
                    Some(pnl),
                    "force-sell",
                );
                self.hourly_stats.record_exit("force-sell", pnl);
                self.notifier
                    .notify(&format!(
                        "⏱️ FORCE-SELL {:?}\npnl=${:+.2} (entry {:.4} → {:.4})\nreason=wall-clock max_hold",
                        pos.window_kind, pnl, pos.entry_price, sell_price
                    ))
                    .await;
            }
            Err(e) => {
                warn!(
                    "💥 Force-sell FAILED for {} @ {:.4}: {:#} (dropping from position map anyway)",
                    pos.pair_id, sell_price, e
                );
                self.notifier
                    .notify(&format!(
                        "💥 FORCE-SELL FAILED {:?}\n{:?} entry ${:.4} x {} — CHECK WALLET",
                        pos.window_kind, pos.side, pos.entry_price, pos.shares
                    ))
                    .await;
            }
        }
        self.positions.write().await.remove(&pos.pair_id);
    }
}

/// Classify why `gengar::evaluate` returned None. Mirrors its gate order;
/// cheap string return for CSV logging.
fn classify_skip(
    cfg: &GengarConfig,
    delta_pct: f64,
    sec_rem: f64,
    up_ask: f64,
    down_ask: f64,
) -> &'static str {
    if sec_rem > cfg.entry_window_start_secs as f64 {
        return "outside_window_early";
    }
    if sec_rem < cfg.entry_window_end_secs as f64 {
        return "outside_window_late";
    }
    if delta_pct.abs() < cfg.min_btc_delta_pct {
        return "delta_too_small";
    }
    let side_price = if delta_pct > 0.0 { up_ask } else { down_ask };
    if side_price > cfg.max_price || side_price < cfg.min_price {
        return "price_out_of_range";
    }
    // If we got here, either prob < min_prob or edge < min_edge.
    // (Can't easily distinguish without recomputing the Brownian prob here,
    // and either way the post-hoc analyzer can reconstruct prob from
    // delta_pct/sec_rem/vol.)
    "prob_or_edge_too_low"
}
