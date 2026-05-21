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
    Release,
}

pub async fn run(
    config: Config,
    state: AppState,
    mut cmd_rx: mpsc::Receiver<SpotifyCommand>,
) {
    // Stable device_id + Discovery live for the program's lifetime.
    // Recreating them on each session restart causes DH key mismatches and
    // stale mDNS advertisements, breaking fast user handoff.
    let session_config_base = {
        let mut c = SessionConfig::default();
        c.ap_port = Some(443);
        c
    };
    let device_id = session_config_base.device_id.clone();
    let client_id = session_config_base.client_id.clone();

    let discovery = match Discovery::builder(device_id.clone(), client_id.clone())
        .name(config.name.clone())
        .device_type(DeviceType::Speaker)
        .launch()
    {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to start Zeroconf discovery: {e}");
            return;
        }
    };
    info!("Zeroconf discovery started as '{}'", config.name);

    // Long-lived creds channel — survives session restarts.
    let (creds_tx, mut creds_rx) = mpsc::channel::<Credentials>(1);
    tokio::spawn(async move {
        let mut discovery = discovery;
        use futures_util::StreamExt;
        while let Some(new_creds) = discovery.next().await {
            info!("Zeroconf: new user '{}' connecting, triggering switch...", new_creds.username.as_deref().unwrap_or("?"));
            if creds_tx.send(new_creds).await.is_err() {
                break;
            }
        }
    });

    // Single session lifetime. User switches trigger process exit + systemd restart.
    loop {
        match run_session(
            &config,
            &state,
            &mut cmd_rx,
            &mut creds_rx,
            &session_config_base,
        ).await {
            Ok(()) => return,
            Err(e) => {
                error!("Spotify session error: {e}, restarting...");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
}

/// Manages a single Spotify session: auth, spclient, dealer, player, spirc.
/// Recreated on user switch or session errors. Discovery stays up in run().
async fn run_session(
    config: &Config,
    state: &AppState,
    cmd_rx: &mut mpsc::Receiver<SpotifyCommand>,
    creds_rx: &mut mpsc::Receiver<Credentials>,
    session_config_base: &SessionConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cache = Cache::new(
        Some(config.cache.clone()),
        None,
        None,
        None,
    )?;

    let credentials = if let Some(creds) = cache.credentials() {
        info!("Using cached credentials for '{}'", creds.username.as_deref().unwrap_or("?"));
        creds
    } else {
        info!("Waiting for Spotify app to connect...");
        let creds = creds_rx.recv().await
            .ok_or("Credentials channel closed")?;
        info!("Received credentials via Zeroconf");
        // Save for next startup
        let tmp_cache = Cache::new(Some(config.cache.clone()), None, None, None).ok();
        if let Some(c) = tmp_cache.as_ref() {
            c.save_credentials(&creds);
        }
        creds
    };

    let session = Session::new(session_config_base.clone(), Some(cache));

    {
        let mut s = state.write().await;
        s.volume = config.initial_volume;
    }

    // Inner loop: recreates player/spirc on device switch, reuses session.
    loop {
        match run_spirc(config, state, cmd_rx, creds_rx, &session, &credentials).await {
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
    creds_rx: &mut mpsc::Receiver<Credentials>,
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

    let t0 = std::time::Instant::now();
    let (spirc, spirc_task) = Spirc::new(
        connect_config,
        session.clone(),
        credentials.clone(),
        player,
        mixer,
    ).await?;
    info!("Spirc ready in {}ms, device '{}' visible", t0.elapsed().as_millis(), config.name);
    {
        let mut s = state.write().await;
        s.restarting = false;
        s.active_user = credentials.username.clone().unwrap_or_default();
    }

    let spirc_handle = tokio::spawn(spirc_task);

    let exit = loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(PlayerEvent::Playing { track_id, position_ms, .. }) => {
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
                                s.last_track_uri = track_id.to_string();
                                s.last_position_ms = position_ms;
                                info!("Playing: {} - {}", s.artist, s.track);
                            }
                            Err(e) => {
                                warn!("Failed to fetch track metadata: {e}");
                            }
                        }
                    }
                    Some(PlayerEvent::PositionChanged { position_ms, .. } | PlayerEvent::Seeked { position_ms, .. }) => {
                        state.write().await.last_position_ms = position_ms;
                    }
                    Some(PlayerEvent::Paused { position_ms, .. }) => {
                        let mut s = state.write().await;
                        s.playing = false;
                        s.last_position_ms = position_ms;
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

                        info!("Switching output device to '{dev}' — reselect in Spotify app");
                        let mut s = state.write().await;
                        s.restarting = true;
                        s.playing = false;
                        s.device = dev;
                        drop(s);

                        break Ok(SpircExit::DeviceSwitch);
                    }
                    Some(SpotifyCommand::Release) => {
                        info!("Release requested: full process restart to clear client-side caches");
                        let _ = spirc.disconnect(true);
                        let _ = std::fs::remove_file(config.cache.join("credentials.json"));
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        // systemd restarts us within ~1s, everyone's Spotify app
                        // sees the device disappear and reappear, forcing fresh Zeroconf.
                        std::process::exit(0);
                    }
                    None => {
                        break Ok(SpircExit::Shutdown);
                    }
                }
            }
            new_creds = creds_rx.recv() => {
                if let Some(new_creds) = new_creds {
                    let user = new_creds.username.as_deref().unwrap_or("?").to_string();
                    let _ = spirc.disconnect(true);

                    // Persist new creds to cache so the next process startup picks them up,
                    // then exit. systemd restarts us in ~1s — mDNS advertisement briefly
                    // drops and reappears, which flushes stale Spotify-app client state on
                    // both sides. Every user switch = clean slate.
                    let cache = Cache::new(Some(config.cache.clone()), None, None, None).ok();
                    if let Some(c) = cache.as_ref() {
                        c.save_credentials(&new_creds);
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    info!("Handing off to '{user}' via process restart");
                    std::process::exit(0);
                }
            }
        }
    };

    let _ = spirc.shutdown();
    spirc_handle.abort();
    exit
}
