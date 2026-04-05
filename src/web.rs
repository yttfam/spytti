use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::audio;
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
    device: String,
    cover_url: String,
    restarting: bool,
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
        .route("/api/play-pause", post(play_pause))
        .route("/api/next", post(next))
        .route("/api/prev", post(prev))
        .route("/api/devices", get(devices))
        .route("/api/device", post(set_device))
        .route("/api/logs", get(logs))
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
        device: s.device.clone(),
        cover_url: s.cover_url.clone(),
        restarting: s.restarting,
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

async fn play_pause(State(state): State<WebState>) -> StatusCode {
    if state.cmd_tx.send(SpotifyCommand::PlayPause).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn next(State(state): State<WebState>) -> StatusCode {
    if state.cmd_tx.send(SpotifyCommand::Next).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn prev(State(state): State<WebState>) -> StatusCode {
    if state.cmd_tx.send(SpotifyCommand::Prev).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Deserialize)]
struct DeviceRequest {
    device: String,
}

async fn set_device(
    State(state): State<WebState>,
    Json(req): Json<DeviceRequest>,
) -> impl IntoResponse {
    if state.cmd_tx.send(SpotifyCommand::SetDevice(req.device.clone())).await.is_ok() {
        (StatusCode::OK, Json(serde_json::json!({"device": req.device})))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "player unavailable"})))
    }
}

async fn logs(State(state): State<WebState>) -> Json<Vec<String>> {
    Json(state.app.read().await.logs.clone())
}

async fn devices() -> Json<Vec<audio::AudioDevice>> {
    Json(audio::list_devices())
}

async fn health() -> StatusCode {
    StatusCode::OK
}
