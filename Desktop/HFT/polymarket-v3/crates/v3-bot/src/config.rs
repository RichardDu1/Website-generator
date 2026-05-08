use serde::Deserialize;
use anyhow::{Result, Context};
use std::path::Path;
use v3_core::{MomentumConfig, MarketMakingConfig, SpreadCaptureConfig};
use v3_risk::RiskLimits;

/// Bot configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    /// Dry-run mode (no real orders)
    pub dry_run: bool,

    /// Event loop tick interval (milliseconds)
    pub tick_interval_ms: u64,

    /// Close all positions on shutdown
    #[serde(default)]
    pub close_on_shutdown: bool,

    /// Polymarket API configuration
    pub polymarket: PolymarketConfig,

    /// Strategy configurations
    pub strategies: StrategyConfigs,

    /// Risk configuration
    pub risk: RiskLimits,

    /// Execution configuration
    pub execution: ExecutionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketConfig {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
    pub chain_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfigs {
    pub momentum: MomentumConfig,
    pub market_making: MarketMakingConfig,
    pub spread_capture: SpreadCaptureConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    /// Order TTL in seconds
    pub order_ttl_secs: u64,

    /// Adverse movement threshold for preemptive cancel ($/second)
    pub adverse_threshold: f64,

    /// Cancel monitor check interval (milliseconds)
    pub cancel_check_interval_ms: u64,

    /// Position monitor check interval (milliseconds)
    pub position_check_interval_ms: u64,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            order_ttl_secs: 30,
            adverse_threshold: 0.5,
            cancel_check_interval_ms: 100,
            position_check_interval_ms: 1000,
        }
    }
}

impl BotConfig {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read config file")?;

        let config: Self = toml::from_str(&content)
            .context("Failed to parse config file")?;

        Ok(config)
    }

    /// Load configuration from environment variables and TOML
    pub fn from_env_and_file<P: AsRef<Path>>(toml_path: P) -> Result<Self> {
        // Load .env file
        dotenv::dotenv().ok();

        // Load base config from TOML
        let mut config = Self::from_file(toml_path)?;

        // Override with environment variables
        if let Ok(api_key) = std::env::var("POLYMARKET_API_KEY") {
            config.polymarket.api_key = api_key;
        }
        if let Ok(api_secret) = std::env::var("POLYMARKET_API_SECRET") {
            config.polymarket.api_secret = api_secret;
        }
        if let Ok(api_passphrase) = std::env::var("POLYMARKET_API_PASSPHRASE") {
            config.polymarket.api_passphrase = api_passphrase;
        }
        if let Ok(chain_id) = std::env::var("CHAIN_ID") {
            config.polymarket.chain_id = chain_id.parse()
                .context("Invalid CHAIN_ID")?;
        }
        if let Ok(dry_run) = std::env::var("DRY_RUN") {
            config.dry_run = dry_run.parse()
                .context("Invalid DRY_RUN")?;
        }

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // API keys are not required - we use PRIVATE_KEY for OrderClient
        // Validation removed as polymarket-mm uses private key authentication

        if self.tick_interval_ms == 0 {
            anyhow::bail!("tick_interval_ms must be > 0");
        }

        if self.execution.order_ttl_secs == 0 {
            anyhow::bail!("order_ttl_secs must be > 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_execution_config() {
        let config = ExecutionConfig::default();
        assert_eq!(config.order_ttl_secs, 30);
        assert_eq!(config.adverse_threshold, 0.5);
    }

    #[test]
    fn test_config_validation() {
        let mut config = BotConfig {
            dry_run: true,
            tick_interval_ms: 100,
            close_on_shutdown: false,
            polymarket: PolymarketConfig {
                api_key: String::new(),
                api_secret: String::new(),
                api_passphrase: String::new(),
                chain_id: 137,
            },
            strategies: StrategyConfigs {
                momentum: MomentumConfig::default(),
                market_making: MarketMakingConfig::default(),
                spread_capture: SpreadCaptureConfig::default(),
            },
            risk: RiskLimits::default(),
            execution: ExecutionConfig::default(),
        };

        // Should pass in dry-run mode
        assert!(config.validate().is_ok());

        // Should fail in live mode without API keys
        config.dry_run = false;
        assert!(config.validate().is_err());
    }
}
