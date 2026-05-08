use serde::{Deserialize, Serialize};
use crate::Outcome;

/// A single leg of a synthetic arbitrage pair (YES or NO side).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairLeg {
    pub outcome: Outcome,
    pub token_id: String,
    pub price: f64,
    pub size: u64,
}

impl PairLeg {
    /// Notional value of this leg in USDC.
    pub fn notional_usdc(&self) -> f64 {
        self.price * self.size as f64
    }
}

/// pair_cost = yes_price + no_price. <1.0 means a risk-free arb exists
/// (assuming fills at these prices and no fees).
pub fn pair_cost(yes: &PairLeg, no: &PairLeg) -> f64 {
    yes.price + no.price
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Outcome;

    #[test]
    fn test_pair_leg_total_usdc() {
        let leg = PairLeg {
            outcome: Outcome::Up,
            token_id: "yes".into(),
            price: 0.48,
            size: 10,
        };
        assert!((leg.notional_usdc() - 4.8).abs() < 1e-9);
    }

    #[test]
    fn test_pair_legs_sum_below_dollar_is_profitable() {
        let yes = PairLeg { outcome: Outcome::Up,   token_id: "y".into(), price: 0.47, size: 10 };
        let no  = PairLeg { outcome: Outcome::Down, token_id: "n".into(), price: 0.49, size: 10 };
        assert!((pair_cost(&yes, &no) - 0.96).abs() < 1e-9);
        assert!(pair_cost(&yes, &no) < 0.975);
    }
}
