mod audio;
mod config;
mod logs;
mod spotify;
mod state;
mod web;

use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let log_buffer = logs::new_buffer();

    // Default to info, but crank librespot_discovery to debug so we can see
    // every Zeroconf HTTP request (including failed auth attempts).
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,librespot_discovery=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(logs::CaptureLayer::new(log_buffer.clone()))
        .init();

    let config = config::Config::load();
    let app_state = state::new_state(config.initial_volume, &config.device);

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(32);

    let web_state = web::WebState {
        app: app_state.clone(),
        cmd_tx,
        log_buffer: log_buffer.clone(),
    };

    let port = config.port;
    let web_handle = tokio::spawn(async move {
        let app = web::router(web_state);
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .expect("Failed to bind port");
        info!("Web server listening on 0.0.0.0:{port}");
        axum::serve(listener, app).await.expect("Web server died");
    });

    let spotify_handle = tokio::spawn(spotify::run(config, app_state, cmd_rx));

    tokio::select! {
        _ = web_handle => info!("Web server exited"),
        _ = spotify_handle => info!("Spotify task exited"),
        _ = tokio::signal::ctrl_c() => info!("Shutting down"),
    }
}
