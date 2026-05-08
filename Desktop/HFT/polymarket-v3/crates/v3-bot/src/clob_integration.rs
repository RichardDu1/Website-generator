//! CLOB WebSocket integration for real-time order book updates and fill events

use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::info;

use v3_mm_exchange::types::{ExchangeEvent, BookSnapshot};
use v3_mm_core::Asset;

/// CLOB feed manager
pub struct ClobFeed {
    rx: mpsc::Receiver<ExchangeEvent>,
    subscribed_tokens: std::collections::HashSet<String>,
}

impl ClobFeed {
    /// Create a new CLOB feed and spawn background task
    ///
    /// # Arguments
    /// * `token_ids` - List of token IDs to subscribe to (e.g., ["123456", "789012"])
    /// * `cid_to_asset` - Mapping from condition_id to Asset for market resolution events
    pub async fn new(
        token_ids: Vec<String>,
        cid_to_asset: HashMap<String, Asset>,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel(1000); // Larger buffer for high-frequency book updates

        // Spawn CLOB WebSocket
        v3_mm_exchange::polymarket::spawn_clob_ws(tx, token_ids.clone(), cid_to_asset, None).await?;

        info!("CLOB feed initialized with {} tokens", token_ids.len());

        Ok(Self {
            rx,
            subscribed_tokens: token_ids.into_iter().collect(),
        })
    }

    /// Resubscribe with new token IDs if any are new.
    /// Returns true if a new subscription was started.
    pub async fn ensure_subscribed(
        &mut self,
        token_ids: Vec<String>,
        cid_to_asset: HashMap<String, Asset>,
    ) -> Result<bool> {
        let new_tokens: Vec<String> = token_ids
            .iter()
            .filter(|t| !self.subscribed_tokens.contains(*t))
            .cloned()
            .collect();

        if new_tokens.is_empty() {
            return Ok(false);
        }

        info!("CLOB: subscribing to {} new tokens", new_tokens.len());

        // Restart CLOB WS with full token list
        let all_tokens: Vec<String> = self.subscribed_tokens.iter().cloned()
            .chain(new_tokens.iter().cloned())
            .collect();

        let (tx, rx) = mpsc::channel(1000);
        v3_mm_exchange::polymarket::spawn_clob_ws(tx, all_tokens.clone(), cid_to_asset, None).await?;

        self.rx = rx;
        self.subscribed_tokens = all_tokens.into_iter().collect();

        info!("CLOB: resubscribed to {} total tokens", self.subscribed_tokens.len());
        Ok(true)
    }

    /// Get next event from CLOB (non-blocking)
    pub async fn try_recv(&mut self) -> Option<ExchangeEvent> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(_) => None,
        }
    }

    /// Get next book update from CLOB (non-blocking)
    pub async fn next_book_update(&mut self) -> Option<BookSnapshot> {
        loop {
            match self.rx.try_recv() {
                Ok(event) => {
                    match event {
                        ExchangeEvent::BookUpdate(snapshot) => return Some(snapshot),
                        _ => continue, // Skip other events (MarketResolved, Fill, etc.)
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

    /// Drain all available book updates
    pub async fn drain_book_updates(&mut self) -> Vec<BookSnapshot> {
        let mut updates = Vec::new();

        while let Some(snapshot) = self.next_book_update().await {
            updates.push(snapshot);
        }

        updates
    }

    /// Drain all available events (book updates, fills, etc.)
    pub async fn drain_events(&mut self) -> Vec<ExchangeEvent> {
        let mut events = Vec::new();

        while let Some(event) = self.try_recv().await {
            events.push(event);
        }

        events
    }
}

/// Book cache for fast lookups
pub struct BookCache {
    cache: HashMap<String, BookSnapshot>,
}

impl BookCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Update cache with new book snapshot
    pub fn update(&mut self, snapshot: BookSnapshot) {
        self.cache.insert(snapshot.token_id.clone(), snapshot);
    }

    /// Get cached book for a token
    pub fn get(&self, token_id: &str) -> Option<&BookSnapshot> {
        self.cache.get(token_id)
    }

    /// Get best bid/ask for a token
    pub fn get_top(&self, token_id: &str) -> Option<(f64, f64)> {
        let snapshot = self.cache.get(token_id)?;

        let best_bid = snapshot.bids.iter()
            .filter_map(|l| l.price.to_string().parse::<f64>().ok())
            .fold(f64::NEG_INFINITY, f64::max);
        let best_ask = snapshot.asks.iter()
            .filter_map(|l| l.price.to_string().parse::<f64>().ok())
            .fold(f64::INFINITY, f64::min);

        if best_bid == f64::NEG_INFINITY || best_ask == f64::INFINITY {
            return None;
        }
        Some((best_bid, best_ask))
    }

    /// Clear stale entries (optional, for memory management)
    pub fn clear_stale(&mut self, max_age_secs: i64) {
        let now = chrono::Utc::now();
        self.cache.retain(|_, snapshot| {
            (now - snapshot.ts).num_seconds() < max_age_secs
        });
    }

    /// Order book imbalance ratio for a token.
    /// Returns (bid_vol - ask_vol) / (bid_vol + ask_vol) in [-1.0, 1.0].
    /// Positive = buy pressure, negative = sell pressure.
    /// Uses top N levels (default 5).
    pub fn imbalance_ratio(&self, token_id: &str, levels: usize) -> f64 {
        let Some(snapshot) = self.cache.get(token_id) else {
            return 0.0;
        };

        let bid_vol: f64 = snapshot.bids.iter().take(levels)
            .filter_map(|l| l.size.to_string().parse::<f64>().ok())
            .sum();
        let ask_vol: f64 = snapshot.asks.iter().take(levels)
            .filter_map(|l| l.size.to_string().parse::<f64>().ok())
            .sum();

        let total = bid_vol + ask_vol;
        if total == 0.0 {
            return 0.0;
        }
        (bid_vol - ask_vol) / total
    }

    /// Sum of top N bid levels in USDC.
    pub fn bid_depth(&self, token_id: &str, levels: usize) -> f64 {
        let Some(snapshot) = self.cache.get(token_id) else {
            return 0.0;
        };
        snapshot.bids.iter().take(levels)
            .filter_map(|l| {
                let price = l.price.to_string().parse::<f64>().ok()?;
                let size = l.size.to_string().parse::<f64>().ok()?;
                Some(price * size)
            })
            .sum()
    }

    /// Sum of top N ask levels in USDC.
    pub fn ask_depth(&self, token_id: &str, levels: usize) -> f64 {
        let Some(snapshot) = self.cache.get(token_id) else {
            return 0.0;
        };
        snapshot.asks.iter().take(levels)
            .filter_map(|l| {
                let price = l.price.to_string().parse::<f64>().ok()?;
                let size = l.size.to_string().parse::<f64>().ok()?;
                Some(price * size)
            })
            .sum()
    }

    /// Spread capture margin: 1.0 - (best_yes_ask + best_no_ask).
    /// Positive means risk-neutral arbitrage profit is available.
    /// token_yes and token_no are the YES and NO token IDs for the same market.
    pub fn spread_capture_margin(&self, token_yes: &str, token_no: &str) -> f64 {
        let yes_ask = self.cache.get(token_yes)
            .map(|s| s.asks.iter()
                .filter_map(|l| l.price.to_string().parse::<f64>().ok())
                .fold(f64::INFINITY, f64::min))
            .unwrap_or(1.0);
        let no_ask = self.cache.get(token_no)
            .map(|s| s.asks.iter()
                .filter_map(|l| l.price.to_string().parse::<f64>().ok())
                .fold(f64::INFINITY, f64::min))
            .unwrap_or(1.0);
        let yes_ask = if yes_ask == f64::INFINITY { 1.0 } else { yes_ask };
        let no_ask = if no_ask == f64::INFINITY { 1.0 } else { no_ask };
        1.0 - (yes_ask + no_ask)
    }
}
