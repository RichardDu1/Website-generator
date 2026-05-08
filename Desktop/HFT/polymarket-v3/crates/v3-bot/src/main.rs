mod config;
mod orchestrator;
mod scanner_integration;
mod binance_integration;
mod rtds_integration;
mod clob_integration;
mod position_tracker;

use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::BotConfig;
use crate::orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "v3_bot=info,v3_core=info,v3_signal=info,v3_strategy=info,v3_execution=info,v3_risk=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Polymarket V3 Bot starting...");

    // Load configuration
    let config = match BotConfig::from_env_and_file("config/bot.toml") {
        Ok(cfg) => {
            info!("Config loaded - dry_run: {}", cfg.dry_run);
            cfg
        },
        Err(e) => {
            error!("Failed to load configuration: {:#}", e);
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        error!("Config state - dry_run: {}", config.dry_run);
        std::process::exit(1);
    }

    info!("Configuration loaded successfully");
    info!("Mode: {}", if config.dry_run { "DRY-RUN" } else { "LIVE" });

    // Create orchestrator
    let orchestrator = match Orchestrator::new(config).await {
        Ok(orch) => orch,
        Err(e) => {
            error!("Failed to create orchestrator: {}", e);
            std::process::exit(1);
        }
    };

    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Received shutdown signal");
    };

    // Run orchestrator with graceful shutdown
    tokio::select! {
        result = orchestrator.run() => {
            if let Err(e) = result {
                error!("Orchestrator error: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("Initiating graceful shutdown...");
            if let Err(e) = orchestrator.shutdown().await {
                error!("Shutdown error: {}", e);
            }
        }
    }

    info!("Bot stopped");
    Ok(())
}
