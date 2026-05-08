//! Dashboard HTTP server with WebSocket support

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::state::{DashboardState, EventLog};

/// Shared dashboard state
#[derive(Clone)]
pub struct DashboardServer {
    state: Arc<RwLock<DashboardState>>,
    events: Arc<RwLock<EventLog>>,
}

impl DashboardServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(DashboardState::default())),
            events: Arc::new(RwLock::new(EventLog::new())),
        }
    }

    /// Update dashboard state
    pub async fn update_state(&self, state: DashboardState) {
        let mut s = self.state.write().await;
        *s = state;
    }

    /// Add event to log
    pub async fn add_event(&self, kind: String, message: String) {
        let mut events = self.events.write().await;
        events.add(kind, message);
    }

    /// Get current state
    pub async fn get_state(&self) -> DashboardState {
        self.state.read().await.clone()
    }

    /// Get events
    pub async fn get_events(&self) -> EventLog {
        self.events.read().await.clone()
    }
}

/// Serve dashboard on specified port
pub async fn serve(server: Arc<DashboardServer>, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .with_state((*server).clone());

    let addr = format!("0.0.0.0:{}", port);
    info!("Dashboard server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve index.html
async fn index_handler() -> impl IntoResponse {
    let html = include_str!("../static/index.html");
    Html(html)
}

/// WebSocket handler
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(server): State<DashboardServer>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, server))
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket, server: DashboardServer) {
    debug!("WebSocket client connected");

    // Send initial state
    let state = server.get_state().await;
    let events = server.get_events().await;

    let full_state = serde_json::json!({
        "state": state,
        "events": events,
    });

    if let Ok(msg) = serde_json::to_string(&full_state) {
        if let Err(e) = socket.send(axum::extract::ws::Message::Text(msg)).await {
            error!("Failed to send initial state: {}", e);
            return;
        }
    }

    // Update loop - send state every second
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    loop {
        interval.tick().await;

        let state = server.get_state().await;
        let events = server.get_events().await;

        let full_state = serde_json::json!({
            "state": state,
            "events": events,
        });

        match serde_json::to_string(&full_state) {
            Ok(msg) => {
                if let Err(e) = socket.send(axum::extract::ws::Message::Text(msg)).await {
                    debug!("WebSocket send error (client likely disconnected): {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("Failed to serialize state: {}", e);
                break;
            }
        }
    }

    debug!("WebSocket client disconnected");
}
