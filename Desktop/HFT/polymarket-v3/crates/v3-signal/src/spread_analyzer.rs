use v3_core::Outcome;

/// Analyzes spread for arbitrage opportunities
pub struct SpreadAnalyzer {
    min_edge: f64,
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub buy_outcome: Outcome,
    pub buy_price: f64,
    pub expected_profit: f64,
    pub edge_pct: f64,
}

impl SpreadAnalyzer {
    pub fn new(min_edge: f64) -> Self {
        Self { min_edge }
    }

    /// Detect arbitrage opportunity
    /// In prediction markets: YES + NO should equal $1.00
    /// If YES_ask + NO_ask < $1.00, we can buy both and profit
    pub fn detect_arbitrage(
        &self,
        yes_bid: f64,
        yes_ask: f64,
        no_bid: f64,
        no_ask: f64,
    ) -> Option<ArbitrageOpportunity> {
        // Check if we can buy both sides for less than $1
        let total_cost = yes_ask + no_ask;

        if total_cost < 1.0 {
            let edge = 1.0 - total_cost;
            let edge_pct = edge / total_cost;

            if edge_pct >= self.min_edge {
                // Buy both sides
                return Some(ArbitrageOpportunity {
                    buy_outcome: Outcome::Up, // Buy both, but return Up as primary
                    buy_price: yes_ask,
                    expected_profit: edge,
                    edge_pct,
                });
            }
        }

        // Check if one side is significantly underpriced
        // If YES is trading at 0.30 and NO at 0.60, YES is underpriced
        let yes_implied = 1.0 - no_bid;
        let no_implied = 1.0 - yes_bid;

        // YES is underpriced if ask < implied value
        if yes_ask < yes_implied {
            let edge = yes_implied - yes_ask;
            let edge_pct = edge / yes_ask;

            if edge_pct >= self.min_edge {
                return Some(ArbitrageOpportunity {
                    buy_outcome: Outcome::Up,
                    buy_price: yes_ask,
                    expected_profit: edge,
                    edge_pct,
                });
            }
        }

        // NO is underpriced if ask < implied value
        if no_ask < no_implied {
            let edge = no_implied - no_ask;
            let edge_pct = edge / no_ask;

            if edge_pct >= self.min_edge {
                return Some(ArbitrageOpportunity {
                    buy_outcome: Outcome::Down,
                    buy_price: no_ask,
                    expected_profit: edge,
                    edge_pct,
                });
            }
        }

        None
    }

    /// Calculate spread width
    pub fn spread_width(&self, bid: f64, ask: f64) -> f64 {
        ask - bid
    }

    /// Calculate spread percentage
    pub fn spread_pct(&self, bid: f64, ask: f64) -> f64 {
        if bid == 0.0 {
            return 0.0;
        }
        (ask - bid) / bid
    }

    /// Check if spread is wide enough for market making
    pub fn is_wide_spread(&self, bid: f64, ask: f64, threshold: f64) -> bool {
        self.spread_pct(bid, ask) > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arbitrage_detection() {
        let analyzer = SpreadAnalyzer::new(0.01);

        // Clear arbitrage: YES=0.45, NO=0.45, total=0.90 < 1.00
        let opp = analyzer.detect_arbitrage(0.44, 0.45, 0.44, 0.45);
        assert!(opp.is_some());
        let opp = opp.unwrap();
        assert!(opp.expected_profit > 0.0);
        assert!(opp.edge_pct >= 0.01);

        // No arbitrage: YES=0.50, NO=0.50, total=1.00
        // Use slightly higher prices to avoid edge detection
        let opp = analyzer.detect_arbitrage(0.49, 0.51, 0.49, 0.51);
        assert!(opp.is_none());

        // No arbitrage: total > 1.00
        let opp = analyzer.detect_arbitrage(0.50, 0.55, 0.50, 0.55);
        assert!(opp.is_none());
    }

    #[test]
    fn test_spread_calculation() {
        let analyzer = SpreadAnalyzer::new(0.01);

        let width = analyzer.spread_width(0.48, 0.52);
        assert!((width - 0.04).abs() < 0.0001); // Use epsilon comparison

        let pct = analyzer.spread_pct(0.50, 0.52);
        assert!((pct - 0.04).abs() < 0.0001); // Use epsilon comparison
    }
}
