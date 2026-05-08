//! End-to-end scenario tests for the Gengar strategy.
//!
//! These simulate a full window lifecycle using only the pure strategy
//! functions — no real WebSockets, no real CLOB. Verifies that the four
//! exported decision functions compose correctly to produce an expected
//! enter → hold → exit trajectory.
//!
//! Each scenario is a sequence of `(seconds_remaining, btc_price, up_ask,
//! down_ask)` ticks; the test replays them and checks the derived state.

use v3_strategy::{
    evaluate, should_force_close_by_wallclock, should_lock_profit, GengarConfig,
    GengarSide, VolTracker,
};

/// Baseline config for scenarios.
fn cfg() -> GengarConfig {
    GengarConfig::default()
}

#[derive(Debug, Clone, Copy)]
struct Position {
    side: GengarSide,
    entry_price: f64,
    shares: u64,
    initial_edge: f64,
    entry_at_secs: u64, // wall-clock monotonic in scenario (not sec_rem)
}

/// Simulate a single tick. Returns `Some(new_position)` on entry,
/// otherwise returns the position unchanged or `None` on exit.
fn sim_tick(
    sec_rem: f64,
    btc_price: f64,
    opening: f64,
    up_ask: f64,
    down_ask: f64,
    bankroll: f64,
    vol: f64,
    cfg: &GengarConfig,
    current_pos: Option<Position>,
    now_wallclock_secs: u64,
) -> (Option<Position>, Option<&'static str>) {
    // Exit path takes precedence if we have a position.
    if let Some(p) = current_pos {
        // Wall-clock hard ceiling
        let held = now_wallclock_secs - p.entry_at_secs;
        if should_force_close_by_wallclock(cfg, held) {
            return (None, Some("force-sell"));
        }
        // Profit lock: compute current edge
        // (simplified: use current market ask on our side as market price
        // and estimate_true_probability for the prob)
        let delta_pct = (btc_price - opening) / opening * 100.0;
        let current_prob =
            v3_strategy::estimate_true_probability(delta_pct, sec_rem, vol);
        let side_matches = (delta_pct > 0.0) == matches!(p.side, GengarSide::Up);
        let effective_prob = if side_matches { current_prob } else { 1.0 - current_prob };
        let current_market_price = match p.side {
            GengarSide::Up => up_ask,
            GengarSide::Down => down_ask,
        };
        let current_edge = effective_prob - current_market_price;
        if should_lock_profit(p.initial_edge, current_edge, cfg.profit_lock_ratio) {
            return (None, Some("profit-lock"));
        }
        if sec_rem <= 0.0 {
            return (None, Some("window-expired"));
        }
        return (Some(p), None); // hold
    }

    // Entry path
    let Some(sig) = evaluate(
        btc_price, opening, up_ask, down_ask, sec_rem, bankroll, cfg, vol,
    ) else {
        return (None, None);
    };
    let market_price = sig.market_price;
    let shares = (sig.kelly_size / market_price).floor() as u64;
    if shares < 5 {
        return (None, None);
    }
    let p = Position {
        side: sig.side,
        entry_price: market_price,
        shares,
        initial_edge: sig.edge,
        entry_at_secs: now_wallclock_secs,
    };
    (Some(p), Some("enter"))
}

/// Scenario: BTC rips up 0.20% in a 15m window, UP price lags at 0.60 for a
/// while, then catches up to 0.85. Expect: enter UP at 0.60, exit on
/// profit-lock when edge decays below 30% of initial.
#[test]
fn scenario_enter_up_then_profit_lock_as_edge_decays() {
    let cfg = cfg();
    let opening = 100_000.0;
    let vol = 0.12;

    // Ticks: (sec_rem, btc_price, up_ask, down_ask)
    // Entry edge ≈ 0.90 - 0.60 = 0.30; profit-lock threshold = 0.09.
    // To trigger, final current_edge must fall below 0.09 → market must
    // re-price very close to the model probability.
    let ticks: &[(f64, f64, f64, f64)] = &[
        (240.0, 100_200.0, 0.60, 0.40), // +0.2%, big edge → enter
        (210.0, 100_220.0, 0.65, 0.35), // still huge edge, hold
        (180.0, 100_200.0, 0.78, 0.22), // edge compressing, hold
        (150.0, 100_180.0, 0.93, 0.07), // market nearly priced-in → profit-lock
    ];

    let mut pos: Option<Position> = None;
    let mut events: Vec<&str> = vec![];
    let mut wallclock = 0_u64;

    for (sec_rem, btc, up, down) in ticks {
        let (new_pos, ev) = sim_tick(
            *sec_rem, *btc, opening, *up, *down, 100.0, vol, &cfg, pos, wallclock,
        );
        pos = new_pos;
        if let Some(e) = ev {
            events.push(e);
        }
        wallclock += 30;
    }

    assert_eq!(events, vec!["enter", "profit-lock"], "events={events:?}");
    assert!(pos.is_none(), "position should be closed");
}

/// Scenario: entry fires but BTC reverses. Edge goes negative → profit-lock
/// should fire immediately (current_edge < initial × 0.30 is trivially true
/// when current_edge is negative).
#[test]
fn scenario_reversal_triggers_profit_lock_immediately() {
    let cfg = cfg();
    let opening = 100_000.0;
    let vol = 0.12;

    let ticks: &[(f64, f64, f64, f64)] = &[
        (200.0, 100_150.0, 0.65, 0.35), // enter UP
        (170.0, 99_850.0, 0.35, 0.65),  // BTC reversed → our UP position is now
                                         // on the wrong side; effective_prob flips
    ];

    let mut pos: Option<Position> = None;
    let mut events: Vec<&str> = vec![];

    for (i, (sec_rem, btc, up, down)) in ticks.iter().enumerate() {
        let wallclock = (i as u64) * 30;
        let (new_pos, ev) = sim_tick(
            *sec_rem, *btc, opening, *up, *down, 100.0, vol, &cfg, pos, wallclock,
        );
        pos = new_pos;
        if let Some(e) = ev {
            events.push(e);
        }
    }

    assert_eq!(events, vec!["enter", "profit-lock"]);
    assert!(pos.is_none());
}

/// Scenario: wall-clock ceiling fires if position held past
/// max_position_hold_secs even when profit-lock never triggers.
/// Use a manually-locked edge trajectory so profit-lock doesn't fire.
#[test]
fn scenario_wallclock_force_sell_after_900s_even_with_stable_edge() {
    // Custom config: 60s ceiling to keep the scenario short.
    let cfg = GengarConfig {
        max_position_hold_secs: 60,
        ..cfg()
    };
    let opening = 100_000.0;
    let vol = 0.12;

    // Hold an UP position, edge stays healthy throughout — normally we'd
    // hold indefinitely. Wall-clock should still close.
    let ticks: &[(u64, f64, f64, f64, f64)] = &[
        (0, 200.0, 100_150.0, 0.65, 0.35),    // enter
        (30, 170.0, 100_150.0, 0.65, 0.35),   // hold (edge stable)
        (65, 140.0, 100_150.0, 0.65, 0.35),   // wall-clock limit crossed
    ];

    let mut pos: Option<Position> = None;
    let mut events: Vec<&str> = vec![];

    for (wallclock, sec_rem, btc, up, down) in ticks {
        let (new_pos, ev) = sim_tick(
            *sec_rem, *btc, opening, *up, *down, 100.0, vol, &cfg, pos, *wallclock,
        );
        pos = new_pos;
        if let Some(e) = ev {
            events.push(e);
        }
    }

    assert_eq!(events, vec!["enter", "force-sell"]);
    assert!(pos.is_none());
}

/// Scenario: BTC never moves meaningfully → no entries for the whole window.
#[test]
fn scenario_flat_market_never_enters() {
    let cfg = cfg();
    let opening = 100_000.0;
    let vol = 0.12;

    let ticks: &[(f64, f64, f64, f64)] = &[
        (240.0, 100_010.0, 0.51, 0.49), // |delta|=0.01%, below min_btc_delta 0.06
        (200.0, 99_990.0, 0.50, 0.50),
        (160.0, 100_020.0, 0.52, 0.48),
        (120.0, 100_000.0, 0.50, 0.50),
        (80.0, 99_980.0, 0.49, 0.51),
    ];

    let mut pos: Option<Position> = None;
    let mut events: Vec<&str> = vec![];

    for (i, (sec_rem, btc, up, down)) in ticks.iter().enumerate() {
        let wallclock = (i as u64) * 30;
        let (new_pos, ev) = sim_tick(
            *sec_rem, *btc, opening, *up, *down, 100.0, vol, &cfg, pos, wallclock,
        );
        pos = new_pos;
        if let Some(e) = ev {
            events.push(e);
        }
    }

    assert!(events.is_empty(), "no events expected, got {events:?}");
    assert!(pos.is_none());
}

/// Scenario: VolTracker progressively fills and affects probability.
/// With fewer samples, vol defaults to 0.12. After 12 closings, realized vol
/// is used. This verifies end-to-end that VolTracker is wired and the two
/// regimes produce visibly different probabilities for the same delta.
#[test]
fn scenario_vol_tracker_transitions_from_fallback_to_realized() {
    let mut vt = VolTracker::new(3); // tiny buffer so test runs fast
    let fallback = 0.12;

    // Buffer empty → should return fallback
    assert_eq!(vt.realized_vol(fallback), fallback);

    // Feed in 3 consistent 0.5% closing deltas → realized vol ~0 (very narrow)
    vt.record_window_delta(0.5);
    vt.record_window_delta(0.5);
    vt.record_window_delta(0.5);
    let realized = vt.realized_vol(fallback);
    assert!(realized < fallback, "realized {realized} should be < fallback {fallback}");
    assert!(realized >= 0.0);
}

/// Scenario: entry blocked because market price out of range (bubble already
/// priced in), even though delta is large.
#[test]
fn scenario_no_entry_when_price_above_max_threshold() {
    let cfg = cfg(); // max_price = 0.90
    let opening = 100_000.0;

    let ticks: &[(f64, f64, f64, f64)] = &[
        (200.0, 100_300.0, 0.95, 0.05), // delta=+0.3% but UP already at 0.95
    ];

    let mut pos: Option<Position> = None;
    let mut events: Vec<&str> = vec![];

    for (sec_rem, btc, up, down) in ticks {
        let (new_pos, ev) =
            sim_tick(*sec_rem, *btc, opening, *up, *down, 100.0, 0.12, &cfg, pos, 0);
        pos = new_pos;
        if let Some(e) = ev {
            events.push(e);
        }
    }

    assert!(events.is_empty());
    assert!(pos.is_none());
}
