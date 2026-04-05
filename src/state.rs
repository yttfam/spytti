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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_sets_initial_volume() {
        let state = new_state(42);
        let s = state.read().await;
        assert_eq!(s.volume, 42);
        assert!(!s.playing);
        assert!(s.track.is_empty());
        assert!(s.artist.is_empty());
        assert!(s.album.is_empty());
    }

    #[tokio::test]
    async fn state_is_writable() {
        let state = new_state(0);
        {
            let mut s = state.write().await;
            s.playing = true;
            s.track = "Test Track".into();
            s.artist = "Test Artist".into();
            s.album = "Test Album".into();
            s.volume = 75;
        }
        let s = state.read().await;
        assert!(s.playing);
        assert_eq!(s.track, "Test Track");
        assert_eq!(s.artist, "Test Artist");
        assert_eq!(s.album, "Test Album");
        assert_eq!(s.volume, 75);
    }

    #[test]
    fn shared_state_default() {
        let s = SharedState::default();
        assert!(!s.playing);
        assert_eq!(s.volume, 0);
        assert!(s.track.is_empty());
    }
}
