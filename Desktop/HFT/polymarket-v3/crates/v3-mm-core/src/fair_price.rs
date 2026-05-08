use statrs::distribution::{ContinuousCDF, Normal};

/// Rolling volatility estimator using EMA of log-returns, time-aware
pub struct VolEstimator {
    ema_var: f64,
    last_price: Option<f64>,
    last_ts: Option<std::time::Instant>,
    window_secs: f64,
    vol_floor: f64,
    vol_cap: f64,
}

impl VolEstimator {
    pub fn new(window_secs: f64, vol_floor: f64, vol_cap: f64) -> Self {
        Self {
            ema_var: 0.0,
            last_price: None,
            last_ts: None,
            window_secs,
            vol_floor,
            vol_cap,
        }
    }

    /// Feed a new price tick with automatic time tracking.
    pub fn update(&mut self, price: f64) {
        let now = std::time::Instant::now();
        if let (Some(prev), Some(prev_ts)) = (self.last_price, self.last_ts) {
            if prev > 0.0 && price > 0.0 {
                let dt = now.duration_since(prev_ts).as_secs_f64().max(0.001);
                let log_ret = (price / prev).ln();
                // Per-second variance: (log_ret^2) / dt
                let var_per_sec = (log_ret * log_ret) / dt;
                // EMA alpha scaled by dt relative to window
                let alpha = (2.0 * dt / (self.window_secs + dt)).min(1.0);
                self.ema_var = alpha * var_per_sec + (1.0 - alpha) * self.ema_var;
            }
        }
        self.last_price = Some(price);
        self.last_ts = Some(now);
    }

    /// Annualized volatility estimate
    pub fn annualized_vol(&self) -> f64 {
        // ema_var is per-second variance, annualize: * seconds_per_year
        let raw = (self.ema_var * 31_536_000.0).sqrt();
        raw.clamp(self.vol_floor, self.vol_cap)
    }

    pub fn has_data(&self) -> bool {
        self.last_price.is_some() && self.ema_var > 0.0
    }
}

/// Calculate fair probability that price ends above open price.
///
/// Uses log-normal model: P(S_T > S_0) = Φ(d)
/// where d = (ln(S/S₀) + σ²τ/2) / (σ√τ)
///
/// `current` = RTDS Chainlink price (settlement source)
/// `open` = price at window open
/// `remaining_secs` = seconds until window closes
/// `annual_vol` = annualized volatility from Binance ticks
pub fn prob_up(current: f64, open: f64, remaining_secs: f64, annual_vol: f64) -> f64 {
    if remaining_secs <= 0.0 {
        return if current >= open { 0.99 } else { 0.01 };
    }
    if open <= 0.0 || current <= 0.0 {
        return 0.5;
    }

    let tau = remaining_secs / 31_536_000.0;
    let sigma_sqrt_tau = annual_vol * tau.sqrt();

    if sigma_sqrt_tau < 1e-12 {
        return if current >= open { 0.99 } else { 0.01 };
    }

    let d = ((current / open).ln() + 0.5 * annual_vol.powi(2) * tau) / sigma_sqrt_tau;

    let normal = Normal::new(0.0, 1.0).unwrap();
    let p = normal.cdf(d);

    p.clamp(0.01, 0.99)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_at_the_money() {
        // Price == open, 7.5 min left → should be ~0.50
        let p = prob_up(100_000.0, 100_000.0, 450.0, 0.50);
        assert!((p - 0.5).abs() < 0.02, "ATM prob should be ~0.50, got {p}");
    }

    #[test]
    fn test_deep_itm() {
        // Price well above open → should approach 1.0
        let p = prob_up(101_000.0, 100_000.0, 60.0, 0.50);
        assert!(p > 0.90, "Deep ITM should be >0.90, got {p}");
    }

    #[test]
    fn test_deep_otm() {
        // Price well below open → should approach 0.0
        let p = prob_up(99_000.0, 100_000.0, 60.0, 0.50);
        assert!(p < 0.10, "Deep OTM should be <0.10, got {p}");
    }

    #[test]
    fn test_expired() {
        assert!((prob_up(101.0, 100.0, 0.0, 0.5) - 0.99).abs() < 0.001);
        assert!((prob_up(99.0, 100.0, 0.0, 0.5) - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_vol_estimator() {
        let mut ve = VolEstimator::new(300.0, 0.15, 2.0);
        // Feed slightly varying prices with small sleeps to simulate ticks
        for i in 0..100 {
            let price = 100_000.0 + (i as f64 % 2.0) * 0.01;
            ve.update(price);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(ve.has_data());
        // With tiny variance, annualized vol should clamp to floor
        assert!((ve.annualized_vol() - 0.15).abs() < 0.01);
    }
}
