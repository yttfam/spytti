# Spytti

A minimal Spotify Connect daemon in Rust. Single binary, three async tasks: librespot for Spotify Connect, axum for the web UI, and ALSA for audio output.

Built to replace moOde on headless Raspberry Pi audio endpoints.

## Features

- **Spotify Connect receiver** via [librespot](https://github.com/librespot-org/librespot) — shows up in your Spotify app
- **Web UI** at port 8080 — now playing, player controls (play/pause, next, prev), volume slider
- **ALSA output** — raw ALSA, no PulseAudio or PipeWire
- **Zeroconf discovery** — no credentials in config, authenticate from the Spotify app
- **Credential caching** — reconnects automatically after restart

## Web API

```
GET  /              HTML UI (embedded, no external deps)
GET  /api/status    {"playing": true, "track": "...", "artist": "...", "album": "...", "volume": 75}
POST /api/volume    {"volume": 75}
POST /api/play-pause
POST /api/next
POST /api/prev
GET  /api/health    200 OK
```

## Configuration

TOML config at `/etc/spytti.toml` or `~/.config/spytti.toml`:

```toml
name = "Spotify Salon"      # Spotify Connect device name (default: hostname)
bitrate = 320                # 96, 160, or 320 (default: 320)
device = "auto"              # ALSA device, or "auto" (default: auto)
cache = "/var/cache/spytti"  # Credential cache path
port = 8080                  # Web UI port
initial_volume = 30          # 0-100
```

All fields are optional. Without a config file, defaults are used.

## Building

### macOS (dev)

```bash
cargo build                  # uses rodio backend
cargo test
```

### Cross-compile for Raspberry Pi

Requires the `aarch64-unknown-linux-gnu` toolchain and ALSA dev headers in the sysroot.

```bash
# Install cross toolchain (macOS)
brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu

# Build
cargo build --target aarch64-unknown-linux-gnu --release \
  --no-default-features --features backend-alsa,rustls,zeroconf

# Strip
aarch64-unknown-linux-gnu-strip target/aarch64-unknown-linux-gnu/release/spytti
```

See `.cargo/config.toml` for linker configuration.

## Deployment

```bash
scp target/aarch64-unknown-linux-gnu/release/spytti user@pi:/usr/local/bin/
scp spytti.toml user@pi:/etc/spytti.toml
scp spytti.service user@pi:/etc/systemd/system/

ssh user@pi "sudo systemctl enable --now spytti"
```

### First run

On first launch, spytti starts Zeroconf discovery. Open your Spotify app, find the device name in the device list, and connect. Credentials are cached for future restarts.

## Architecture

```
main.rs      tokio runtime, signal handling
config.rs    TOML config with serde defaults
state.rs     Arc<RwLock<SharedState>> shared between tasks
spotify.rs   librespot session, Spirc, player event loop
web.rs       axum routes
ui.html      embedded single-page UI
```

## Target hardware

- Raspberry Pi 3/4 (aarch64, Raspbian/Debian)
- USB audio output via ALSA

## License

MIT
