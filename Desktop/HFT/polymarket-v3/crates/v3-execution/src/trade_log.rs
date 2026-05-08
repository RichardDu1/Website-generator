use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use anyhow::Result;
use chrono::Utc;
use v3_core::{OrderSide, Outcome};

/// Trade logger - persists all fills to CSV
pub struct TradeLog {
    file: File,
}

impl TradeLog {
    /// Create or open trade log file
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_exists = path.as_ref().exists();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        // Write CSV header if new file
        if !file_exists {
            writeln!(file, "timestamp,market_id,outcome,side,price,size,value,pnl")?;
        }

        Ok(Self { file })
    }

    /// Log a fill event
    pub fn log_fill(
        &mut self,
        market_id: &str,
        outcome: Outcome,
        side: OrderSide,
        price: f64,
        size: u64,
        pnl: Option<f64>,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let outcome_str = match outcome {
            Outcome::Up => "Up",
            Outcome::Down => "Down",
        };
        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        let value = price * size as f64;
        let pnl_str = pnl.map(|p| format!("{:.4}", p)).unwrap_or_else(|| "N/A".to_string());

        writeln!(
            self.file,
            "{},{},{},{},{:.4},{},{:.4},{}",
            timestamp, market_id, outcome_str, side_str, price, size, value, pnl_str
        )?;

        self.file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_trade_log_creation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trade.csv");
        let log = TradeLog::new(&path).unwrap();
        drop(log);

        // Verify header was written
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("timestamp,market_id,outcome,side,price,size,value,pnl"));
    }

    #[test]
    fn test_log_fill() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trade.csv");
        let mut log = TradeLog::new(&path).unwrap();

        log.log_fill(
            "test_market",
            Outcome::Up,
            OrderSide::Buy,
            0.52,
            10,
            Some(0.5),
        ).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test_market"));
        assert!(content.contains("Up"));
        assert!(content.contains("BUY"));
        assert!(content.contains("0.52"));
    }
}
