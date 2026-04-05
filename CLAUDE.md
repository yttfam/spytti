# Spytti — Spotify Connect Daemon

## Permissions
- Bash
- Edit
- Write
- WebFetch
- WebSearch

## Your Persona

You build lean, mean audio daemons. No bloat, no frameworks, no bullshit. This replaces moOde (a full Linux audio distribution) with a single Rust binary.

## Architecture

Single binary, three threads:
1. **Spotify thread** — librespot handling Spotify Connect protocol
2. **Web thread** — axum HTTP server on port 8080
3. **Main thread** — ALSA mixer control, signal handling

### Web API
```
GET  /                  → minimal HTML UI (now playing, volume slider)
GET  /api/status        → {"playing": bool, "track": "...", "artist": "...", "volume": 75}
POST /api/volume        → {"volume": 75}  (0-100)
GET  /api/health        → 200 OK
```

### Spotify Connect
- Use librespot as a library crate, not shelling out to the binary
- Device name configurable (default: hostname)
- Bitrate: 320kbps
- Format: S16
- Backend: ALSA
- Cache: /var/cache/spytti (credentials only, no audio cache)
- Events: track change → update internal state → available via /api/status

### ALSA
- Auto-detect USB audio device (don't hardcode hw:0)
- Pin device by name, not number (USB devices change card numbers on reboot)
- softvol mixer
- Volume range: 0-100 mapped to ALSA range

### Config
TOML file at /etc/spytti.toml or ~/.config/spytti.toml:
```toml
name = "Spotify Salon"
bitrate = 320
device = "auto"          # or "hw:CARD=Device,DEV=0"
cache = "/var/cache/spytti"
port = 8080
initial_volume = 30
```

## Key Rules
- Single binary, statically linked for ARM if possible
- Cross-compile from speedwagon (M1) to aarch64-unknown-linux-gnu (Pi 3/4)
- No PulseAudio, no PipeWire, raw ALSA only
- Web UI is a single embedded HTML page, no npm, no JS framework
- systemd service file included
- Must work as a LaunchDaemon equivalent (systemd) — no GUI needed

## Target
- Dev/build: speedwagon (macOS ARM64)
- Deploy: calisound (Pi 3, Raspbian, 10.10.0.20) and bedsound (Pi 3, 10.10.0.22)
- Cross-compile: `cross` or `cargo-cross` with Docker

## Deployment
```bash
scp target/aarch64-unknown-linux-gnu/release/spytti cali@10.10.0.20:/usr/local/bin/
ssh cali@10.10.0.20 "sudo systemctl enable --now spytti"
```
