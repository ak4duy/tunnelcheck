use serde::{Deserialize, Serialize};

use std::{
    fs::OpenOptions,
    io::{IsTerminal, Write},
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

#[derive(Deserialize)]
struct Entry {
    name: String,
    status: String,
    ts: u64,
    message: String,
}

fn events_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tunnelcheck/events.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn age(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m", secs / 60),
        3600..86400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86400),
    }
}

pub fn show_history(only: Option<&str>, page_size: usize) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(events_path()).unwrap_or_default();
    let mut entries: Vec<Entry> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .filter(|e: &Entry| only.is_none_or(|n| e.name == n))
        .collect();
    if entries.is_empty() {
        println!("no history");
        return Ok(());
    }
    entries.reverse();
    let pages: Vec<_> = entries.chunks(page_size).collect();
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let now = now_secs();
    let mut page = 0;
    loop {
        for e in pages[page] {
            let ago = age(now.saturating_sub(e.ts));
            println!("{ago:>5} ago  {:<4}  {:<16}  {}", e.status, e.name, e.message);
        }
        if !interactive {
            if page + 1 == pages.len() {
                return Ok(());
            }
            page += 1;
            continue;
        }
        print!("-- page {}/{} [n]ext [p]rev [q]uit: ", page + 1, pages.len());
        std::io::stdout().flush()?;
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line)? == 0 {
            return Ok(());
        }
        match line.trim() {
            "n" | "" if page + 1 < pages.len() => page += 1,
            "p" if page > 0 => page -= 1,
            "q" => return Ok(()),
            _ => {}
        }
    }
}

pub fn cache_state(name: &str, ok: bool, msg: &str) -> std::io::Result<()> {
    let path = events_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let status = if ok { "UP" } else { "DOWN" };
    let ts = now_secs();
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
