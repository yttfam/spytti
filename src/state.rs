use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct SharedState {
    pub playing: bool,
    pub track: String,
    pub artist: String,
    pub album: String,
    pub volume: u16,
}

pub type AppState = Arc<RwLock<SharedState>>;

pub fn new_state(initial_volume: u16) -> AppState {
    Arc::new(RwLock::new(SharedState {
        volume: initial_volume,
        ..Default::default()
    }))
}
