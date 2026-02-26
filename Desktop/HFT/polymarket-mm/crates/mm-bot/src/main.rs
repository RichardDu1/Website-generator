//! Polymarket 15-min Market Maker Bot

mod orchestrator;
pub mod trade_log;

use anyhow::Result;
use mm_core::Asset;
use mm_core::quoter::QuoterConfig;
use mm_exchange::binance::spawn_binance_feed;
use mm_exchange::polymarket::OrderClient;
use mm_exchange::rtds::spawn_rtds_feed;
use mm_exchange::scanner::spawn_market_scanner;
use mm_exchange::types::ExchangeEvent;
use mm_risk::limits::RiskConfig;
use orchestrator::Orchestrator;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::EnvFilter;

const POLY_REST_URL: &str = "https://clob.polymarket.com";

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Parse ASSETS env var (e.g. "BTC,ETH") into a filter set. None = all assets.
fn parse_assets_filter() -> Option<HashSet<Asset>> {
    let val = std::env::var("ASSETS").ok()?;
    let set: HashSet<Asset> = val
        .split(',')
        .filter_map(|s| match s.trim().to_uppercase().as_str() {
            "BTC" => Some(Asset::BTC),
            "ETH" => Some(Asset::ETH),
            "SOL" => Some(Asset::SOL),
            "XRP" => Some(Asset::XRP),
            _ => None,
        })
        .collect();
    if set.is_empty() { None } else { Some(set) }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!("Polymarket Market Maker starting...");

    // Determine dry_run mode
    let dry_run = std::env::var("DRY_RUN").unwrap_or_else(|_| "true".into()) != "false";

    // Build order client if not dry_run and credentials are available
    let order_client = if !dry_run {
        let pk = std::env::var("POLY_PRIVATE_KEY")
            .expect("POLY_PRIVATE_KEY required when DRY_RUN=false");
        let client = OrderClient::new(POLY_REST_URL, &pk).await?;
        info!("Order client authenticated (live mode)");
        Some(client)
    } else {
        info!("Running in dry-run mode (no real orders)");
        None
    };

    // Channel for all exchange events
    let (tx, rx) = mpsc::channel::<ExchangeEvent>(512);

    // Spawn data feeds
    spawn_rtds_feed(tx.clone(), None).await?;
    spawn_binance_feed(
        tx.clone(),
        &["btcusdt", "ethusdt", "solusdt", "xrpusdt"],
        None,
    ).await?;

    // Spawn market scanner (discovers active 15-min windows)
    spawn_market_scanner(tx.clone()).await?;

    let quoter_config = QuoterConfig {
        base_spread: env_f64("BASE_SPREAD", 0.015),
        inventory_skew: env_f64("INVENTORY_SKEW", 0.003),
        min_edge: env_f64("MIN_EDGE", 0.005),
        edge_asymmetry: env_f64("EDGE_ASYMMETRY", 0.015),
        size_per_quote: env_u64("SIZE_PER_QUOTE", 20),
        requote_threshold: env_f64("REQUOTE_THRESHOLD", 0.01),
        fair_p_cutoff: env_f64("FAIR_P_CUTOFF", 0.05),
    };

    let risk_config = RiskConfig {
        max_daily_loss: env_f64("MAX_DAILY_LOSS", 200.0),
        rtds_stale_timeout: Duration::from_secs(env_u64("RTDS_STALE_TIMEOUT_SECS", 30)),
        binance_stale_timeout: Duration::from_millis(env_u64("BINANCE_STALE_TIMEOUT_MS", 5000)),
        max_open_orders: env_usize("MAX_OPEN_ORDERS", 40),
    };

    let max_per_market = env_f64("MAX_PER_MARKET", 50.0);
    let max_per_asset = env_f64("MAX_PER_ASSET", 200.0);
    let max_total = env_f64("MAX_TOTAL", 500.0);
    let wind_down_secs = env_f64("WIND_DOWN_SECS", 120.0);
    let stop_quote_secs = env_f64("STOP_QUOTE_SECS", 60.0);
    let assets_filter = parse_assets_filter();

    info!(
        "Config: size_per_quote={} max_per_market={} max_daily_loss={} assets={:?}",
        quoter_config.size_per_quote, max_per_market, risk_config.max_daily_loss, assets_filter
    );

    let mut orch = Orchestrator::new(
        quoter_config,
        max_per_market,
        max_per_asset,
        max_total,
        risk_config,
        dry_run,
        wind_down_secs,
        stop_quote_secs,
        order_client,
        assets_filter,
        tx.clone(),
    );

    info!("Orchestrator ready, entering event loop (dry_run={dry_run})");
    orch.run(rx).await?;

    Ok(())
}
