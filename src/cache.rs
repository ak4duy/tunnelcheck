use serde::Serialize;

use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
struct Event<'a> {
    name: &'a str,
    status: &'a str,
    ts: u64,
    message: &'a str,
}

pub fn cache_state(name: &str, ok: bool, msg: &str) -> std::io::Result<()> {
    let path = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tunnelcheck/events.json");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let status = if ok { "UP" } else { "DOWN" };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        f,
        "{}",
        serde_json::to_string(&Event {
            name,
            status,
            ts,
            message: msg
        })?
    )
}
