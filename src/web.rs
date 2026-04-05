use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::spotify::SpotifyCommand;
use crate::state::AppState;

#[derive(Clone)]
pub struct WebState {
    pub app: AppState,
    pub cmd_tx: mpsc::Sender<SpotifyCommand>,
}

#[derive(Serialize)]
struct StatusResponse {
    playing: bool,
    track: String,
    artist: String,
    album: String,
    volume: u16,
}

#[derive(Deserialize)]
struct VolumeRequest {
    volume: u16,
}

pub fn router(state: WebState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/status", get(status))
        .route("/api/volume", post(set_volume))
        .route("/api/health", get(health))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("ui.html"))
}

async fn status(State(state): State<WebState>) -> Json<StatusResponse> {
    let s = state.app.read().await;
    Json(StatusResponse {
        playing: s.playing,
        track: s.track.clone(),
        artist: s.artist.clone(),
        album: s.album.clone(),
        volume: s.volume,
    })
}

async fn set_volume(
    State(state): State<WebState>,
    Json(req): Json<VolumeRequest>,
) -> impl IntoResponse {
    let vol = req.volume.min(100);
    if state.cmd_tx.send(SpotifyCommand::SetVolume(vol)).await.is_ok() {
        (StatusCode::OK, Json(serde_json::json!({"volume": vol})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "player unavailable"})))
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}
