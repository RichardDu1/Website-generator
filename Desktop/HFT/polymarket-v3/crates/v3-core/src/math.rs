/// Calculate price velocity ($/second)
pub fn price_velocity(prices: &[(f64, f64)]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }

    let (time1, price1) = prices[0];
    let (time2, price2) = prices[prices.len() - 1];

    let time_diff = time2 - time1;
    if time_diff == 0.0 {
        return 0.0;
    }

    (price2 - price1) / time_diff
}

/// Calculate price acceleration ($/second²)
pub fn price_acceleration(velocities: &[(f64, f64)]) -> f64 {
    if velocities.len() < 2 {
        return 0.0;
    }

    let (time1, vel1) = velocities[0];
    let (time2, vel2) = velocities[velocities.len() - 1];

    let time_diff = time2 - time1;
    if time_diff == 0.0 {
        return 0.0;
    }

    (vel2 - vel1) / time_diff
}

/// Calculate simple moving average
pub fn sma(prices: &[f64]) -> f64 {
    if prices.is_empty() {
        return 0.0;
    }
    prices.iter().sum::<f64>() / prices.len() as f64
}

/// Calculate standard deviation
pub fn std_dev(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }

    let mean = sma(prices);
    let variance = prices.iter()
        .map(|p| (p - mean).powi(2))
        .sum::<f64>() / prices.len() as f64;

    variance.sqrt()
}

/// Calculate volatility (normalized std dev)
pub fn volatility(prices: &[f64]) -> f64 {
    if prices.is_empty() {
        return 0.0;
    }

    let mean = sma(prices);
    if mean == 0.0 {
        return 0.0;
    }

    std_dev(prices) / mean
}

/// Convert probability to price (0-1 -> 0-1)
pub fn prob_to_price(prob: f64) -> f64 {
    prob.clamp(0.01, 0.99)
}

/// Convert price to probability (0-1 -> 0-1)
pub fn price_to_prob(price: f64) -> f64 {
    price.clamp(0.01, 0.99)
}

/// Calculate Kelly criterion position size
pub fn kelly_size(win_prob: f64, win_amount: f64, loss_amount: f64, bankroll: f64) -> f64 {
    if win_prob <= 0.0 || win_prob >= 1.0 {
        return 0.0;
    }

    let loss_prob = 1.0 - win_prob;
    let kelly = (win_prob * win_amount - loss_prob * loss_amount) / win_amount;

    // Use fractional Kelly (25%) for safety
    let fraction = 0.25;
    (kelly * fraction * bankroll).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_velocity() {
        let prices = vec![(0.0, 100.0), (1.0, 105.0), (2.0, 110.0)];
        let vel = price_velocity(&prices);
        assert!((vel - 5.0).abs() < 0.01); // $5/second
    }

    #[test]
    fn test_sma() {
        let prices = vec![100.0, 110.0, 120.0];
        assert_eq!(sma(&prices), 110.0);
    }

    #[test]
    fn test_volatility() {
        let prices = vec![100.0, 100.0, 100.0];
        assert_eq!(volatility(&prices), 0.0);
    }
}
