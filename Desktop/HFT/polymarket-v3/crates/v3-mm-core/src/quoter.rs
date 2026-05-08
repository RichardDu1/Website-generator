use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Per-market quote state: tracks what we currently have on the book
#[derive(Debug, Clone)]
pub struct QuoteState {
    pub bid_order_id: Option<String>,
    pub ask_order_id: Option<String>,
    pub bid_price: Option<Decimal>,
    pub ask_price: Option<Decimal>,
    pub bid_size: Option<u64>,
    pub ask_size: Option<u64>,
    pub last_fair_p: f64,
    pub last_quote_time: Option<Instant>,
}

impl Default for QuoteState {
    fn default() -> Self {
        Self {
            bid_order_id: None,
            ask_order_id: None,
            bid_price: None,
            ask_price: None,
            bid_size: None,
            ask_size: None,
            last_fair_p: 0.0,
            last_quote_time: None,
        }
    }
}

const MIN_REQUOTE_INTERVAL_SECS: f64 = 2.0;
/// Force requote if order age exceeds this, even if prices haven't moved much.
/// Ensures our orders don't go stale on a quiet market.
const MAX_QUOTE_AGE_SECS: f64 = 10.0;

impl QuoteState {
    /// Should we requote based on fair price movement and time throttle?
    pub fn should_requote(&self, new_fair_p: f64, config: &QuoterConfig) -> bool {
        if self.last_fair_p == 0.0 {
            return true; // never quoted, always quote
        }
        // Time throttle: don't requote more often than every MIN_REQUOTE_INTERVAL_SECS
        let elapsed = match self.last_quote_time {
            Some(t) => t.elapsed().as_secs_f64(),
            None => f64::MAX,
        };
        if elapsed < MIN_REQUOTE_INTERVAL_SECS {
            return false;
        }
        // Force requote if quotes are getting old (even without price move)
        if elapsed > MAX_QUOTE_AGE_SECS {
            return true;
        }
        // Only requote if fair_p moved enough that the tick-rounded price
        // would actually change. Since tick = 0.01, we need fair_p to move
        // by at least 1c to change the output price. Use a threshold of 1.5c
        // to avoid flip-flopping at tick boundaries.
        (new_fair_p - self.last_fair_p).abs() > config.requote_threshold.max(0.015)
    }
}

/// A single quote to place
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub token_id: String,
    pub side: Side,
    pub price: Decimal,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// Quoter: generates bid/ask quotes around fair probability
pub struct Quoter {
    pub config: QuoterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoterConfig {
    pub base_spread: f64,
    pub inventory_skew: f64,
    pub min_edge: f64,
    pub size_per_quote: u64,
    pub requote_threshold: f64,
    pub fair_p_cutoff: f64, // skip quoting when fair_p < cutoff or > 1-cutoff
    pub rewards_max_spread: f64, // max spread to qualify for liquidity rewards (full width)
}

impl Default for QuoterConfig {
    fn default() -> Self {
        Self {
            base_spread: 0.03,
            inventory_skew: 0.005,
            min_edge: 0.01,
            size_per_quote: 50,
            requote_threshold: 0.005,
            fair_p_cutoff: 0.08,
            rewards_max_spread: 0.045, // 4.5c for crypto_15_min markets
        }
    }
}

impl Quoter {
    pub fn new(config: QuoterConfig) -> Self {
        Self { config }
    }

    /// Hard cap on inventory per market (in shares). Beyond this, we only
    /// allow inventory-reducing quotes. 2x the typical order size.
    /// With SIZE_PER_QUOTE=20, this caps at 40 shares per market.
    const INVENTORY_HARD_CAP_MULT: i64 = 2;

    /// Generate bid/ask quotes for a market (book-aware, rewards-optimized).
    ///
    /// Strategy: maximize rewards score S = ((v-s)/v)^2 * b by quoting as
    /// tight as possible while maintaining min_edge safety and staying within
    /// rewardsMaxSpread. When book data is available, join the best bid/ask
    /// to get the tightest spread. Fall back to base_spread when no book.
    ///
    /// `fair_p` - fair probability of YES outcome (0.01..0.99)
    /// `inventory` - net YES shares held (positive = long YES)
    /// `remaining_secs` - seconds until window closes
    /// `vol` - annualized volatility
    /// `token_id_yes` - the YES token ID for this market
    /// `token_id_no` - the NO token ID (ask is placed as BUY NO)
    /// `yes_book` - (best_bid, best_ask) for YES token, (0,0) if unknown
    /// `no_book` - (best_bid, best_ask) for NO token, (0,0) if unknown
    pub fn generate_quotes(
        &self,
        fair_p: f64,
        inventory: i64,
        remaining_secs: f64,
        vol: f64,
        token_id_yes: &str,
        token_id_no: &str,
        _yes_book: (f64, f64),
        _no_book: (f64, f64),
    ) -> (Option<Quote>, Option<Quote>) {
        // Don't quote if too close to expiry
        if remaining_secs < 60.0 {
            return (None, None);
        }

        // Don't quote at extreme probabilities (nearly certain outcome)
        if fair_p < self.config.fair_p_cutoff || fair_p > (1.0 - self.config.fair_p_cutoff) {
            return (None, None);
        }

        // Hard inventory cap: if |inventory| exceeds INVENTORY_HARD_CAP_MULT × size,
        // only allow inventory-reducing quotes.
        let inv_cap = (self.config.size_per_quote as i64) * Self::INVENTORY_HARD_CAP_MULT;
        let hit_long_cap = inventory >= inv_cap; // too long, don't buy more
        let hit_short_cap = inventory <= -inv_cap; // too short, don't sell more

        // 1. Inventory skew: push quotes to reduce position
        let skew = self.config.inventory_skew * (inventory as f64);

        // 2. Compute ideal bid/ask — try to be as tight as possible
        //    Priority: join book > min_edge from fair > base_spread fallback
        //    When inventory is elevated, widen min_edge on the INCREASING side
        //    to reduce adverse selection risk. Each SIZE_PER_QUOTE of inventory adds 1c.
        let inv_ratio = (inventory.abs() as f64) / (self.config.size_per_quote as f64);
        let inv_edge_bump = (inv_ratio * 0.01).min(0.05); // max 5c extra edge
                                                          // Apply bump only to the side that would INCREASE inventory
        let bid_min_edge = if inventory >= 0 {
            self.config.min_edge + inv_edge_bump // long: buying more is risky
        } else {
            self.config.min_edge // short: buying reduces inventory, keep tight
        };
        let ask_min_edge = if inventory <= 0 {
            self.config.min_edge + inv_edge_bump // short: selling more is risky
        } else {
            self.config.min_edge // long: selling reduces inventory, keep tight
        };
        let min_edge = self.config.min_edge; // base min_edge for spread calculations
        let half_max_reward_spread = self.config.rewards_max_spread / 2.0;

        // Fallback half-spread: base_spread adjusted by time and vol
        let time_factor = (900.0 / remaining_secs.max(60.0)).sqrt();
        let vol_factor = (vol / 0.50).clamp(0.5, 3.0);
        let fallback_half = self.config.base_spread * time_factor * vol_factor;

        // 3. Determine bid price.
        //    In these thin 15-min markets, we are often the sole MM so book top
        //    is just our own previous quotes. Use fair_p-based pricing exclusively.
        //    Bid = fair_p - half_spread - skew, clamped to rewards range.
        let raw_half_bid = fallback_half.max(min_edge);
        let bid_half = raw_half_bid.min(half_max_reward_spread); // rewards cap
        let bid_price = fair_p - bid_half - skew;

        // 4. Determine ask price (in YES terms).
        let raw_half_ask = fallback_half.max(min_edge);
        let ask_half = raw_half_ask.min(half_max_reward_spread); // rewards cap
        let ask_price = fair_p + ask_half - skew;

        // 5. Ensure total spread stays within rewards_max_spread
        //    Account for tick rounding which can add up to 2c (1c each side)
        let total_spread = ask_price - bid_price;
        let target_half = (self.config.rewards_max_spread - 0.01) / 2.0; // 1c buffer for rounding
        let (bid_adj, ask_adj) = if total_spread > self.config.rewards_max_spread {
            let mid = (bid_price + ask_price) / 2.0;
            (mid - target_half, mid + target_half)
        } else {
            (bid_price, ask_price)
        };

        // 6. No client-side anti-crossing check.
        //    In these thin 15-min markets we are often the sole MM. The REST
        //    book top data is delayed and frequently reflects our own previous
        //    quotes or stale counterparty orders. Client-side crossing checks
        //    based on this data cause excessive suppression (Bug 17):
        //    - Bug 16 original: clamping to best_ask-0.01 created dangerous prices
        //    - Bug 16 fix v1: suppressing entirely caused single-sided quoting
        //    Instead, we let Polymarket's CLOB reject with "crosses book" if our
        //    order truly crosses. The orchestrator already handles that error
        //    gracefully (logs [CROSS] warning, does not backoff).
        let bid_final = bid_adj;
        let ask_final = ask_adj;

        // 7. Round to tick (0.01) and clamp
        let bid_dec = tick_floor(bid_final);
        let ask_dec = tick_ceil(ask_final);

        // Polymarket minimum order size is 5 shares, minimum notional is $1.
        const POLY_MIN_SIZE: u64 = 5;
        let min_size_for = |price: Decimal| -> u64 {
            let p = f64_from_decimal(price);
            if p <= 0.0 {
                return self.config.size_per_quote;
            }
            let notional_min = (1.0 / p).ceil() as u64;
            let min = notional_min.max(POLY_MIN_SIZE);
            let max_size = self.config.size_per_quote * 2;
            self.config.size_per_quote.max(min).min(max_size)
        };

        // 8. Min edge check — only emit if we have enough edge
        //    Also enforce hard inventory cap: suppress inventory-increasing side.
        //    Use per-side min_edge: inventory-increasing side has wider edge requirement.
        let bid = if hit_long_cap {
            None // at long cap, don't buy more YES
        } else if fair_p - f64_from_decimal(bid_dec) >= bid_min_edge && bid_dec > Decimal::ZERO {
            let bp = bid_dec.min(Decimal::new(99, 2)).max(Decimal::new(1, 2));
            Some(Quote {
                token_id: token_id_yes.to_string(),
                side: Side::Buy,
                price: bp,
                size: min_size_for(bp),
            })
        } else {
            None
        };

        let ask = if hit_short_cap {
            None // at short cap, don't sell more YES
        } else if f64_from_decimal(ask_dec) - fair_p >= ask_min_edge && ask_dec < Decimal::ONE {
            // On Polymarket, "sell YES" is expressed as "BUY NO @ (1 - ask_price)"
            let no_price = (Decimal::ONE - ask_dec).min(Decimal::new(99, 2)).max(Decimal::new(1, 2));
            Some(Quote {
                token_id: token_id_no.to_string(),
                side: Side::Buy,
                price: no_price,
                size: min_size_for(no_price),
            })
        } else {
            None
        };

        (bid, ask)
    }
}

fn tick_floor(val: f64) -> Decimal {
    let ticks = (val * 100.0).floor() as i64;
    Decimal::new(ticks, 2)
}

fn tick_ceil(val: f64) -> Decimal {
    let ticks = (val * 100.0).ceil() as i64;
    Decimal::new(ticks, 2)
}

fn dec(val: f64) -> Decimal {
    Decimal::from_f64_retain(val).unwrap_or(Decimal::ZERO)
}

fn f64_from_decimal(d: Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    d.to_f64().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_quoter() -> Quoter {
        Quoter::new(QuoterConfig::default())
    }

    // No book data = (0,0)
    const NO_BOOK: (f64, f64) = (0.0, 0.0);

    #[test]
    fn test_symmetric_at_50() {
        let q = make_quoter();
        let (bid, ask) = q.generate_quotes(
            0.50,
            0,
            450.0,
            0.50,
            "token_yes",
            "token_no",
            NO_BOOK,
            NO_BOOK,
        );
        let bid = bid.expect("should have bid");
        let ask = ask.expect("should have ask");
        let ask_yes_price = 1.0 - f64_from_decimal(ask.price);
        let mid = (f64_from_decimal(bid.price) + ask_yes_price) / 2.0;
        assert!((mid - 0.50).abs() < 0.02, "mid={mid}");
        assert_eq!(ask.side, Side::Buy);
        assert_eq!(ask.token_id, "token_no");
    }

    #[test]
    fn test_inventory_skew_long() {
        let q = make_quoter();
        let (bid, ask) = q.generate_quotes(0.50, 3, 450.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        let (bid0, ask0) = q.generate_quotes(0.50, 0, 450.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        let bid_p = f64_from_decimal(bid.unwrap().price);
        let ask_no_p = f64_from_decimal(ask.unwrap().price);
        let bid0_p = f64_from_decimal(bid0.unwrap().price);
        let ask0_no_p = f64_from_decimal(ask0.unwrap().price);
        assert!(bid_p < bid0_p, "long inventory should lower bid");
        assert!(
            ask_no_p > ask0_no_p,
            "long inventory should raise NO price (lower YES ask)"
        );
    }

    #[test]
    fn test_no_quote_near_expiry() {
        let q = make_quoter();
        let (bid, ask) = q.generate_quotes(0.50, 0, 30.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        assert!(bid.is_none());
        assert!(ask.is_none());
    }

    #[test]
    fn test_no_quote_extreme_fair_p() {
        let q = make_quoter();
        let (bid, ask) = q.generate_quotes(0.99, 0, 450.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        assert!(bid.is_none());
        assert!(ask.is_none());
        let (bid, ask) = q.generate_quotes(0.01, 0, 450.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        assert!(bid.is_none());
        assert!(ask.is_none());
        let (bid, _) = q.generate_quotes(0.10, 0, 450.0, 0.30, "t", "tn", NO_BOOK, NO_BOOK);
        assert!(bid.is_some(), "fair_p=0.10 should produce bid");
    }

    #[test]
    fn test_no_client_side_crossing_check() {
        let q = make_quoter();
        // Bug 17 fix: no client-side crossing check. Let exchange reject.
        // fair=0.70 with a stale YES best_ask = 0.52 → bid should STILL be generated
        // (exchange will reject with "crosses book" if it truly crosses)
        let yes_book = (0.48, 0.52);
        let no_book = (0.48, 0.52);
        let (bid, _) = q.generate_quotes(0.70, 0, 450.0, 0.30, "t", "tn", yes_book, no_book);
        let bp = f64_from_decimal(
            bid.expect("bid should exist, let exchange handle crossing")
                .price,
        );
        assert!(bp >= 0.66, "bid {bp} should be near fair 0.70");

        // Without book data, same behavior
        let (bid2, _) = q.generate_quotes(0.70, 0, 450.0, 0.30, "t", "tn", NO_BOOK, NO_BOOK);
        let bp2 = f64_from_decimal(bid2.expect("should have bid").price);
        assert!(
            bp2 >= 0.66,
            "without book, bid {bp2} should be near fair 0.70"
        );
    }

    #[test]
    fn test_rewards_spread_capping() {
        let q = make_quoter();
        // With high vol, fallback spread would be wide, but rewards_max_spread caps it
        let (bid, ask) = q.generate_quotes(0.50, 0, 450.0, 1.50, "t", "tn", NO_BOOK, NO_BOOK);
        let bid = bid.expect("should have bid");
        let ask = ask.expect("should have ask");
        let bid_p = f64_from_decimal(bid.price);
        let ask_yes_p = 1.0 - f64_from_decimal(ask.price);
        let spread = ask_yes_p - bid_p;
        // After tick rounding, spread may be up to rewards_max_spread + 2c
        assert!(
            spread <= 0.065,
            "spread {spread} should be capped near rewards_max_spread 4.5c"
        );
        // Without capping, vol=1.5 would give spread ~18c, so verify it's much tighter
        assert!(
            spread >= 0.02,
            "spread {spread} should still have some width"
        );
    }

    #[test]
    fn test_fair_p_based_pricing() {
        let q = make_quoter();
        // With fair_p=0.80, bid should be ~0.78 and ask ~0.82
        let (bid, ask) = q.generate_quotes(0.80, 0, 450.0, 0.50, "t", "tn", NO_BOOK, NO_BOOK);
        let bid = bid.expect("should have bid at fair=0.80");
        let ask = ask.expect("should have ask at fair=0.80");
        let bid_p = f64_from_decimal(bid.price);
        let ask_yes_p = 1.0 - f64_from_decimal(ask.price);
        assert!(bid_p >= 0.75, "bid {bid_p} should be near fair 0.80");
        assert!(bid_p <= 0.80, "bid {bid_p} should be below fair 0.80");
        assert!(
            ask_yes_p >= 0.80,
            "ask {ask_yes_p} should be above fair 0.80"
        );
        assert!(
            ask_yes_p <= 0.85,
            "ask {ask_yes_p} should be near fair 0.80"
        );
    }
}
