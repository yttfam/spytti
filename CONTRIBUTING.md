# Contributing to Spytti

## Philosophy

Spytti is a single-purpose tool. It plays Spotify over ALSA. That's it.

Before proposing a feature, ask: does this belong in a 12MB binary on a Raspberry Pi? If the answer involves "optional" or "configurable" or "plugin system", the answer is probably no.

## Ground rules

- No frameworks. No npm. No build tools beyond cargo.
- The web UI is one HTML file. It stays that way.
- Dependencies must justify their weight. Check `cargo bloat --crates` before adding one.
- If it works on a Pi 3 with 1GB RAM, it ships. If it doesn't, it doesn't.

## Development

```bash
# Build (macOS dev, rodio backend)
cargo build
cargo test

# Cross-compile for Pi (aarch64, ALSA backend)
cargo build --target aarch64-unknown-linux-gnu --release \
  --no-default-features --features backend-alsa,rustls,zeroconf
```

## Code style

- No unnecessary abstractions. Three lines of duplicated code beats a premature helper function.
- No comments explaining what the code does. Comments explain *why*.
- Error handling: recover if you can, crash clearly if you can't.
- Tests: unit tests inline with `#[cfg(test)]`, integration tests in `tests/`.

## Pull requests

- One thing per PR. Bug fix? One PR. Feature? One PR. Don't mix.
- Describe what changed and why. Not how — the diff shows that.
- If it touches the web UI, test it on a phone screen. The Pi sits in a corner, you control it from the couch.
- If it adds a dependency, explain why nothing lighter exists.

## What we'll merge

- Bug fixes
- Performance improvements (especially startup time and memory)
- ALSA compatibility fixes for different DACs
- Web UI improvements that don't add JS dependencies

## What we won't merge

- Bluetooth support
- AirPlay support
- MPD integration
- A settings page
- TypeScript
- Anything that requires `node_modules/`

## Reporting issues

Include:
- Pi model and OS version
- USB DAC model (if relevant)
- Output of `aplay -l`
- Relevant lines from `journalctl -u spytti`
