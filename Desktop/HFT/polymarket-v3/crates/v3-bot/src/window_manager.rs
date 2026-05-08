//! Polymarket 5m / 15m / 4h BTC UP/DOWN window math.
//!
//! Polymarket uses deterministic slugs: `btc-updown-{5m|15m|4h}-{window_ts}`
//! where `window_ts = now - (now % period)`. This module encodes that math
//! so we don't have to keep string-formatting slugs by hand.
//!
//! Multi-asset (I5): Polymarket runs the same scheme for `{btc,eth,sol,xrp,
//! bnb,doge,hype}-updown-*`. `WindowKind::slug(asset, ts)` takes an Asset.
//! The bot is configured to track a subset via GengarConfig.tracked_assets
//! (default: [BTC]).
//!
//! Note: verified via live Gamma API on 2026-05-08 that Polymarket runs
//!       5m / 15m / 4h for BTC (NOT 1h — `btc-updown-1h-*` returns empty).
//!       If a 1h tier is added later, extend `WindowKind` with `H1`.

use serde::{Deserialize, Serialize};
use v3_mm_core::Asset;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    M5,
    M15,
    H4,
}

impl WindowKind {
    pub fn period_secs(&self) -> u64 {
        match self {
            WindowKind::M5 => 300,
            WindowKind::M15 => 900,
            WindowKind::H4 => 14_400,
        }
    }

    pub fn slug_prefix(&self) -> &'static str {
        match self {
            WindowKind::M5 => "5m",
            WindowKind::M15 => "15m",
            WindowKind::H4 => "4h",
        }
    }

    /// Start-of-window timestamp for any `now_secs` (UTC epoch seconds).
    pub fn current_window_ts(&self, now_secs: i64) -> u64 {
        let p = self.period_secs() as i64;
        (now_secs - now_secs.rem_euclid(p)) as u64
    }

    /// Polymarket Gamma slug for a specific asset + window ts.
    pub fn slug(&self, asset: Asset, window_ts: u64) -> String {
        format!("{}-updown-{}-{}", asset.slug_prefix(), self.slug_prefix(), window_ts)
    }

    /// Remaining seconds in the window. Can be negative if `now_secs` has
    /// already crossed the window end; callers should clamp to 0 if needed.
    pub fn seconds_remaining(&self, window_ts: u64, now_secs: i64) -> i64 {
        (window_ts as i64 + self.period_secs() as i64) - now_secs
    }

    pub fn all() -> [WindowKind; 3] {
        [WindowKind::M5, WindowKind::M15, WindowKind::H4]
    }
}

/// Per-window state owned by `WindowManager`.
#[derive(Debug, Clone)]
pub struct WindowState {
    pub asset: Asset,
    pub kind: WindowKind,
    pub window_ts: u64,
    pub slug: String,
    pub condition_id: String,
    pub token_id_up: String,
    pub token_id_down: String,
    /// Binance spot at the moment we first observed this window.
    pub opening_price: Option<f64>,
}

// ---------------------------------------------------------------------------
// WindowManager: fetches and refreshes the 3 active windows from Gamma API.
// ---------------------------------------------------------------------------

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

const GAMMA_API: &str = "https://gamma-api.polymarket.com";

pub struct WindowManager {
    http: Client,
    /// Keyed by (asset, kind).
    windows: Arc<RwLock<HashMap<(Asset, WindowKind), WindowState>>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("v3-bot/0.1")
                .build()
                .unwrap_or_else(|_| Client::new()),
            windows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// For each (asset, kind) in `tracked`, compute the current slug and
    /// (re)load it from Gamma if we haven't seen this slug before.
    ///
    /// Returns a list of `(asset, kind, old_state)` for windows that were
    /// *replaced* (rollover). Windows loaded for the first time produce no
    /// entry.
    pub async fn refresh_all(
        &self,
        tracked: &[Asset],
    ) -> Result<Vec<(Asset, WindowKind, WindowState)>> {
        let now = chrono::Utc::now().timestamp();
        let mut rolled: Vec<(Asset, WindowKind, WindowState)> = Vec::new();
        for &asset in tracked {
            for kind in WindowKind::all() {
                let key = (asset, kind);
                let ts = kind.current_window_ts(now);
                let slug = kind.slug(asset, ts);
                let (need_refresh, old_state) = {
                    let map = self.windows.read().await;
                    let existing = map.get(&key);
                    let need = existing.map_or(true, |w| w.slug != slug);
                    (need, existing.cloned())
                };
                if !need_refresh {
                    continue;
                }
                match self.fetch_window(asset, kind, ts, &slug).await {
                    Ok(ws) => {
                        self.windows.write().await.insert(key, ws);
                        info!("📥 Loaded window {:?} {:?}: {}", asset, kind, slug);
                        if let Some(old) = old_state {
                            rolled.push((asset, kind, old));
                        }
                    }
                    Err(e) => warn!("Failed to fetch {}: {:#}", slug, e),
                }
            }
        }
        Ok(rolled)
    }

    async fn fetch_window(
        &self,
        asset: Asset,
        kind: WindowKind,
        window_ts: u64,
        slug: &str,
    ) -> Result<WindowState> {
        let url = format!("{GAMMA_API}/events?slug={slug}");
        let resp: Value = self
            .http
            .get(&url)
            .send()
            .await
            .context("gamma get")?
            .json()
            .await
            .context("gamma json")?;
        let arr = resp.as_array().context("gamma response not array")?;
        let event = arr.first().context("empty gamma result")?;
        let markets = event["markets"]
            .as_array()
            .context("no markets array")?;
        let market = markets.first().context("empty markets")?;
        let condition_id = market["conditionId"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let clob_tokens: Vec<String> = match &market["clobTokenIds"] {
            Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => vec![],
        };
        let outcomes: Vec<String> = match &market["outcomes"] {
            Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
            Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => vec![],
        };

        let (mut up, mut down) = (String::new(), String::new());
        for (i, o) in outcomes.iter().enumerate() {
            match o.to_lowercase().as_str() {
                "up" => up = clob_tokens.get(i).cloned().unwrap_or_default(),
                "down" => down = clob_tokens.get(i).cloned().unwrap_or_default(),
                _ => {}
            }
        }
        if up.is_empty() {
            up = clob_tokens.first().cloned().unwrap_or_default();
        }
        if down.is_empty() {
            down = clob_tokens.get(1).cloned().unwrap_or_default();
        }

        Ok(WindowState {
            asset,
            kind,
            window_ts,
            slug: slug.to_string(),
            condition_id,
            token_id_up: up,
            token_id_down: down,
            opening_price: None,
        })
    }

    pub async fn get(&self, asset: Asset, kind: WindowKind) -> Option<WindowState> {
        self.windows.read().await.get(&(asset, kind)).cloned()
    }

    /// Set the opening price once (first write wins) for a window.
    pub async fn set_opening_price(&self, asset: Asset, kind: WindowKind, price: f64) {
        if let Some(w) = self.windows.write().await.get_mut(&(asset, kind)) {
            if w.opening_price.is_none() {
                w.opening_price = Some(price);
            }
        }
    }

    /// Return all currently subscribed CLOB token IDs (UP + DOWN per window).
    pub async fn all_tokens(&self) -> Vec<String> {
        let map = self.windows.read().await;
        let mut v = Vec::with_capacity(map.len() * 2);
        for w in map.values() {
            if !w.token_id_up.is_empty() {
                v.push(w.token_id_up.clone());
            }
            if !w.token_id_down.is_empty() {
                v.push(w.token_id_down.clone());
            }
        }
        v
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_seconds_correct() {
        assert_eq!(WindowKind::M5.period_secs(), 300);
        assert_eq!(WindowKind::M15.period_secs(), 900);
        assert_eq!(WindowKind::H4.period_secs(), 14_400);
    }

    #[test]
    fn window_ts_aligns_to_period() {
        assert_eq!(WindowKind::M5.current_window_ts(1_700_000_123), 1_700_000_100);
        assert_eq!(WindowKind::M15.current_window_ts(1_700_000_999), 1_700_000_100);
        // 4h = 14_400s; floor(1_700_000_100 / 14400) * 14400 = 1_699_992_000
        assert_eq!(WindowKind::H4.current_window_ts(1_700_000_100), 1_699_992_000);
    }

    #[test]
    fn slug_format() {
        assert_eq!(
            WindowKind::M5.slug(Asset::BTC, 1_700_000_100),
            "btc-updown-5m-1700000100"
        );
        assert_eq!(
            WindowKind::M15.slug(Asset::ETH, 1_700_000_100),
            "eth-updown-15m-1700000100"
        );
        assert_eq!(
            WindowKind::H4.slug(Asset::SOL, 1_700_000_100),
            "sol-updown-4h-1700000100"
        );
        assert_eq!(
            WindowKind::M5.slug(Asset::XRP, 1_700_000_100),
            "xrp-updown-5m-1700000100"
        );
    }

    #[test]
    fn seconds_remaining_countdown() {
        let wk = WindowKind::M5;
        let ts = 1_700_000_100_u64;
        let now = ts as i64 + 100;
        assert_eq!(wk.seconds_remaining(ts, now), 200);
    }

    #[test]
    fn seconds_remaining_negative_past_end() {
        let wk = WindowKind::M15;
        let ts = 1_700_000_000_u64;
        let now = ts as i64 + 1000; // 100s past 900s window end
        assert_eq!(wk.seconds_remaining(ts, now), -100);
    }

    #[test]
    fn all_enumerates_three_kinds() {
        assert_eq!(WindowKind::all().len(), 3);
    }
}
