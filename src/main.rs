mod config;
mod spotify;
mod state;
mod web;

use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = config::Config::load();
    let app_state = state::new_state(config.initial_volume);

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(32);

    let web_state = web::WebState {
        app: app_state.clone(),
        cmd_tx,
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
