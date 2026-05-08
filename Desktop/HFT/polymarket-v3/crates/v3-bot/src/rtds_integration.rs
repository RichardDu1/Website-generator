//! RTDS WebSocket integration for Chainlink price data

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;

use v3_mm_exchange::types::ExchangeEvent;

/// RTDS feed manager
pub struct RtdsFeed {
    rx: mpsc::Receiver<ExchangeEvent>,
}

impl RtdsFeed {
    /// Create a new RTDS feed and spawn background task
    pub async fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel(100);

        // Spawn RTDS WebSocket
        v3_mm_exchange::rtds::spawn_rtds_feed(tx, None).await?;

        info!("RTDS feed initialized");

        Ok(Self { rx })
    }

    /// Get next price update from RTDS (non-blocking)
    pub async fn next_price(&mut self) -> Option<v3_mm_exchange::types::RtdsPrice> {
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    match event {
                        ExchangeEvent::RtdsPrice(price) => return Some(price),
                        ExchangeEvent::RtdsAlive => continue, // Heartbeat, keep polling
                        _ => continue,
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

    /// Drain all available price updates
    pub async fn drain_prices(&mut self) -> Vec<v3_mm_exchange::types::RtdsPrice> {
        let mut prices = Vec::new();

        while let Some(price) = self.next_price().await {
            prices.push(price);
        }

        prices
    }
}
