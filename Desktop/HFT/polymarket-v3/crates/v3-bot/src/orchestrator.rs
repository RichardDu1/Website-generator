//! Placeholder orchestrator while Gengar rewrite is in flight.
//!
//! Task 1A deletes the Momentum / MarketMaking / SpreadCapture plumbing.
//! Task 4B replaces this file with the real GengarOrchestrator.
//! Meanwhile this stub keeps `main.rs` compiling.

use anyhow::Result;
use tracing::{info, warn};

use crate::config::BotConfig;

pub struct Orchestrator {
    config: BotConfig,
}

impl Orchestrator {
    pub async fn new(config: BotConfig) -> Result<Self> {
        info!("Orchestrator stub: Gengar rewrite in progress. dry_run={}", config.dry_run);
        Ok(Self { config })
    }

    pub async fn run(&self) -> Result<()> {
        warn!(
            "⚠️  Orchestrator stub active. No strategy wired yet (Task 4B pending). dry_run={}",
            self.config.dry_run
        );
        // Park forever so `tokio::select!` in main.rs waits for ctrl-c.
        std::future::pending::<()>().await;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Orchestrator stub shutdown");
        Ok(())
    }
}
