//! Binance WebSocket integration for real-time price data

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use v3_mm_exchange::types::ExchangeEvent;

/// Binance feed manager
pub struct BinanceFeed {
    rx: mpsc::Receiver<ExchangeEvent>,
}

impl BinanceFeed {
    /// Create a new Binance feed and spawn background task
    pub async fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);

        // Spawn Binance WebSocket with all supported symbols
        let symbols = vec!["BTCUSDT", "ETHUSDT", "SOLUSDT", "XRPUSDT"];
        info!("Spawning Binance feed for symbols: {:?}", symbols);

        match v3_mm_exchange::binance::spawn_binance_feed(tx, &symbols, None).await {
            Ok(_) => info!("Binance feed spawned successfully"),
            Err(e) => {
                tracing::error!("Failed to spawn Binance feed: {}", e);
                return Err(e);
            }
        }

        info!("Binance feed initialized");

        Ok(Self { rx })
    }

    /// Get next tick from Binance (non-blocking)
    pub async fn next_tick(&mut self) -> Option<v3_mm_exchange::types::BinanceTick> {
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    if let ExchangeEvent::BinanceTick(tick) = event {
                        return Some(tick);
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    return None;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return None;
                }
            }
        }
    }

    /// Drain all available ticks
    pub async fn drain_ticks(&mut self) -> Vec<v3_mm_exchange::types::BinanceTick> {
        let mut ticks = Vec::new();

        while let Some(tick) = self.next_tick().await {
            ticks.push(tick);
        }

        ticks
    }
}
