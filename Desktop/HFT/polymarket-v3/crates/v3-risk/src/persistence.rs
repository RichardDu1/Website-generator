use std::path::Path;
use std::fs;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use tracing::{debug, warn};

/// Daily PnL state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyPnlState {
    pub date: String,  // YYYY-MM-DD format
    pub daily_pnl: f64,
    pub last_updated: DateTime<Utc>,
}

impl DailyPnlState {
    pub fn new(daily_pnl: f64) -> Self {
        let now = Utc::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            daily_pnl,
            last_updated: now,
        }
    }

    /// Save to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize PnL state")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .context("Failed to create state directory")?;
        }

        fs::write(path.as_ref(), json)
            .context("Failed to write PnL state file")?;

        debug!("Saved daily PnL state: {:.2} for {}", self.daily_pnl, self.date);
        Ok(())
    }

    /// Load from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .context("Failed to read PnL state file")?;

        let state: Self = serde_json::from_str(&content)
            .context("Failed to parse PnL state file")?;

        debug!("Loaded daily PnL state: {:.2} for {}", state.daily_pnl, state.date);
        Ok(state)
    }

    /// Check if this state is for today
    pub fn is_today(&self) -> bool {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.date == today
    }

    /// Load or create new state for today
    pub fn load_or_new<P: AsRef<Path>>(path: P) -> Self {
        match Self::load(&path) {
            Ok(state) => {
                if state.is_today() {
                    debug!("Restored daily PnL: {:.2}", state.daily_pnl);
                    state
                } else {
                    warn!("PnL state is from {}, starting fresh for today", state.date);
                    Self::new(0.0)
                }
            }
            Err(e) => {
                debug!("No existing PnL state found ({}), starting fresh", e);
                Self::new(0.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_and_load() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let state = DailyPnlState::new(15.5);
        state.save(path).unwrap();

        let loaded = DailyPnlState::load(path).unwrap();
        assert_eq!(loaded.daily_pnl, 15.5);
        assert_eq!(loaded.date, state.date);
    }

    #[test]
    fn test_is_today() {
        let state = DailyPnlState::new(10.0);
        assert!(state.is_today());

        let old_state = DailyPnlState {
            date: "2020-01-01".to_string(),
            daily_pnl: 5.0,
            last_updated: Utc::now(),
        };
        assert!(!old_state.is_today());
    }

    #[test]
    fn test_load_or_new_missing_file() {
        let state = DailyPnlState::load_or_new("/tmp/nonexistent_pnl_state.json");
        assert_eq!(state.daily_pnl, 0.0);
        assert!(state.is_today());
    }

    #[test]
    fn test_load_or_new_old_date() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write old state
        let old_state = DailyPnlState {
            date: "2020-01-01".to_string(),
            daily_pnl: 100.0,
            last_updated: Utc::now(),
        };
        old_state.save(path).unwrap();

        // Should create new state for today
        let state = DailyPnlState::load_or_new(path);
        assert_eq!(state.daily_pnl, 0.0);
        assert!(state.is_today());
    }
}
