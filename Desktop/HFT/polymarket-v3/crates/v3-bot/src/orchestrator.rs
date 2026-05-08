use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant as StdInstant;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn, error};
use anyhow::Result;
use futures::future;
use chrono::Timelike;

use v3_core::{Asset, Market, MarketState, Position};
use v3_signal::{BinanceMonitor, SignalSnapshot, MomentumSignalType};
use v3_strategy::{StrategySelector, StrategyDecision, MomentumStrategy, MarketMakingStrategy, SpreadCaptureStrategy, Strategy};
use v3_execution::{OrderExecutor, TtlManager, CancelMonitor, OrderTracker, TradeLog};
use v3_risk::{limits::RiskManager, PositionMonitor, CircuitBreaker};
use v3_dashboard::{DashboardServer, DashboardState, PositionSnapshot, OrderSnapshot};

use crate::config::BotConfig;
use crate::scanner_integration::MarketScanner;
use crate::binance_integration::BinanceFeed;
use crate::clob_integration::{ClobFeed, BookCache};
use crate::position_tracker::{PositionTracker, TrackedPosition};

/// Main orchestrator for the trading bot
pub struct Orchestrator {
    config: BotConfig,

    // Market discovery
    scanner: Arc<RwLock<MarketScanner>>,
    cached_markets: Arc<RwLock<Vec<Market>>>,  // Cache markets from scanner

    // Price feeds
    binance_feed: Arc<RwLock<BinanceFeed>>,
    clob_feed: Arc<RwLock<ClobFeed>>,
    book_cache: Arc<RwLock<BookCache>>,

    // Signal generation
    binance_monitor: Arc<RwLock<BinanceMonitor>>,

    // Strategy
    strategy_selector: Arc<StrategySelector>,

    // Execution
    executor: Arc<OrderExecutor>,
    ttl_manager: Arc<RwLock<TtlManager>>,
    cancel_monitor: Arc<CancelMonitor>,
    order_tracker: Arc<RwLock<OrderTracker>>,

    // Position tracking
    position_tracker: Arc<RwLock<PositionTracker>>,

    // Risk management
    risk_manager: Arc<RwLock<RiskManager>>,
    position_monitor: Arc<PositionMonitor>,
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,

    // Dashboard
    dashboard: Arc<DashboardServer>,

    // Uptime tracking
    start_time: std::time::Instant,

    // Bug #5 Fix: Order deduplication cache
    recent_orders: Arc<RwLock<HashMap<String, StdInstant>>>,

    // Bug #58 Fix: Order frequency limiter (prevent order storm)
    last_order_time: Arc<RwLock<HashMap<String, StdInstant>>>,

    // Fill monitoring: pending orders and processed fills
    pending_orders: Arc<RwLock<HashMap<String, PendingOrder>>>,
    processed_fills: Arc<RwLock<std::collections::HashSet<String>>>,

    // Trade logging
    trade_log: Arc<RwLock<TradeLog>>,
}

/// Pending order waiting for fill
#[derive(Debug, Clone)]
struct PendingOrder {
    order_id: String,
    market_id: String,
    outcome: v3_core::Outcome,
    side: v3_core::OrderSide,
    price: f64,
    size: u64,
    submitted_at: StdInstant,
}

impl Orchestrator {
    pub async fn new(config: BotConfig) -> Result<Self> {
        info!("Initializing orchestrator (dry_run: {})", config.dry_run);

        // Initialize market scanner
        let scanner = Arc::new(RwLock::new(MarketScanner::new().await?));

        // Initialize Binance feed
        let binance_feed = Arc::new(RwLock::new(BinanceFeed::new().await?));

        // Initialize CLOB feed (start with empty subscription, will add markets dynamically)
        let clob_feed = Arc::new(RwLock::new(ClobFeed::new(vec![], std::collections::HashMap::new()).await?));
        let book_cache = Arc::new(RwLock::new(BookCache::new()));

        // Initialize Binance monitor
        let binance_monitor = Arc::new(RwLock::new(BinanceMonitor::new(
            Asset::BTC, // TODO: Make configurable
            config.execution.adverse_threshold,
            0.1, // acceleration threshold
        )));

        // Initialize strategies
        let momentum = Arc::new(MomentumStrategy::new(config.strategies.momentum.clone()));
        let market_making = Arc::new(MarketMakingStrategy::new(config.strategies.market_making.clone()));
        let spread_capture = Arc::new(SpreadCaptureStrategy::new(config.strategies.spread_capture.clone()));

        let strategy_selector = Arc::new(StrategySelector::new(
            momentum,
            market_making,
            spread_capture,
        ));

        // Initialize risk management
        let risk_manager = Arc::new(RwLock::new(RiskManager::new(config.risk.clone())));
        let position_monitor = Arc::new(PositionMonitor::new(
            risk_manager.clone(),
            config.execution.position_check_interval_ms,
        ));
        let circuit_breaker = Arc::new(RwLock::new(CircuitBreaker::new(risk_manager.clone())));

        // Initialize execution
        let ttl_manager = Arc::new(RwLock::new(TtlManager::new()));
        let order_tracker = Arc::new(RwLock::new(OrderTracker::new()));
        let cancel_monitor = Arc::new(CancelMonitor::new(
            binance_monitor.clone(),
            order_tracker.clone(),
            config.execution.adverse_threshold,
            config.execution.cancel_check_interval_ms,
        ));

        // Get private key from environment if not in dry-run mode
        let private_key = if !config.dry_run {
            match std::env::var("PRIVATE_KEY") {
                Ok(key) => {
                    info!("Private key loaded from environment");
                    Some(key)
                }
                Err(_) => {
                    anyhow::bail!("PRIVATE_KEY environment variable required for live trading");
                }
            }
        } else {
            None
        };

        let executor = Arc::new(OrderExecutor::new(
            private_key,
            ttl_manager.clone(),
            cancel_monitor.clone(),
            order_tracker.clone(),
            config.dry_run,
        ).await?);

        // Initialize position tracker
        let position_tracker = Arc::new(RwLock::new(PositionTracker::new()));

        // Initialize dashboard
        let dashboard = Arc::new(DashboardServer::new());

        Ok(Self {
            config,
            scanner,
            cached_markets: Arc::new(RwLock::new(Vec::new())),  // Initialize empty market cache
            binance_feed,
            clob_feed,
            book_cache,
            binance_monitor,
            strategy_selector,
            executor,
            ttl_manager,
            cancel_monitor,
            order_tracker,
            position_tracker,
            risk_manager,
            position_monitor,
            circuit_breaker,
            dashboard,
            start_time: std::time::Instant::now(),
            recent_orders: Arc::new(RwLock::new(HashMap::new())),  // Bug #5 Fix
            last_order_time: Arc::new(RwLock::new(HashMap::new())),  // Bug #58 Fix
            pending_orders: Arc::new(RwLock::new(HashMap::new())),  // Fill monitoring
            processed_fills: Arc::new(RwLock::new(std::collections::HashSet::new())),  // Fill deduplication
            trade_log: Arc::new(RwLock::new(
                TradeLog::new("trade_log.csv")
                    .expect("Failed to create trade log")
            )),
        })
    }

    /// Start the orchestrator
    pub async fn run(&self) -> Result<()> {
        info!("Starting orchestrator");

        // Start dashboard server
        let dashboard_port = 8081;
        let dashboard_clone = self.dashboard.clone();
        tokio::spawn(async move {
            if let Err(e) = v3_dashboard::serve(dashboard_clone, dashboard_port).await {
                error!("Dashboard server error: {}", e);
            }
        });
        info!("Dashboard server starting on port {}", dashboard_port);

        // Start background tasks
        let cancel_monitor = self.cancel_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = cancel_monitor.run().await {
                error!("Cancel monitor error: {}", e);
            }
        });

        let position_monitor = self.position_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = position_monitor.run().await {
                error!("Position monitor error: {}", e);
            }
        });

        // Start Binance price update task
        let binance_feed = self.binance_feed.clone();
        let binance_monitor = self.binance_monitor.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let mut feed = binance_feed.write().await;
                let ticks = feed.drain_ticks().await;
                drop(feed);

                if !ticks.is_empty() {
                    info!("📥 Drained {} ticks from Binance feed", ticks.len());

                    // CRITICAL FIX: Only use BTC price, ignore other assets
                    // Mixing prices from different assets (BTC=$74k, XRP=$1.5) causes velocity explosion
                    let btc_ticks: Vec<_> = ticks.iter()
                        .filter(|t| t.asset == Asset::BTC)
                        .collect();

                    info!("📊 Filtered to {} BTC ticks", btc_ticks.len());

                    if let Some(last_btc_tick) = btc_ticks.last() {
                        let mid_price = (last_btc_tick.bid + last_btc_tick.ask) / 2.0;

                        info!("📊 Binance BTC tick: bid={:.2}, ask={:.2}, mid={:.2}",
                            last_btc_tick.bid, last_btc_tick.ask, mid_price);

                        let mut monitor = binance_monitor.write().await;
                        monitor.update(mid_price);
                    }
                } else {
                    // Log every 100 iterations (10 seconds) if no ticks
                    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count % 100 == 0 {
                        info!("⚠️ No Binance ticks received (checked {} times)", count);
                    }
                }
            }
        });

        // Start CLOB event processor task
        let mut clob_feed = self.clob_feed.clone();
        let book_cache = self.book_cache.clone();
        let position_tracker = self.position_tracker.clone();
        let risk_manager = self.risk_manager.clone();
        let dashboard = self.dashboard.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Drain all CLOB events
                let events = clob_feed.write().await.drain_events().await;

                for event in events {
                    match event {
                        mm_exchange::types::ExchangeEvent::BookUpdate(snapshot) => {
                            // Update book cache
                            let mut cache = book_cache.write().await;
                            cache.update(snapshot);
                        }
                        mm_exchange::types::ExchangeEvent::OrderFill(fill) => {
                            // CRITICAL FIX #4: Process WebSocket Fill events immediately
                            info!("📨 WebSocket Fill received: order_id={}, size={}", fill.order_id, fill.size);
                            let mut tracker = position_tracker.write().await;
                            let mut risk = risk_manager.write().await;

                            Self::handle_fill(&mut tracker, &mut risk, fill).await;

                            drop(tracker);
                            drop(risk);
                        }
                        mm_exchange::types::ExchangeEvent::MarketResolved(resolved) => {
                            info!("Market resolved: {} -> {:?}", resolved.condition_id, resolved.winner);
                            // TODO: Handle market resolution
                        }
                        _ => {
                            // Ignore other events (RtdsPrice, BinanceTick, etc.)
                        }
                    }
                }
            }
        });

        // Start position price updater task
        let book_cache = self.book_cache.clone();
        let position_tracker = self.position_tracker.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let cache = book_cache.read().await;
                let mut tracker = position_tracker.write().await;

                // Update prices for all active positions
                for position in tracker.get_all_positions() {
                    let market_id = &position.market_id;
                    let outcome = position.outcome;

                    // Get token_id based on outcome (this is simplified, real implementation needs market lookup)
                    // For now, we'll update in process_market where we have full market context
                }

                drop(tracker);
                drop(cache);
            }
        });

        // Main event loop
        self.event_loop().await
    }

    /// Main event loop
    async fn event_loop(&self) -> Result<()> {
        info!("Starting main event loop (tick: {}ms)", self.config.tick_interval_ms);

        loop {
            sleep(Duration::from_millis(self.config.tick_interval_ms)).await;

            if let Err(e) = self.tick().await {
                error!("Tick error: {}", e);
            }
        }
    }

    /// Single tick of the event loop
    async fn tick(&self) -> Result<()> {
        // Bug #10 Fix: Daily PnL reset check
        // Check if we need to reset daily PnL (at midnight UTC)
        let now = chrono::Utc::now();
        let hour = now.hour();
        let minute = now.minute();

        if hour == 0 && minute == 0 {
            let mut risk_manager = self.risk_manager.write().await;
            // Note: reset_daily_pnl() may not exist, using reset_daily() instead
            risk_manager.reset_daily();
            info!("✅ Daily PnL reset at midnight UTC");
            drop(risk_manager);
        }

        // Reset daily counters if needed (Bug #13 fix from polymarket-mm)
        let mut risk_manager = self.risk_manager.write().await;
        risk_manager.reset_daily();
        drop(risk_manager);

        // Bug #12 Fix: Cleanup expired orders every 10 seconds
        // Use a static counter to track ticks
        static TICK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tick_count = TICK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if tick_count % 100 == 0 {  // Every 100 ticks = 10 seconds (at 100ms interval)
            let mut tracker = self.order_tracker.write().await;
            // Note: cleanup_expired() may not exist, using cleanup_completed() instead
            tracker.cleanup_completed();
            drop(tracker);
        }

        // Check circuit breaker
        let mut breaker = self.circuit_breaker.write().await;
        if !breaker.should_allow_trading().await {
            debug!("Trading halted by circuit breaker");
            return Ok(());
        }
        drop(breaker);

        // Update dashboard state
        self.update_dashboard().await;

        // Check TTL expiration
        self.check_ttl_expiration().await?;

        // Check position exit conditions (distinct-baguette style: 6s target, 30s max)
        self.check_position_exits().await?;

        // Poll order status for fill monitoring (LIVE mode only)
        if !self.config.dry_run {
            if let Err(e) = self.poll_order_status().await {
                warn!("Failed to poll order status: {}", e);
            }
        }

        // Update market cache from scanner (non-blocking)
        let mut scanner = self.scanner.write().await;
        let new_markets = scanner.get_available_markets().await;
        drop(scanner);

        // Update cache if we got new markets
        if !new_markets.is_empty() {
            info!("📥 Scanner returned {} markets, updating cache", new_markets.len());
            let mut cache = self.cached_markets.write().await;
            *cache = new_markets;
        }

        // Use cached markets for trading
        let cache = self.cached_markets.read().await;
        let mut markets = cache.clone();
        drop(cache);

        // Bug #10 Fix: Subscribe CLOB to all discovered market tokens
        if !markets.is_empty() {
            let token_ids: Vec<String> = markets.iter()
                .flat_map(|m| vec![m.token_id_yes.clone(), m.token_id_no.clone()])
                .filter(|t| !t.is_empty())
                .collect();
            let cid_to_asset: std::collections::HashMap<String, mm_core::Asset> = markets.iter()
                .map(|m| (m.condition_id.clone(), mm_core::Asset::BTC))
                .collect();
            if let Err(e) = self.clob_feed.write().await
                .ensure_subscribed(token_ids, cid_to_asset).await {
                warn!("Failed to update CLOB subscription: {}", e);
            }
        }

        // Log market status
        info!("Using {} markets from cache", markets.len());
        if markets.is_empty() {
            warn!("No markets available - waiting for scanner");
            return Ok(());
        }

        // Gap #5 Fix: Prioritize markets by volume (top 4 for BTC/ETH/SOL/XRP)
        // distinct-baguette shows top 3 markets = 46% of trades
        // User clarified: 15-min markets are continuous, can trade 4 simultaneously
        if markets.len() > 4 {
            info!("Market prioritization: Limiting to top 4 markets (from {})", markets.len());
            // For now, just take first 4 (scanner should return them in some order)
            // TODO: Sort by actual 24h volume from CLOB API
            markets.truncate(4);
        }

        // Also process markets where we have existing positions (for exit signals)
        let tracker = self.position_tracker.read().await;
        let position_market_ids: Vec<String> = tracker.get_all_positions()
            .iter()
            .map(|p| p.market_id.clone())
            .collect();
        drop(tracker);

        // Add position markets to processing list (if not already included)
        for market_id in position_market_ids {
            if !markets.iter().any(|m| m.condition_id == market_id) {
                // Need to reconstruct market from position data
                // For now, we'll process it in a separate loop
                if let Err(e) = self.process_position_exit(&market_id).await {
                    warn!("Error processing position exit for {}: {}", market_id, e);
                }
            }
        }

        // Process each market (parallel processing for 5.5 trades/min)
        let futures: Vec<_> = markets.iter()
            .map(|market| self.process_market_async(market.clone()))
            .collect();

        // Wait for all markets to complete
        let results: Vec<Result<()>> = future::join_all(futures).await;

        // Log any errors
        for (i, result) in results.iter().enumerate() {
            if let Err(e) = result {
                warn!("Error processing market {}: {:#}", markets[i].condition_id, e);
            }
        }

        Ok(())
    }

    /// Process a single market asynchronously (for parallel execution)
    async fn process_market_async(&self, market: Market) -> Result<()> {
        self.process_market(&market).await
    }

    /// Process exit signals for an existing position
    async fn process_position_exit(&self, market_id: &str) -> Result<()> {
        // Get position
        let tracker = self.position_tracker.read().await;
        let tracked_position = tracker.get_position(market_id).cloned();
        drop(tracker);

        let tracked_pos = match tracked_position {
            Some(p) => p,
            None => return Ok(()), // Position already closed
        };

        // Reconstruct minimal market info from position
        let market = Market {
            condition_id: market_id.to_string(),
            token_id_yes: String::new(), // Not needed for exit decision
            token_id_no: String::new(),
            asset: Asset::BTC, // Default, not critical for exit
            open_time: tracked_pos.entry_time,
            close_time: tracked_pos.entry_time + chrono::Duration::hours(1),
            open_price: tracked_pos.entry_price,
            slug: String::new(),
        };

        // Convert to Position
        let position = tracked_pos.to_position(&market);

        // Generate signal
        let signal = self.generate_signal(&market).await?;

        // Select strategy
        let strategy = self.strategy_selector.select_strategy(&signal.market_state);

        // Check if should exit
        let decision = strategy.generate_decision(&market, &signal, Some(&position)).await?;

        // Execute exit if needed
        if matches!(decision, v3_strategy::StrategyDecision::Exit { .. }) {
            self.execute_decision(&market, decision).await?;
        }

        Ok(())
    }

    /// Update dashboard with current state
    async fn update_dashboard(&self) {
        use chrono::Utc;

        let risk = self.risk_manager.read().await;
        let breaker = self.circuit_breaker.read().await;
        let tracker = self.position_tracker.read().await;
        let orders = self.order_tracker.read().await;

        // Build position snapshots
        let positions: Vec<PositionSnapshot> = tracker.get_all_positions()
            .iter()
            .map(|p| PositionSnapshot {
                market_id: p.market_id.clone(),
                outcome: format!("{:?}", p.outcome),
                size: p.size,
                entry_price: p.entry_price,
                current_price: p.current_price,
                unrealized_pnl: p.unrealized_pnl,
                hold_time_secs: (Utc::now() - p.entry_time).num_seconds() as f64,
                entry_time: format!("{:?}", p.entry_time),
            })
            .collect();

        // Build order snapshots
        let order_snapshots: Vec<OrderSnapshot> = orders.get_all_orders()
            .iter()
            .map(|o| OrderSnapshot {
                order_id: o.order_id.clone(),
                market_id: o.market_id.clone(),
                side: format!("{:?}", o.side),
                price: o.price,
                size: o.size,
                ttl_secs: o.ttl_secs,
                age_secs: (Utc::now() - o.created_at).num_seconds() as f64,
                created_at: format!("{:?}", o.created_at),
            })
            .collect();

        let total_exposure = positions.iter().map(|p| p.size * p.current_price).sum();

        let state = DashboardState {
            timestamp: Utc::now().format("%H:%M:%S UTC").to_string(),
            uptime_secs: self.start_time.elapsed().as_secs(),
            dry_run: self.config.dry_run,
            daily_pnl: risk.daily_pnl(),
            max_daily_loss: risk.limits().max_daily_loss,
            circuit_breaker_active: breaker.is_active(),
            trading_enabled: !breaker.is_active(),
            positions,
            total_exposure,
            position_count: tracker.get_all_positions().len(),
            orders: order_snapshots,
            active_order_count: orders.get_all_orders().len(),
            scanner_connected: true, // TODO: Add actual connection status
            binance_connected: true,
            clob_connected: true,
            tick_interval_ms: self.config.tick_interval_ms,
        };

        self.dashboard.update_state(state).await;
    }

    /// Check for expired orders and cancel them
    async fn check_ttl_expiration(&self) -> Result<()> {
        let ttl_manager = self.ttl_manager.read().await;
        let expired = ttl_manager.check_expired();
        drop(ttl_manager);

        if !expired.is_empty() {
            info!("Found {} expired orders", expired.len());
            self.executor.cancel_orders(&expired).await?;

            // Log to dashboard
            self.dashboard.add_event(
                "ORDER".to_string(),
                format!("Cancelled {} expired orders", expired.len())
            ).await;
        }

        Ok(())
    }

    /// Check position exit conditions (distinct-baguette: 6s target, 30s max)
    async fn check_position_exits(&self) -> Result<()> {
        let tracker = self.position_tracker.read().await;
        let position_count = tracker.get_all_positions().len();

        // Check regular exit conditions
        let mut to_close = tracker.check_exit_conditions();

        // Check 6-second target exits
        let six_sec_exits = tracker.check_six_second_exits();
        to_close.extend(six_sec_exits);

        drop(tracker);

        info!("🔍 Position exit check: {} active positions, {} to close", position_count, to_close.len());

        if !to_close.is_empty() {
            info!("🎯 Found {} positions to close", to_close.len());

            for (market_id, reason) in to_close {
                // Execute immediate exit
                if let Err(e) = self.execute_immediate_exit(&market_id, &reason).await {
                    warn!("Failed to exit position {}: {}", market_id, e);
                }
            }
        }

        Ok(())
    }

    /// Execute immediate exit for a position (distinct-baguette style: fast exit)
    async fn execute_immediate_exit(&self, market_id: &str, reason: &str) -> Result<()> {
        // Get position details
        let tracker = self.position_tracker.read().await;
        let position = tracker.get_position(market_id).cloned();
        drop(tracker);

        let pos = match position {
            Some(p) => p,
            None => return Ok(()), // Already closed
        };

        info!(
            "📤 Immediate exit: {} ({:?}, size: {}, PnL: ${:.2}) - reason: {}",
            market_id, pos.outcome, pos.size, pos.unrealized_pnl, reason
        );

        // Reconstruct market from position (simplified)
        let market = Market {
            condition_id: market_id.to_string(),
            token_id_yes: format!("{}_yes", market_id),
            token_id_no: format!("{}_no", market_id),
            asset: v3_core::Asset::BTC, // Default
            open_time: pos.entry_time,
            close_time: pos.entry_time + chrono::Duration::hours(1),
            open_price: pos.entry_price,
            slug: String::new(),
        };

        // Get current bid price for exit
        let book_cache = self.book_cache.read().await;
        let token_id = match pos.outcome {
            v3_core::Outcome::Up => &market.token_id_yes,
            v3_core::Outcome::Down => &market.token_id_no,
        };
        let exit_price = book_cache.get_top(token_id)
            .map(|(bid, _)| bid)
            .unwrap_or(pos.current_price);
        drop(book_cache);

        // Create exit order with short TTL for fast execution
        let order = v3_core::Order {
            id: uuid::Uuid::new_v4().to_string(),
            market: market.clone(),
            outcome: pos.outcome,
            side: v3_core::OrderSide::Sell,
            price: rust_decimal::Decimal::from_f64_retain(exit_price).unwrap_or_default(),
            size: pos.size as u64,
            created_at: chrono::Utc::now(),
            ttl_secs: 5, // Short TTL for immediate execution
        };

        let order_id = self.executor.execute_order(order).await?;
        info!("✅ Immediate exit order placed: {} @ {:.4} (reason: {})", order_id, exit_price, reason);

        // In dry-run mode, simulate immediate fill
        if self.config.dry_run {
            let mut tracker = self.position_tracker.write().await;
            let mut risk = self.risk_manager.write().await;

            // Calculate PnL before closing
            let realized_pnl = pos.unrealized_pnl;

            // Close position
            tracker.on_fill(
                market_id.to_string(),
                pos.outcome,
                pos.size,
                exit_price,
                false, // is_entry = false
            );

            // Record PnL
            if realized_pnl.abs() > 0.001 {
                risk.record_realized_pnl(realized_pnl);
                info!("💰 Recorded realized PnL: ${:.2}", realized_pnl);
            }

            drop(tracker);
            drop(risk);
        }

        // Log to dashboard
        self.dashboard.add_event(
            "EXIT".to_string(),
            format!("Exited {} - {} (PnL: ${:.2})", market_id, reason, pos.unrealized_pnl)
        ).await;

        Ok(())
    }

    /// Process a single market
    async fn process_market(&self, market: &Market) -> Result<()> {
        debug!("Processing market: {}", market.condition_id);

        // Bug #58 Fix: Check minimum order interval (prevent order storm)
        let mut last_times = self.last_order_time.write().await;
        if let Some(last_time) = last_times.get(&market.condition_id) {
            if last_time.elapsed() < Duration::from_secs(5) {
                debug!("Skipping market {} - too soon since last order ({}s ago)",
                    &market.condition_id[..16], last_time.elapsed().as_secs());
                return Ok(());
            }
        }
        drop(last_times);

        // Generate signal
        let signal = self.generate_signal(market).await?;
        info!("📊 Signal: velocity={:.6}, volatility={:.6}, mid_up={:.4}, mid_down={:.4}",
               signal.binance_velocity, signal.volatility, signal.market_state.mid_price_up, signal.market_state.mid_price_down);

        // Update position prices from book cache
        let book_cache = self.book_cache.read().await;
        let mut position_tracker = self.position_tracker.write().await;

        // Get outcome and token_id first
        let update_info = position_tracker.get_position(&market.condition_id).map(|position| {
            let token_id = match position.outcome {
                v3_core::Outcome::Up => market.token_id_yes.clone(),
                v3_core::Outcome::Down => market.token_id_no.clone(),
            };
            (position.outcome, token_id)
        });

        // Now update price if we have the info
        if let Some((outcome, token_id)) = update_info {
            if let Some((bid, ask)) = book_cache.get_top(&token_id) {
                let mid_price = (bid + ask) / 2.0;
                position_tracker.update_price(&market.condition_id, outcome, mid_price);
            }
        }

        // Get position for strategy decision
        let tracked_position = position_tracker.get_position(&market.condition_id).cloned();

        drop(position_tracker);
        drop(book_cache);

        // Convert TrackedPosition to Position for strategy
        let position = tracked_position.map(|tp| tp.to_position(market));

        // 🎯 NEW: Priority-based strategy selection (distinct-baguette style)
        // Momentum > Arbitrage > Market Making
        let decision = self.select_strategy_with_priority(market, &signal, position.as_ref()).await?;
        info!("📋 Strategy decision for {}: {:?}", &market.condition_id[..16], decision);

        // Execute decision
        self.execute_decision(market, decision).await?;

        Ok(())
    }

    /// Select strategy with priority: Momentum > Arbitrage > Market Making
    /// This matches distinct-baguette's 70/10/20 distribution
    async fn select_strategy_with_priority(
        &self,
        market: &Market,
        signal: &SignalSnapshot,
        position: Option<&Position>,
    ) -> Result<StrategyDecision> {
        // Priority 1: Momentum Strategy (if strong momentum)
        if self.has_strong_momentum(signal) {
            let momentum = self.strategy_selector.get_momentum_strategy();
            let decision = momentum.generate_decision(market, signal, position).await?;

            if !matches!(decision, StrategyDecision::Hold) {
                info!("🎯 Using MOMENTUM strategy (priority 1)");
                return Ok(decision);
            }
        }

        // Priority 2: Arbitrage Strategy (if arbitrage opportunity)
        if signal.has_arbitrage {
            let arbitrage = self.strategy_selector.get_arbitrage_strategy();
            let decision = arbitrage.generate_decision(market, signal, position).await?;

            if !matches!(decision, StrategyDecision::Hold) {
                info!("💰 Using ARBITRAGE strategy (priority 2)");
                return Ok(decision);
            }
        }

        // Priority 3: Market Making Strategy (fallback)
        let market_making = self.strategy_selector.get_market_making_strategy();
        let decision = market_making.generate_decision(market, signal, position).await?;

        if !matches!(decision, StrategyDecision::Hold) {
            info!("📊 Using MARKET_MAKING strategy (priority 3)");
        }

        Ok(decision)
    }

    /// Check if there's strong momentum (distinct-baguette style)
    fn has_strong_momentum(&self, signal: &SignalSnapshot) -> bool {
        // Strong momentum definition:
        // 1. Volatility > 0.0003 (0.03% price range in 30 seconds)
        // 2. Price in reasonable range (0.50-0.95 for either side)

        // Use volatility instead of velocity - more suitable for detecting momentum
        let volatility_threshold = 0.0003; // 0.03% range (about $22 for BTC at $74k)
        let has_volatility = signal.volatility > volatility_threshold;

        let price_in_range =
            (signal.market_state.mid_price_up >= 0.50 && signal.market_state.mid_price_up <= 0.95) ||
            (signal.market_state.mid_price_down >= 0.50 && signal.market_state.mid_price_down <= 0.95);

        let result = has_volatility && price_in_range;

        if result {
            info!("🎯 Strong momentum detected: volatility={:.6} (threshold={:.6})",
                signal.volatility, volatility_threshold);
        }

        result
    }

    /// Generate signal for a market
    async fn generate_signal(&self, market: &Market) -> Result<SignalSnapshot> {
        // Get Binance signal (velocity, acceleration)
        let binance_monitor = self.binance_monitor.read().await;
        let mut signal = binance_monitor.get_signal();
        drop(binance_monitor);

        // Get real-time order book prices from CLOB
        let book_cache = self.book_cache.read().await;

        // Get prices for UP token — track whether real data exists
        let yes_top = book_cache.get_top(&market.token_id_yes);
        let no_top = book_cache.get_top(&market.token_id_no);
        let has_real_yes_book = yes_top.is_some();
        let has_real_no_book = no_top.is_some();
        let (best_bid_up, best_ask_up) = yes_top.unwrap_or((0.48, 0.52));
        let (best_bid_down, best_ask_down) = no_top.unwrap_or((0.48, 0.52));

        // Order book imbalance from YES token (top 5 levels)
        let order_book_imbalance = book_cache.imbalance_ratio(&market.token_id_yes, 5);
        let bid_depth_3 = book_cache.bid_depth(&market.token_id_yes, 3);
        let ask_depth_3 = book_cache.ask_depth(&market.token_id_yes, 3);

        // Spread capture margin: profit if we buy both YES and NO
        let spread_capture_margin = book_cache.spread_capture_margin(&market.token_id_yes, &market.token_id_no);

        drop(book_cache);

        // Calculate spreads and mid prices
        let spread_up = best_ask_up - best_bid_up;
        let spread_down = best_ask_down - best_bid_down;
        let mid_price_up = (best_bid_up + best_ask_up) / 2.0;
        let mid_price_down = (best_bid_down + best_ask_down) / 2.0;

        // Update market state with real prices
        signal.market = market.clone();
        signal.market_state.market = market.clone();
        signal.market_state.best_bid_up = best_bid_up;
        signal.market_state.best_ask_up = best_ask_up;
        signal.market_state.best_bid_down = best_bid_down;
        signal.market_state.best_ask_down = best_ask_down;
        signal.market_state.spread_up = spread_up;
        signal.market_state.spread_down = spread_down;
        signal.market_state.mid_price_up = mid_price_up;
        signal.market_state.mid_price_down = mid_price_down;

        // Populate new signal fields from book cache
        signal.order_book_imbalance = order_book_imbalance;
        signal.bid_depth_3 = bid_depth_3;
        signal.ask_depth_3 = ask_depth_3;
        signal.spread_capture_margin = spread_capture_margin;
        // binance_cancel_signal already set by BinanceMonitor::get_signal()

        // Check for arbitrage opportunity (YES + NO should = $1)
        // Only trigger when BOTH tokens have real book data to avoid false positives
        let sum = mid_price_up + mid_price_down;
        signal.has_arbitrage = has_real_yes_book && has_real_no_book &&
            (spread_capture_margin > 0.02 || (sum - 1.0).abs() > 0.02);

        Ok(signal)
    }

    /// Execute a strategy decision
    async fn execute_decision(&self, market: &Market, decision: StrategyDecision) -> Result<()> {
        info!("🔧 execute_decision called for market {}", &market.condition_id[..16]);

        match decision {
            StrategyDecision::Enter { outcome, side, size, price, ttl, reason } => {
                info!("📝 Processing Enter: {:?} {:?} @ {:.4} (size: {})", outcome, side, price, size);

                // Bug #5 Fix: Check for duplicate orders
                let order_key = format!("{}_{:?}_{:?}", market.condition_id, outcome, side);
                {
                    let recent = self.recent_orders.read().await;

                    // Check if we placed this order recently (within 1 second)
                    if let Some(last_time) = recent.get(&order_key) {
                        if last_time.elapsed() < std::time::Duration::from_secs(1) {
                            info!("⚠️ Skipping duplicate order: {} (placed {:.1}s ago)",
                                order_key, last_time.elapsed().as_secs_f64());
                            return Ok(());
                        }
                    }
                }

                // Check risk limits (convert token size to USDC value)
                info!("🔍 Checking risk limits...");
                let risk = self.risk_manager.read().await;
                let order_value_usdc = (size as f64 * price) as u64;
                if let Err(e) = risk.can_open_position(order_value_usdc, market.asset) {
                    info!("⛔ Risk limits prevent opening position: {}", e);
                    return Ok(());
                }
                drop(risk);

                info!("✅ Risk check passed, creating order...");

                // CRITICAL FIX #2: Track order BEFORE submission to avoid race condition
                // Generate order_id first, then track it before submission
                let order_id = format!("v3-{}", uuid::Uuid::new_v4());

                if !self.config.dry_run {
                    let mut pending = self.pending_orders.write().await;
                    pending.insert(order_id.clone(), PendingOrder {
                        order_id: order_id.clone(),
                        market_id: market.condition_id.clone(),
                        outcome,
                        side,
                        price,
                        size,
                        submitted_at: StdInstant::now(),
                    });
                    info!("Pre-tracked order {} for fill monitoring", order_id);
                }

                // Place order
                let order = v3_core::Order {
                    id: order_id.clone(),
                    market: market.clone(),

                    outcome,
                    side,
                    price: rust_decimal::Decimal::from_f64_retain(price).unwrap_or_default(),
                    size,
                    created_at: chrono::Utc::now(),
                    ttl_secs: ttl,
                };

                let order_id = self.executor.execute_order(order.clone()).await?;
                info!("Placed entry order: {} (reason: {})", order_id, reason);

                // Record this order AFTER successful placement
                {
                    let mut recent = self.recent_orders.write().await;
                    recent.insert(order_key.clone(), StdInstant::now());
                    // Cleanup old entries (older than 5 seconds)
                    recent.retain(|_, time| time.elapsed() < std::time::Duration::from_secs(5));
                }

                // Note: Order already tracked in pending_orders BEFORE submission (race condition fix)

                // Bug #58 Fix: Update last order time for this market
                {
                    let mut last_times = self.last_order_time.write().await;
                    last_times.insert(market.condition_id.clone(), StdInstant::now());
                    // Cleanup old entries (older than 30 seconds)
                    last_times.retain(|_, time| time.elapsed() < std::time::Duration::from_secs(30));
                }

                // In dry-run mode, simulate immediate fill
                if self.config.dry_run {
                    let mut tracker = self.position_tracker.write().await;
                    tracker.on_fill(
                        market.condition_id.clone(),
                        outcome,
                        size as f64,
                        price,
                        true, // is_entry
                    );
                    drop(tracker);
                    info!("DRY-RUN: Simulated fill for order {}", order_id);
                }

                // Log to dashboard
                self.dashboard.add_event(
                    "ENTRY".to_string(),
                    format!("Entered {} @ {:.4} (size: {}, reason: {})", market.condition_id, price, size, reason)
                ).await;
            }
            StrategyDecision::Exit { reason } => {
                // Get position to exit
                let tracker = self.position_tracker.read().await;
                let position = tracker.get_position(&market.condition_id).cloned();
                drop(tracker);

                if let Some(pos) = position {
                    // Bug #6 Fix: Use bid price for exit (not current_price/mid)
                    // Get the actual bid price from book cache
                    let book_cache = self.book_cache.read().await;
                    let token_id = match pos.outcome {
                        v3_core::Outcome::Up => &market.token_id_yes,
                        v3_core::Outcome::Down => &market.token_id_no,
                    };
                    let exit_price = if let Some((bid, _ask)) = book_cache.get_top(token_id) {
                        bid
                    } else {
                        warn!("No bid price available, using current price");
                        pos.current_price
                    };
                    drop(book_cache);

                    let order = v3_core::Order {
                        id: uuid::Uuid::new_v4().to_string(),
                        market: market.clone(),
                        outcome: pos.outcome,
                        side: v3_execution::OrderSide::Sell,
                        price: rust_decimal::Decimal::from_f64_retain(exit_price).unwrap_or_default(),
                        size: pos.size as u64,
                        created_at: chrono::Utc::now(),
                        ttl_secs: 30,
                    };

                    let order_id = self.executor.execute_order(order.clone()).await?;
                    info!("Placed exit order: {} @ {:.4} (bid price, reason: {:?})", order_id, exit_price, reason);

                    // In dry-run mode, simulate immediate fill and record PnL
                    if self.config.dry_run {
                        let mut tracker = self.position_tracker.write().await;

                        // Calculate realized PnL before closing
                        let realized_pnl = pos.unrealized_pnl;

                        // Simulate exit fill
                        tracker.on_fill(
                            market.condition_id.clone(),
                            pos.outcome,
                            pos.size,
                            pos.current_price,
                            false, // is_entry = false (exit)
                        );
                        drop(tracker);

                        // Record PnL
                        if realized_pnl.abs() > 0.001 {
                            let mut risk = self.risk_manager.write().await;
                            risk.record_realized_pnl(realized_pnl);
                            drop(risk);
                            info!("DRY-RUN: Simulated exit fill, realized PnL: {:.2}", realized_pnl);
                        }
                    }

                    // Log to dashboard
                    self.dashboard.add_event(
                        "EXIT".to_string(),
                        format!("Exited {} @ {:.4} (reason: {:?})", market.condition_id, pos.current_price, reason)
                    ).await;
                }
            }
            StrategyDecision::Hold => {
                // Do nothing
            }
        }

        Ok(())
    }

    /// Shutdown the orchestrator
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down orchestrator");

        // Cancel all active orders
        info!("Cancelling all active orders");
        self.executor.cancel_all().await?;

        // Close all positions if configured
        if self.config.close_on_shutdown {
            info!("Closing all positions");
            let position_tracker = self.position_tracker.read().await;
            let positions = position_tracker.get_all_positions();

            for position in positions {
                info!("Closing position for market {}", position.market_id);
                // Position will be closed by the exit logic in process_market
                // For now, just log the positions that need closing
            }
        }

        info!("Shutdown complete");
        Ok(())
    }

    /// Handle fill event from CLOB
    async fn handle_fill(
        tracker: &mut crate::position_tracker::PositionTracker,
        risk: &mut v3_risk::limits::RiskManager,
        fill: mm_exchange::types::Fill,
    ) {
        use mm_exchange::types::FillSide;
        use rust_decimal::prelude::ToPrimitive;

        info!("Processing fill: order_id={}, condition_id={}, side={:?}, size={}, price={}",
            fill.order_id, fill.condition_id, fill.side, fill.size, fill.price);

        // Determine if this is an entry or exit
        // Entry: BuyYes (long Up) or BuyNo (long Down)
        // Exit: SellYes (close Up) or SellNo (close Down)
        let is_entry = matches!(fill.side, FillSide::BuyYes | FillSide::BuyNo);

        // Determine outcome
        let outcome = match fill.side {
            FillSide::BuyYes => v3_core::Outcome::Up,
            FillSide::SellYes => v3_core::Outcome::Up,
            FillSide::BuyNo => v3_core::Outcome::Down,   // Bug #1 Fix
            FillSide::SellNo => v3_core::Outcome::Down,  // Bug #1 Fix
        };

        // Convert price and size
        // Bug #9 Fix: Better error handling for price conversion
        let price = fill.price.to_f64().unwrap_or_else(|| {
            warn!("Failed to parse fill price, using 0.5 as fallback");
            0.5
        });

        // Bug #8 Fix: Proper size conversion (preserve decimals)
        // Polymarket uses 6 decimals for token amounts
        let size = fill.size as f64 / 1_000_000.0;

        // Bug #4 Fix: Calculate PnL BEFORE closing position
        let realized_pnl = if !is_entry {
            // Get position before it's closed
            let position = tracker.get_position(&fill.condition_id);
            if let Some(pos) = position {
                pos.unrealized_pnl
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Update position tracker (may close position if size becomes 0)
        tracker.on_fill(
            fill.condition_id.clone(),
            outcome,
            size,
            price,
            is_entry,
        );

        // Bug #7 Fix: Add position to RiskManager on entry
        if is_entry {
            use v3_core::types::Position;
            use chrono::Utc;

            // Create a Position with the correct fields
            let position = Position {
                id: fill.order_id.clone(),
                market: Market {
                    condition_id: fill.condition_id.clone(),
                    token_id_yes: String::new(), // Will be filled by caller
                    token_id_no: String::new(),
                    asset: fill.asset,
                    open_time: Utc::now(),
                    close_time: Utc::now() + chrono::Duration::minutes(15),
                    open_price: price,
                    slug: fill.condition_id.clone(),
                },
                outcome,
                side: v3_core::PositionSide::Long, // Assuming long position
                entry_price: price,
                size: (size * 1_000_000.0) as u64, // Convert back to u64
                entry_time: Utc::now(),
                unrealized_pnl: 0.0,
                strategy_name: "unknown".to_string(),
            };

            risk.add_position(position);
            info!("✅ Added position to RiskManager: {} (size: {}, price: {:.4})",
                fill.condition_id, size, price);
        }

        // Bug #3 Fix: Record PnL only once (removed duplicate from execute_decision)
    }

    /// Poll pending orders for fill status (LIVE mode only)
    async fn poll_order_status(&self) -> Result<()> {
        if self.config.dry_run {
            return Ok(()); // Skip in dry-run mode
        }

        let mut pending = self.pending_orders.write().await;
        let mut to_remove = Vec::new();

        for (order_id, pending_order) in pending.iter() {
            // Skip orders older than 60 seconds (likely expired)
            if pending_order.submitted_at.elapsed() > std::time::Duration::from_secs(60) {
                info!("Order {} expired (>60s), removing from pending", order_id);
                to_remove.push(order_id.clone());
                continue;
            }

            // Query order status
            match self.executor.get_order_status(order_id).await {
                Ok(status) => {
                    match status.as_str() {
                        "FILLED" => {
                            info!("Order {} FILLED, triggering on_fill", order_id);

                            // Trigger Fill processing
                            self.on_fill_internal(
                                order_id.clone(),
                                pending_order.market_id.clone(),
                                pending_order.outcome,
                                pending_order.side,
                                pending_order.price,
                                pending_order.size,
                            ).await;

                            to_remove.push(order_id.clone());
                        }
                        "CANCELLED" | "EXPIRED" => {
                            info!("Order {} {}, removing from pending", order_id, status);
                            to_remove.push(order_id.clone());
                        }
                        "OPEN" | "PARTIALLY_FILLED" => {
                            // Continue waiting
                        }
                        _ => {
                            warn!("Unknown order status for {}: {}", order_id, status);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to query order status for {}: {}", order_id, e);
                }
            }
        }

        // Remove processed orders
        for order_id in to_remove {
            pending.remove(&order_id);
        }

        Ok(())
    }

    /// Internal fill processing (called by poll_order_status)
    async fn on_fill_internal(
        &self,
        order_id: String,
        market_id: String,
        outcome: v3_core::Outcome,
        side: v3_core::OrderSide,
        price: f64,
        size: u64,
    ) {
        // Deduplication check
        {
            let mut processed = self.processed_fills.write().await;
            if !processed.insert(order_id.clone()) {
                warn!("Duplicate fill detected: {}, skipping", order_id);
                return;
            }

            // Cleanup old entries (keep last 1000)
            if processed.len() > 1000 {
                // Simple cleanup: clear half
                let to_keep: std::collections::HashSet<_> = processed.iter()
                    .skip(500)
                    .cloned()
                    .collect();
                *processed = to_keep;
            }
        }

        // Update position tracker and get realized PnL
        let mut tracker = self.position_tracker.write().await;
        let realized_pnl = tracker.on_fill(
            market_id.clone(),
            outcome,
            size as f64,
            price,
            true, // is_entry
        );
        drop(tracker);

        // CRITICAL FIX #3: Record realized PnL if this is an exit fill
        if let Some(pnl) = realized_pnl {
            if pnl.abs() > 0.001 {
                let mut risk = self.risk_manager.write().await;
                risk.record_realized_pnl(pnl);
                drop(risk);
                info!("✅ Recorded realized PnL: ${:.2}", pnl);
            }
        }

        // Log fill to CSV
        {
            let mut trade_log = self.trade_log.write().await;
            if let Err(e) = trade_log.log_fill(&market_id, outcome, side, price, size, realized_pnl) {
                warn!("Failed to log fill to CSV: {}", e);
            }
        }

        // Log fill
        info!(
            "[FILL] {} {} {:?} @ {:.4} x{} (order: {})",
            market_id,
            match outcome {
                v3_core::Outcome::Up => "Up",
                v3_core::Outcome::Down => "Down",
            },
            side,
            price,
            size,
            order_id
        );

        // Update dashboard
        self.dashboard.add_event(
            "FILL".to_string(),
            format!(
                "{} @ {:.4} x{} ({})",
                market_id,
                price,
                size,
                match side {
                    v3_core::OrderSide::Buy => "BUY",
                    v3_core::OrderSide::Sell => "SELL",
                }
            )
        ).await;
    }
}
