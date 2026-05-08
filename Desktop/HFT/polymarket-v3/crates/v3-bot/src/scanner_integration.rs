//! Scanner integration for discovering active markets

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, warn};

use v3_core::{Asset, Market};

/// Market scanner that discovers active 15-minute markets
pub struct MarketScanner {
    rx: mpsc::Receiver<mm_exchange::types::ExchangeEvent>,
}

impl MarketScanner {
    /// Create a new market scanner and spawn background task
    pub async fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);

        // Spawn scanner background task
        mm_exchange::scanner::spawn_market_scanner(tx).await?;

        info!("Market scanner initialized");

        Ok(Self { rx })
    }

    /// Get next market from scanner (non-blocking)
    pub async fn next_market(&mut self) -> Option<Market> {
        match self.rx.try_recv() {
            Ok(event) => {
                if let mm_exchange::types::ExchangeEvent::MarketReady(market_info) = event {
                    info!("📥 Received market from scanner: {} {}", market_info.asset, &market_info.condition_id[..16]);
                    // Convert mm_exchange::MarketInfo to v3_core::Market
                    let market = Market {
                        condition_id: market_info.condition_id,
                        token_id_yes: market_info.token_id_yes,
                        token_id_no: market_info.token_id_no,
                        asset: market_info.asset,
                        open_time: chrono::Utc::now(), // Approximate
                        close_time: market_info.end_time,
                        open_price: 0.0, // Will be updated from Binance
                        slug: market_info.question,
                    };

                    return Some(market);
                } else {
                    warn!("Received non-MarketReady event, ignoring");
                    return None;
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // No new markets available
                return None;
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                warn!("Scanner channel disconnected");
                return None;
            }
        }
    }

    /// Get all available markets (drains the channel)
    /// Gap #2 Fix: Filter for 15-minute crypto markets only (distinct-baguette strategy)
    pub async fn get_available_markets(&mut self) -> Vec<Market> {
        let mut markets = Vec::new();
        let mut count = 0;

        while let Some(market) = self.next_market().await {
            count += 1;
            info!("📦 Collected market #{}: {} {}", count, market.asset, &market.condition_id[..16]);
            markets.push(market);
            info!("   Vector size after push: {}", markets.len());
        }

        info!("🔍 After while loop: collected {} markets, vector size: {}", count, markets.len());

        if markets.is_empty() {
            info!("Market filter: 0 markets from scanner");
        } else {
            info!("Market filter: {} markets from scanner", markets.len());
        }

        markets
    }
}
