use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_LOG_LINES: usize = 200;

#[derive(Clone, Default)]
pub struct SharedState {
    pub playing: bool,
    pub track: String,
    pub artist: String,
    pub album: String,
    pub cover_url: String,
    pub volume: u16,
    pub device: String,
    pub logs: Vec<String>,
}

impl SharedState {
    pub fn push_log(&mut self, line: String) {
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.remove(0);
        }
        self.logs.push(line);
    }
}

pub type AppState = Arc<RwLock<SharedState>>;

pub fn new_state(initial_volume: u16, device: &str) -> AppState {
    Arc::new(RwLock::new(SharedState {
        volume: initial_volume,
        device: device.to_string(),
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_state_sets_initial_volume() {
        let state = new_state(42, "auto");
        let s = state.read().await;
        assert_eq!(s.volume, 42);
        assert!(!s.playing);
        assert!(s.track.is_empty());
        assert!(s.artist.is_empty());
        assert!(s.album.is_empty());
        assert_eq!(s.device, "auto");
    }

    #[tokio::test]
    async fn new_state_sets_device() {
        let state = new_state(30, "hw:CARD=0,DEV=0");
        let s = state.read().await;
        assert_eq!(s.device, "hw:CARD=0,DEV=0");
    }

    #[tokio::test]
    async fn state_is_writable() {
        let state = new_state(0, "auto");
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
