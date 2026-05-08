use serde::{Deserialize, Serialize};

/// Global configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub strategies: StrategyConfigs,
    pub risk: RiskConfig,
    pub execution: ExecutionConfig,
    pub dashboard: DashboardConfig,
}

/// Strategy configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfigs {
    pub momentum: MomentumConfig,
    pub market_making: MarketMakingConfig,
    pub spread_capture: SpreadCaptureConfig,
}

/// Momentum strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumConfig {
    pub enabled: bool,
    pub min_velocity: f64,           // $/second
    pub position_size: u64,          // USDC
    pub max_hold_time: u64,          // seconds
    pub stop_loss_pct: f64,          // 0.02 = 2%
    pub take_profit_pct: f64,        // 0.03 = 3%
    pub order_ttl: u64,              // seconds
}

impl Default for MomentumConfig {
    fn default() -> Self {
        // Read position size from environment variable, default to 5 (Polymarket minimum)
        let position_size = std::env::var("MAX_POSITION_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5);  // Polymarket minimum: 5 shares

        Self {
            enabled: true,
            min_velocity: 0.0003,  // 0.03%/s (降低阈值提高入场频率)
            position_size,
            max_hold_time: 30,
            stop_loss_pct: 0.02,
            take_profit_pct: 0.03,
            order_ttl: 30,
        }
    }
}

/// Market making strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakingConfig {
    pub enabled: bool,
    pub base_spread: f64,            // 0.02 = 2%
    pub size_per_side: u64,          // USDC
    pub inventory_skew: f64,         // 0.01 = 1% per unit
    pub order_ttl: u64,              // seconds
    pub max_inventory: i64,          // max position imbalance
}

impl Default for MarketMakingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_spread: 0.02,
            size_per_side: 5,
            inventory_skew: 0.01,
            order_ttl: 30,
            max_inventory: 50,
        }
    }
}

/// Spread capture strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadCaptureConfig {
    pub enabled: bool,
    /// YES_ask + NO_ask must be < this to trigger Gabagool entry.
    pub max_pair_cost: f64,
    /// Per-leg notional size in USDC.
    pub position_size_usdc: f64,
    /// Force-close the pair if not both legs filled within this many seconds.
    pub kill_switch_secs: u64,
    /// Seconds to wait for the opposite leg to fill before we start hedging.
    pub hedge_timeout_secs: u64,
    /// Aggression used for hedge fallback orders (basis points vs leg price).
    pub hedge_slippage_bps: u32,
    /// Legacy single-leg threshold (unused by Gabagool path).
    pub min_spread_edge: f64,
    /// Max hold time for a pair position (seconds).
    pub max_hold_time: u64,
    /// Legacy per-leg share count (unused by Gabagool path, kept for compat).
    pub position_size: u64,
}

impl Default for SpreadCaptureConfig {
    fn default() -> Self {
        // Read position size from environment variable, default to 5 (Polymarket minimum)
        let position_size = std::env::var("MAX_POSITION_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5);  // Polymarket minimum: 5 shares

        Self {
            enabled: true,
            max_pair_cost: 0.975,
            position_size_usdc: 5.0,
            kill_switch_secs: 900,
            hedge_timeout_secs: 3,
            hedge_slippage_bps: 200,
            min_spread_edge: 0.01,
            max_hold_time: 900,
            position_size,
        }
    }
}

/// Risk configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size: u64,      // USDC
    pub max_positions: usize,
    pub max_daily_loss: f64,         // USDC
    pub max_hold_time: u64,          // seconds
    pub max_exposure_per_asset: f64, // USDC
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size: 10,
            max_positions: 20,
            max_daily_loss: 50.0,
            max_hold_time: 30,
            max_exposure_per_asset: 100.0,
        }
    }
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub order_ttl_secs: u64,
    pub cancel_check_interval_ms: u64,
    pub adverse_move_threshold: f64,  // price change % to trigger cancel
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            order_ttl_secs: 30,
            cancel_check_interval_ms: 100,
            adverse_move_threshold: 0.01, // 1%
        }
    }
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 3030,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            strategies: StrategyConfigs {
                momentum: MomentumConfig::default(),
                market_making: MarketMakingConfig::default(),
                spread_capture: SpreadCaptureConfig::default(),
            },
            risk: RiskConfig::default(),
            execution: ExecutionConfig::default(),
            dashboard: DashboardConfig::default(),
        }
    }
}
