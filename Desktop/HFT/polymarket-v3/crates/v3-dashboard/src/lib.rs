//! V3 Dashboard - Real-time monitoring UI

pub mod server;
pub mod state;

pub use server::{serve, DashboardServer};
pub use state::{DashboardEvent, DashboardState, EventLog, OrderSnapshot, PositionSnapshot};
