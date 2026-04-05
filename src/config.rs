use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_bitrate")]
    pub bitrate: u32,
    #[serde(default = "default_device")]
    pub device: String,
    #[serde(default = "default_cache")]
    pub cache: PathBuf,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_initial_volume")]
    pub initial_volume: u16,
}

fn default_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Spytti".into())
}

fn default_bitrate() -> u32 {
    320
}

fn default_device() -> String {
    "auto".into()
}

fn default_cache() -> PathBuf {
    PathBuf::from("/var/cache/spytti")
}

fn default_port() -> u16 {
    8080
}

fn default_initial_volume() -> u16 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: default_name(),
            bitrate: default_bitrate(),
            device: default_device(),
            cache: default_cache(),
            port: default_port(),
            initial_volume: default_initial_volume(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let paths = [
            PathBuf::from("/etc/spytti.toml"),
            dirs_next().join("spytti.toml"),
        ];

        for path in &paths {
            if path.exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => match toml::from_str(&content) {
                        Ok(config) => {
                            tracing::info!("Loaded config from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse {}: {}", path.display(), e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to read {}: {}", path.display(), e);
                    }
                }
            }
        }

        tracing::info!("No config file found, using defaults");
        Self::default()
    }
}

fn dirs_next() -> PathBuf {
    if let Some(config_dir) = dirs_next_config() {
        config_dir
    } else {
        PathBuf::from(".")
    }
}

fn dirs_next_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert_eq!(config.bitrate, 320);
        assert_eq!(config.device, "auto");
        assert_eq!(config.cache, PathBuf::from("/var/cache/spytti"));
        assert_eq!(config.port, 8080);
        assert_eq!(config.initial_volume, 30);
        assert!(!config.name.is_empty());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
            name = "Living Room"
            bitrate = 160
            device = "hw:CARD=Device,DEV=0"
            cache = "/tmp/spytti-test"
            port = 9090
            initial_volume = 50
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.name, "Living Room");
        assert_eq!(config.bitrate, 160);
        assert_eq!(config.device, "hw:CARD=Device,DEV=0");
        assert_eq!(config.cache, PathBuf::from("/tmp/spytti-test"));
        assert_eq!(config.port, 9090);
        assert_eq!(config.initial_volume, 50);
    }

    #[test]
    fn parse_partial_config_uses_defaults() {
        let config: Config = toml::from_str(r#"name = "Kitchen""#).unwrap();
        assert_eq!(config.name, "Kitchen");
        assert_eq!(config.bitrate, 320);
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn parse_empty_config() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.bitrate, 320);
        assert_eq!(config.initial_volume, 30);
    }
}
