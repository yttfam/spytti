# Spytti 🎵

The YTT family's music sibling. A minimal Spotify Connect receiver with volume control and a tiny web UI.

Replaces moOde's 500MB bloat with ~2MB of Rust.

## What it does

- Spotify Connect receiver (librespot)
- ALSA volume control
- Minimal web UI (now playing, volume, source)
- Zero dependencies beyond ALSA

## What it doesn't do

- MPD
- File playback
- Bluetooth
- DAB radio
- Any of the other 47 things moOde ships

## Target hardware

- Raspberry Pi 3 (calisound, 10.10.0.20)
- Raspberry Pi 3 (bedsound, 10.10.0.22)
- USB audio output (ALSA)

## Stack

- Rust
- librespot (Spotify Connect)
- ALSA (audio output + mixer)
- axum (web server)
- Cross-compile for aarch64/armv7
