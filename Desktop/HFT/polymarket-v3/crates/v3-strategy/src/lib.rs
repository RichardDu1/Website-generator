pub mod strategy_trait;
pub mod momentum;
pub mod market_making;
pub mod spread_capture;
pub mod selector;

pub use strategy_trait::{Strategy, StrategyDecision};
pub use momentum::MomentumStrategy;
pub use market_making::MarketMakingStrategy;
pub use spread_capture::SpreadCaptureStrategy;
pub use selector::StrategySelector;
