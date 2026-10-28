use serde::Deserialize;
use std::{collections::HashSet, num::NonZeroU64, path::PathBuf};

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
    /// Discord ID for mention
    pub mention: Option<String>,
    /// Custom message for mention
    pub mention_message: Option<String>,
    /// Consecutive failures before a target is declared down
    #[serde(default = "default_failures_before_alert")]
    pub failures_before_alert: NonZeroU64,
    /// Resend the down alert every N seconds while a target stays down
    pub remind_every_secs: Option<NonZeroU64>,
    /// "HH:MM-HH:MM" local time window in which down alerts are held back
    pub quiet_hours: Option<QuietHours>,
    #[serde(default, rename = "target")]
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Clone)]
pub struct Target {
    pub name: String,
    /// http(s)://..., tcp://host:port, icmp://host or dns://name
    pub url: String,
    #[serde(default = "default_status")]
    pub expect_status: u16,
    pub timeout_secs: Option<u64>,
    pub retries: Option<u64>,
    /// HTTP body must contain this text
    pub keyword: Option<String>,
    /// Overrides the --watch interval for this target
    pub interval_secs: Option<NonZeroU64>,
    pub failures_before_alert: Option<NonZeroU64>,
    pub remind_every_secs: Option<NonZeroU64>,
    pub quiet_hours: Option<QuietHours>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(try_from = "String")]
pub struct QuietHours {
    start: u32,
    end: u32,
}

impl TryFrom<String> for QuietHours {
    type Error = String;

    fn try_from(s: String) -> Result<Self, String> {
        let minutes = |hm: &str| {
            let (h, m) = hm.trim().split_once(':')?;
            let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
            (h < 24 && m < 60).then_some(h * 60 + m)
        };
        s.split_once('-')
            .and_then(|(a, b)| {
                Some(QuietHours {
                    start: minutes(a)?,
                    end: minutes(b)?,
                })
            })
            .ok_or_else(|| format!("quiet_hours {s:?} is not HH:MM-HH:MM"))
    }
}

impl QuietHours {
    pub fn contains(&self, minute_of_day: u32) -> bool {
        if self.start <= self.end {
            (self.start..self.end).contains(&minute_of_day)
        } else {
            minute_of_day >= self.start || minute_of_day < self.end
        }
    }
}

pub fn default_failures_before_alert() -> NonZeroU64 {
    NonZeroU64::MIN
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

pub fn problems(text: &str) -> anyhow::Result<Vec<String>> {
    let mut found = Vec::new();
    let cfg: Config = serde_ignored::deserialize(toml::Deserializer::new(text), |path| {
        found.push(format!("unknown key: {path}"))
    })?;
    let mut seen = HashSet::new();
    for t in &cfg.targets {
        if !seen.insert(t.name.as_str()) {
            found.push(format!("duplicate target name: {}", t.name));
        }
        if !["http://", "https://", "tcp://", "icmp://", "dns://"]
            .iter()
            .any(|s| t.url.starts_with(s))
        {
            found.push(format!("{}: unsupported url {:?}", t.name, t.url));
        }
    }
    Ok(found)
}

#[cfg(test)]
#[path = "../tests/config.rs"]
mod tests;
