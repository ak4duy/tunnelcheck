use serde::{Deserialize, Serialize};

use std::{
    collections::HashMap,
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

#[derive(Serialize, Deserialize)]
struct Check {
    name: String,
    ok: bool,
    ms: u64,
    ts: u64,
}

const BEATS: usize = 30;

#[derive(Deserialize)]
struct Entry {
    name: String,
    status: String,
    ts: u64,
    message: String,
}

fn cache_path(file: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tunnelcheck")
        .join(file)
}

fn append(file: &str, value: &impl Serialize) -> std::io::Result<()> {
    let path = cache_path(file);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(value)?)
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
    let text = std::fs::read_to_string(cache_path("events.json")).unwrap_or_default();
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
            println!(
                "{ago:>5} ago  {:<4}  {:<16}  {}",
                e.status, e.name, e.message
            );
        }
        if !interactive {
            if page + 1 == pages.len() {
                return Ok(());
            }
            page += 1;
            continue;
        }
        print!(
            "-- page {}/{} [n]ext [p]rev [q]uit: ",
            page + 1,
            pages.len()
        );
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

pub fn last_states() -> HashMap<String, bool> {
    let text = std::fs::read_to_string(cache_path("events.json")).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .map(|e| (e.name, e.status == "UP"))
        .collect()
}

pub fn cache_state(name: &str, ok: bool, msg: &str) -> std::io::Result<()> {
    let status = if ok { "UP" } else { "DOWN" };
    append(
        "events.json",
        &Event {
            name,
            status,
            ts: now_secs(),
            message: msg,
        },
    )
}

// shortcut: checks.json grows forever (~100KB/target/month at 5min), add rotation if that matters
pub fn record_check(name: &str, ok: bool, ms: u64) -> std::io::Result<()> {
    append(
        "checks.json",
        &Check {
            name: name.to_string(),
            ok,
            ms,
            ts: now_secs(),
        },
    )
}

fn uptime(checks: &[&Check], since: u64) -> String {
    let window: Vec<_> = checks.iter().filter(|c| c.ts >= since).collect();
    if window.is_empty() {
        return "-".into();
    }
    let up = window.iter().filter(|c| c.ok).count();
    format!("{:.2}%", up as f64 * 100.0 / window.len() as f64)
}

fn sparkline(ms: &[u64]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let lo = ms.iter().copied().min().unwrap_or(0);
    let hi = ms.iter().copied().max().unwrap_or(0);
    ms.iter()
        .map(|&v| BARS[((v - lo) * 7 / (hi - lo).max(1)) as usize])
        .collect()
}

pub fn show_status(names: &[&str]) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(cache_path("checks.json")).unwrap_or_default();
    let checks: Vec<Check> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let paint = |code: &str, s: &str| {
        if color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let now = now_secs();
    let w = names
        .iter()
        .map(|n| n.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);

    println!(
        "{:<w$}  {:<4}  {:<BEATS$}  {:>7}  {:>7}  {:>7}  LATENCY",
        "NAME", "NOW", "LAST CHECKS", "24H", "30D", "AVG"
    );
    for name in names {
        let mine: Vec<&Check> = checks.iter().filter(|c| c.name == *name).collect();
        let recent = &mine[mine.len().saturating_sub(BEATS)..];
        let beats: String = recent
            .iter()
            .map(|c| {
                if c.ok {
                    paint("32", "█")
                } else {
                    paint("31", "▁")
                }
            })
            .collect();
        let state = match mine.last() {
            Some(c) if c.ok => paint("32", "UP  "),
            Some(_) => paint("31", "DOWN"),
            None => "-   ".into(),
        };
        let up_ms: Vec<u64> = recent.iter().filter(|c| c.ok).map(|c| c.ms).collect();
        let avg = match up_ms.len() {
            0 => "-".into(),
            n => format!("{}ms", up_ms.iter().sum::<u64>() / n as u64),
        };
        println!(
            "{name:<w$}  {state}  {}{beats}  {:>7}  {:>7}  {avg:>7}  {}",
            " ".repeat(BEATS - recent.len()),
            uptime(&mine, now.saturating_sub(86400)),
            uptime(&mine, now.saturating_sub(30 * 86400)),
            sparkline(&up_ms)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_counts_only_window() {
        let c = |ok, ts| Check {
            name: "a".into(),
            ok,
            ms: 1,
            ts,
        };
        let checks = [c(false, 10), c(true, 100), c(true, 200), c(false, 300)];
        let refs: Vec<&Check> = checks.iter().collect();
        assert_eq!(uptime(&refs, 0), "50.00%");
        assert_eq!(uptime(&refs, 100), "66.67%");
        assert_eq!(uptime(&refs, 1000), "-");
    }

    #[test]
    fn sparkline_scales_min_to_max() {
        assert_eq!(sparkline(&[10, 45, 80]), "▁▄█");
        assert_eq!(sparkline(&[5, 5]), "▁▁");
        assert_eq!(sparkline(&[]), "");
    }
}
