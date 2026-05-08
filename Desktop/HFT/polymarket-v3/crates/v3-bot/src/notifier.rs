//! Optional Telegram webhook notifier.
//!
//! Reads `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID` from env at construction.
//! If either is missing, the notifier silently no-ops — the bot runs fine
//! without notifications in headless environments.
//!
//! All methods are fire-and-forget: if the Telegram API is unreachable or
//! returns an error, we log a warning and move on. Never block the hot path.

use reqwest::Client;
use std::sync::Arc;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct Notifier {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    http: Client,
    bot_token: String,
    chat_id: String,
}

impl Notifier {
    /// Build from explicit (Option) inputs. Primary constructor — testable
    /// without env var gymnastics. `from_env` is just a wrapper that reads
    /// the same values from the process environment.
    pub fn new(bot_token: Option<String>, chat_id: Option<String>) -> Self {
        let token = bot_token.filter(|s| !s.is_empty());
        let chat = chat_id.filter(|s| !s.is_empty());
        match (token, chat) {
            (Some(bot_token), Some(chat_id)) => {
                let http = Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_else(|_| Client::new());
                Self {
                    inner: Some(Arc::new(Inner {
                        http,
                        bot_token,
                        chat_id,
                    })),
                }
            }
            _ => Self { inner: None },
        }
    }

    /// Build from env. Reads `TELEGRAM_BOT_TOKEN` + `TELEGRAM_CHAT_ID`.
    /// Returns a disabled no-op instance when either is missing.
    pub fn from_env() -> Self {
        let token = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        let chat = std::env::var("TELEGRAM_CHAT_ID").ok();
        if token.as_deref().map(str::is_empty).unwrap_or(true)
            || chat.as_deref().map(str::is_empty).unwrap_or(true)
        {
            debug!("Telegram notifier disabled (TELEGRAM_BOT_TOKEN/CHAT_ID not set)");
        }
        Self::new(token, chat)
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Send a plain-text message to Telegram. Fire-and-forget: errors logged,
    /// never propagated.
    pub async fn notify(&self, text: &str) {
        let Some(inner) = self.inner.clone() else { return };
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            inner.bot_token
        );
        let body = serde_json::json!({
            "chat_id": inner.chat_id,
            "text": text,
            "disable_web_page_preview": true,
        });
        match inner.http.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                let status = resp.status();
                warn!("Telegram notify failed: HTTP {status}");
            }
            Err(e) => warn!("Telegram notify error: {:#}", e),
        }
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_when_both_none() {
        let n = Notifier::new(None, None);
        assert!(!n.is_enabled());
    }

    #[test]
    fn disabled_when_either_missing() {
        assert!(!Notifier::new(Some("x".into()), None).is_enabled());
        assert!(!Notifier::new(None, Some("y".into())).is_enabled());
    }

    #[test]
    fn disabled_when_values_empty() {
        assert!(!Notifier::new(Some(String::new()), Some("123".into())).is_enabled());
        assert!(!Notifier::new(Some("xyz".into()), Some(String::new())).is_enabled());
    }

    #[test]
    fn enabled_when_both_set_non_empty() {
        let n = Notifier::new(Some("fake-token".into()), Some("42".into()));
        assert!(n.is_enabled());
    }

    #[tokio::test]
    async fn notify_is_noop_when_disabled() {
        let n = Notifier::new(None, None);
        // Should complete instantly without any HTTP call.
        n.notify("test").await;
    }
}
