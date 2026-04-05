use crate::config::Config;
use crate::state::AppState;
use librespot::core::cache::Cache;
use librespot::core::config::SessionConfig;
use librespot::core::session::Session;
use librespot::core::authentication::Credentials;
use librespot::connect::ConnectConfig;
use librespot::discovery::Discovery;
use librespot::core::config::DeviceType;
use librespot::metadata::{Metadata, Track, Album};
use librespot::metadata::image::ImageSize;
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, Bitrate, PlayerConfig};
use librespot::playback::mixer::{self, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent};
use librespot::connect::Spirc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

pub enum SpotifyCommand {
    SetVolume(u16),
    PlayPause,
    Next,
    Prev,
    SetDevice(String),
}

pub async fn run(
    config: Config,
    state: AppState,
    mut cmd_rx: mpsc::Receiver<SpotifyCommand>,
) {
    loop {
        if let Err(e) = run_session(&config, &state, &mut cmd_rx).await {
            let msg = format!("Spotify session error: {e}, restarting in 2s...");
            error!("{msg}");
            state.write().await.push_log(msg);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

/// Outer loop: manages session, discovery, credentials.
/// Only recreated on actual session errors (auth failure, network down).
async fn run_session(
    config: &Config,
    state: &AppState,
    cmd_rx: &mut mpsc::Receiver<SpotifyCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut session_config = SessionConfig::default();
    session_config.ap_port = Some(443);
    let device_id = session_config.device_id.clone();
    let client_id = session_config.client_id.clone();

    let cache = Cache::new(
        Some(config.cache.clone()),
        None,
        None,
        None,
    )?;

    // Always start Zeroconf discovery so Spotify sees us on the local network.
    let mut discovery = Discovery::builder(device_id, client_id)
        .name(config.name.clone())
        .device_type(DeviceType::Speaker)
        .launch()?;
    info!("Zeroconf discovery started as '{}'", config.name);

    let credentials = if let Some(creds) = cache.credentials() {
        info!("Using cached credentials");
        creds
    } else {
        info!("Waiting for Spotify app to connect...");
        use futures_util::StreamExt;
        let creds = discovery.next().await
            .ok_or("Discovery stream ended without credentials")?;
        info!("Received credentials via Zeroconf");
        creds
    };

    // Keep discovery alive in the background for LAN visibility
    tokio::spawn(async move {
        use futures_util::StreamExt;
        while let Some(_creds) = discovery.next().await {}
    });

    let session = Session::new(session_config, Some(cache));

    {
        let mut s = state.write().await;
        s.volume = config.initial_volume;
    }

    // Inner loop: recreates player/spirc on device switch, reuses session.
    loop {
        match run_spirc(config, state, cmd_rx, &session, &credentials).await {
            Ok(SpircExit::DeviceSwitch) => {
                info!("Device switched, recreating player...");
                continue;
            }
            Ok(SpircExit::Shutdown) => {
                return Ok(());
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}

enum SpircExit {
    DeviceSwitch,
    Shutdown,
}

/// Inner loop: manages player, mixer, spirc.
/// Returns DeviceSwitch to recreate with new device, or error to restart session.
async fn run_spirc(
    config: &Config,
    state: &AppState,
    cmd_rx: &mut mpsc::Receiver<SpotifyCommand>,
    session: &Session,
    credentials: &Credentials,
) -> Result<SpircExit, Box<dyn std::error::Error + Send + Sync>> {
    let mixer_config = MixerConfig::default();
    let mixer_fn = mixer::find(Some("softvol")).expect("No softmixer available");
    let mixer = mixer_fn(mixer_config)?;

    let player_config = PlayerConfig {
        bitrate: match config.bitrate {
            96 => Bitrate::Bitrate96,
            160 => Bitrate::Bitrate160,
            _ => Bitrate::Bitrate320,
        },
        ..Default::default()
    };

    let current_device = state.read().await.device.clone();
    let device = if current_device.is_empty() || current_device == "auto" {
        None
    } else {
        Some(current_device)
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

    let connect_config = ConnectConfig {
        name: config.name.clone(),
        device_type: DeviceType::Speaker,
        initial_volume: (config.initial_volume as u16).min(100) * 655,
        ..Default::default()
    };

    let (spirc, spirc_task) = Spirc::new(
        connect_config,
        session.clone(),
        credentials.clone(),
        player,
        mixer,
    ).await?;

    let msg = format!("Spirc started, device '{}' visible on Spotify Connect", config.name);
    info!("{msg}");
    state.write().await.push_log(msg);

    let spirc_handle = tokio::spawn(spirc_task);

    let exit = loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(PlayerEvent::Playing { track_id, .. }) => {
                        match Track::get(session, &track_id).await {
                            Ok(track) => {
                                let artist_name = track.artists.0.first()
                                    .map(|a| a.name.clone())
                                    .unwrap_or_default();

                                let cover_url = match Album::get(session, &track.album.id).await {
                                    Ok(album) => album.covers.0.iter()
                                        .find(|img| img.size == ImageSize::LARGE)
                                        .or_else(|| album.covers.0.first())
                                        .map(|img| format!("https://i.scdn.co/image/{}", img.id.to_base16()))
                                        .unwrap_or_default(),
                                    Err(_) => String::new(),
                                };

                                let mut s = state.write().await;
                                s.playing = true;
                                s.track = track.name;
                                s.artist = artist_name;
                                s.album = track.album.name;
                                s.cover_url = cover_url;
                                let msg = format!("Playing: {} - {}", s.artist, s.track);
                                info!("{msg}");
                                s.push_log(msg);
                            }
                            Err(e) => {
                                warn!("Failed to fetch track metadata: {e}");
                                state.write().await.push_log(format!("WARN: metadata fetch failed: {e}"));
                            }
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
                        break Err("Player event channel closed (sink error?)".into());
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
                    Some(SpotifyCommand::PlayPause) => {
                        let _ = spirc.play_pause();
                    }
                    Some(SpotifyCommand::Next) => {
                        let _ = spirc.next();
                    }
                    Some(SpotifyCommand::Prev) => {
                        let _ = spirc.prev();
                    }
                    Some(SpotifyCommand::SetDevice(dev)) => {
                        let _ = spirc.disconnect(true);
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                        let msg = format!("Switching output device to '{dev}'");
                        info!("{msg}");
                        let mut s = state.write().await;
                        s.push_log(msg);
                        s.playing = false;
                        s.device = dev;
                        drop(s);

                        break Ok(SpircExit::DeviceSwitch);
                    }
                    None => {
                        break Ok(SpircExit::Shutdown);
                    }
                }
            }
        }
    };

    let _ = spirc.shutdown();
    spirc_handle.abort();
    exit
}
