use serde::Deserialize;
use std::{num::NonZeroU64, path::PathBuf};

#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Retries
    #[serde(default = "default_retries")]
    pub retries: u64,
    /// Fail HTTPS checks whose certificate expires within this many days (0 disables)
    #[serde(default = "default_cert_warn_days")]
    pub cert_warn_days: i64,
    /// Discord webhook to POST to when a target goes down
    pub webhook_url: Option<String>,
    #[serde(default, rename = "target")]
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Clone)]
pub struct Target {
    pub name: String,
    /// http(s)://... or tcp://host:port
    pub url: String,
    #[serde(default = "default_status")]
    pub expect_status: u16,
    pub timeout_secs: Option<u64>,
    pub retries: Option<u64>,
    /// HTTP body must contain this text
    pub keyword: Option<String>,
    /// Overrides the --watch interval for this target
    pub interval_secs: Option<NonZeroU64>,
}

pub fn default_timeout() -> u64 {
    5
}

pub fn default_retries() -> u64 {
    0
}

pub fn default_cert_warn_days() -> i64 {
    7
}

pub fn default_status() -> u16 {
    200
}

pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tunnelcheck/config.toml")
}
