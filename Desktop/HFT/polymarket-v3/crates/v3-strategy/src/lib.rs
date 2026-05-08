pub mod strategy_trait;
pub mod vol_tracker;
pub mod gengar;

pub use strategy_trait::{Strategy, StrategyDecision, ExitReason};
pub use vol_tracker::VolTracker;
pub use gengar::{GengarConfig, Side as GengarSide, TradeSignal, estimate_true_probability, kelly_bet_size, evaluate, should_lock_profit, should_force_close_by_wallclock};
