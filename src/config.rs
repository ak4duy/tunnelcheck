use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default, rename = "target")]
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Clone)]
pub struct Target {
    pub name: String,
    pub url: String,
    #[serde(default = "default_status")]
    pub expect_status: u16,
    pub timeout_ms: Option<u64>,
}

pub fn default_timeout() -> u64 {
    5000
}

pub fn default_status() -> u16 {
    200
}

pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tunnelcheck/config.toml")
}
