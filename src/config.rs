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
