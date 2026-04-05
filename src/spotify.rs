use crate::config::Config;
use crate::state::AppState;
use librespot::core::cache::Cache;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use librespot::connect::ConnectConfig;
use librespot::discovery::Discovery;
use librespot::core::config::DeviceType;
use librespot::metadata::{Metadata, Track};
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, Bitrate, PlayerConfig};
use librespot::playback::mixer::{self, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent};
use librespot::connect::Spirc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

pub enum SpotifyCommand {
    SetVolume(u16),
}

pub async fn run(
    config: Config,
    state: AppState,
    mut cmd_rx: mpsc::Receiver<SpotifyCommand>,
) {
    loop {
        if let Err(e) = run_inner(&config, &state, &mut cmd_rx).await {
            error!("Spotify session error: {e}, restarting in 5s...");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

async fn run_inner(
    config: &Config,
    state: &AppState,
    cmd_rx: &mut mpsc::Receiver<SpotifyCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session_config = SessionConfig::default();
    let device_id = session_config.device_id.clone();

    let cache = Cache::new(
        Some(config.cache.clone()),
        None,
        None,
        None,
    )?;

    // Try cached credentials first, fall back to zeroconf discovery
    let credentials = if let Some(creds) = cache.credentials() {
        info!("Using cached credentials");
        creds
    } else {
        info!("No cached credentials, starting Zeroconf discovery as '{}'", config.name);
        let client_id = SessionConfig::default().client_id;
        let mut discovery = Discovery::builder(device_id.clone(), client_id)
            .name(config.name.clone())
            .device_type(DeviceType::Speaker)
            .launch()?;

        use futures_util::StreamExt;
        let creds = discovery.next().await
            .ok_or("Discovery stream ended without credentials")?;
        info!("Received credentials via Zeroconf");
        creds
    };

    // Create session (Spirc::new will connect it)
    let session = Session::new(session_config, Some(cache));

    // Mixer
    let mixer_config = MixerConfig::default();
    let mixer_fn = mixer::find(Some("softvol")).expect("No softmixer available");
    let mixer = mixer_fn(mixer_config)?;

    // Player
    let player_config = PlayerConfig {
        bitrate: match config.bitrate {
            96 => Bitrate::Bitrate96,
            160 => Bitrate::Bitrate160,
            _ => Bitrate::Bitrate320,
        },
        ..Default::default()
    };

    let device = if config.device == "auto" {
        None
    } else {
        Some(config.device.clone())
    };

    let volume_getter = mixer.get_soft_volume();
    let player = Player::new(
        player_config,
        session.clone(),
        volume_getter,
        move || {
            let backend = audio_backend::find(None).expect("No audio backend");
            backend(device, AudioFormat::default())
        },
    );

    let mut event_rx = player.get_player_event_channel();

    // Spirc (Spotify Connect)
    let connect_config = ConnectConfig {
        name: config.name.clone(),
        device_type: DeviceType::Speaker,
        initial_volume: (config.initial_volume as u16).min(100) * 655,
        ..Default::default()
    };

    {
        let mut s = state.write().await;
        s.volume = config.initial_volume;
    }

    let (spirc, spirc_task) = Spirc::new(
        connect_config,
        session.clone(),
        credentials,
        player,
        mixer,
    ).await?;

    info!("Spirc started, device '{}' visible on Spotify Connect", config.name);

    let spirc_handle = tokio::spawn(spirc_task);

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(PlayerEvent::Playing { track_id, .. }) => {
                        match Track::get(&session, &track_id).await {
                            Ok(track) => {
                                let artist_name = track.artists.0.first()
                                    .map(|a| a.name.clone())
                                    .unwrap_or_default();
                                let mut s = state.write().await;
                                s.playing = true;
                                s.track = track.name;
                                s.artist = artist_name;
                                s.album = track.album.name;
                                info!("Playing: {} - {}", s.artist, s.track);
                            }
                            Err(e) => warn!("Failed to fetch track metadata: {e}"),
                        }
                    }
                    Some(PlayerEvent::Paused { .. }) => {
                        state.write().await.playing = false;
                    }
                    Some(PlayerEvent::Stopped { .. }) => {
                        let mut s = state.write().await;
                        s.playing = false;
                        s.track.clear();
                        s.artist.clear();
                        s.album.clear();
                    }
                    Some(PlayerEvent::VolumeChanged { volume }) => {
                        let vol_pct = (volume as u32 * 100 / 65535) as u16;
                        state.write().await.volume = vol_pct;
                    }
                    None => {
                        warn!("Player event channel closed");
                        break;
                    }
                    _ => {}
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SpotifyCommand::SetVolume(vol)) => {
                        let vol_raw = (vol.min(100) as u32 * 65535 / 100) as u16;
                        let _ = spirc.set_volume(vol_raw);
                        state.write().await.volume = vol;
                    }
                    None => {
                        info!("Command channel closed, shutting down");
                        break;
                    }
                }
            }
        }
    }

    let _ = spirc.shutdown();
    spirc_handle.abort();
    Ok(())
}
