use crate::{
    cache::{self, Check},
    config::Target,
};
use anyhow::Context;
use std::path::Path;

const DAYS: usize = 30;
const DAY: u64 = 86400;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const STYLE: &str = r#"
:root {
  color-scheme: light dark;
  --bg: #f5f6f8;
  --fg: #1d2128;
  --card: #fff;
  --line: #e3e6ea;
  --muted: #5f6875;
  --up: #1a7f37;
  --down: #cf222e;
  --idle: #d5dae0;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0f1115;
    --fg: #e6e8eb;
    --card: #181b21;
    --line: #2a2f37;
    --muted: #9aa3ae;
    --up: #3fb950;
    --down: #f85149;
    --idle: #2a2f37;
  }
}
body {
  margin: 0;
  padding: 24px 16px;
  background: var(--bg);
  color: var(--fg);
  font: 15px/1.4 system-ui, sans-serif;
}
main {
  max-width: 960px;
  margin: 0 auto;
}
header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: baseline;
  gap: 4px 16px;
  margin: 0 0 16px;
}
h1 {
  font-size: 1.5em;
  font-weight: 500;
  margin: 0;
}
header p {
  margin: 0;
  color: var(--muted);
  font-size: .85em;
}
.banner {
  margin: 0 0 16px;
  padding: 12px 16px;
  border-radius: 10px;
  border-left: 4px solid var(--up);
  background: var(--card);
  font-weight: 600;
}
.banner.down {
  border-color: var(--down);
  color: var(--down);
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
  gap: 12px;
}
article {
  min-width: 0;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--card);
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}
b {
  font-weight: 500;
  overflow-wrap: anywhere;
}
i {
  flex: none;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--idle);
  color: #fff;
  font: 700 13px/1 system-ui, sans-serif;
}
.up i {
  background: var(--up);
}
.down i {
  background: var(--down);
}
small {
  display: block;
  margin: 2px 0 12px;
  color: var(--muted);
  font-size: .8em;
  overflow-wrap: anywhere;
}
.beats {
  display: flex;
  gap: 2px;
  height: 32px;
}
.beats > div {
  position: relative;
  flex: 1;
  border-radius: 3px;
  background: var(--idle);
  outline: 0;
  cursor: default;
}
.beats .ok {
  background: var(--up);
}
.beats .fail {
  background: var(--down);
}
.beats > div:hover,
.beats > div:focus-visible {
  background: var(--muted);
}
.tip {
  display: none;
  position: absolute;
  z-index: 1;
  top: calc(100% + 10px);
  left: 50%;
  transform: translateX(-50%);
  width: max-content;
  max-width: min(280px, 80vw);
  padding: 14px 16px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--card);
  box-shadow: 0 8px 24px rgb(0 0 0 / .18);
  font-size: .85em;
  pointer-events: none;
}
.beats > div:hover .tip,
.beats > div:focus .tip {
  display: block;
}
.beats > div:nth-child(-n + 8) .tip {
  left: 0;
  transform: none;
}
.beats > div:nth-last-child(-n + 8) .tip {
  left: auto;
  right: 0;
  transform: none;
}
.tip strong {
  display: block;
  margin: 0 0 10px;
  font-weight: 600;
}
.tip p {
  margin: 0;
  color: var(--muted);
}
.tip .row {
  display: flex;
  justify-content: space-between;
  gap: 32px;
  margin: 0 0 8px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--bg);
  color: var(--down);
  font-weight: 500;
}
.axis {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 8px 0 6px;
  color: var(--muted);
  font-size: .8em;
}
.axis hr {
  flex: 1;
  margin: 0;
  border: 0;
  border-top: 1px solid var(--line);
}
.state {
  margin: 0;
  color: var(--muted);
  font-size: .85em;
}
footer {
  margin: 24px 0 0;
  color: var(--muted);
  font-size: .8em;
  text-align: center;
}
footer a {
  color: inherit;
}
.up .state {
  color: var(--up);
}
.down .state {
  color: var(--down);
}
"#;

pub fn write_page(targets: &[Target], out: &Path) -> anyhow::Result<()> {
    let html = render(targets, &cache::read_checks(), cache::now_secs());
    std::fs::write(out, html).with_context(|| format!("writing {}", out.display()))
}

fn render(targets: &[Target], checks: &[Check], now: u64) -> String {
    let mut down = 0;
    let cards: String = targets
        .iter()
        .map(|t| {
            let mine: Vec<&Check> = checks.iter().filter(|c| c.name == t.name).collect();
            let (class, mark, state) = match mine.last() {
                Some(c) if c.ok => ("up", "✓", "UP"),
                Some(_) => ("down", "✕", "DOWN"),
                None => ("none", "–", "NO DATA"),
            };
            down += usize::from(class == "down");
            let latency = match mine.last() {
                Some(c) if c.ok => format!("{}ms", c.ms),
                _ => "-".into(),
            };
            let checked = mine.last().map_or("never checked".into(), |c| {
                format!("{} ago", cache::age(now.saturating_sub(c.ts)))
            });
            let beats = days(&mine, now);
            format!(
                r#"<article class="{class}">
  <div class="head">
    <b>{name}</b>
    <i title="{state}">{mark}</i>
  </div>
  <small>{url}</small>
  <div class="beats">
{beats}  </div>
  <div class="axis">
    <span>{DAYS} days ago</span>
    <hr>
    <span>{uptime} uptime 30d</span>
    <hr>
    <span>today</span>
  </div>
  <p class="state">{state} · {latency} · {checked}</p>
</article>
"#,
                name = escape(&t.name),
                url = escape(&t.url),
                uptime = cache::uptime(&mine, now.saturating_sub(DAYS as u64 * DAY)),
            )
        })
        .collect();
    let banner = match down {
        0 => r#"<p class="banner">All systems up</p>"#.to_string(),
        n => format!(
            r#"<p class="banner down">{n} of {} targets down</p>"#,
            targets.len()
        ),
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>tunnelcheck status</title>
<style>{STYLE}</style>
</head>
<body>
<main>
<header>
  <h1>Current status</h1>
  <p>Daily status over the last {DAYS} days (UTC)</p>
</header>
{banner}
<section class="grid">
{cards}</section>
<footer>
  Generated {generated} by tunnelcheck v{version}<br>
  <a href="{repo}">Source</a>
</footer>
</main>
</body>
</html>
"#,
        generated = utc(now),
        version = env!("CARGO_PKG_VERSION"),
        repo = env!("CARGO_PKG_REPOSITORY"),
    )
}

fn days(mine: &[&Check], now: u64) -> String {
    let first = (now - now % DAY).saturating_sub((DAYS as u64 - 1) * DAY);
    let mut down = [0u64; DAYS];
    let mut fails = [0usize; DAYS];
    let mut seen = [false; DAYS];
    for (i, c) in mine.iter().enumerate() {
        if let Some(d) = c.ts.checked_sub(first).map(|s| (s / DAY) as usize)
            && d < DAYS
        {
            seen[d] = true;
            fails[d] += usize::from(!c.ok);
        }
        if c.ok {
            continue;
        }
        let end = mine.get(i + 1).map_or(now, |n| n.ts);
        for (d, secs) in down.iter_mut().enumerate() {
            let start = first + d as u64 * DAY;
            *secs += end.min(start + DAY).saturating_sub(c.ts.max(start));
        }
    }
    (0..DAYS)
        .map(|d| {
            let (class, body) = if down[d] > 0 || fails[d] > 0 {
                (
                    "fail",
                    format!(
                        r#"<p class="row"><span>Downtime</span><span>{}</span></p>
        <p>{}</p>"#,
                        duration(down[d]),
                        plural(fails[d] as u64, "failed check")
                    ),
                )
            } else if seen[d] {
                ("ok", "<p>No downtime recorded on this day.</p>".into())
            } else {
                ("none", "<p>No data recorded on this day.</p>".into())
            };
            format!(
                r#"    <div class="{class}" tabindex="0">
      <div class="tip">
        <strong>{}</strong>
        {body}
      </div>
    </div>
"#,
                date(first + d as u64 * DAY)
            )
        })
        .collect()
}

fn duration(secs: u64) -> String {
    match (secs / 3600, secs % 3600 / 60) {
        (0, 0) => "&lt;1 min".into(),
        (0, m) => plural(m, "min"),
        (h, 0) => plural(h, "hr"),
        (h, m) => format!("{} {}", plural(h, "hr"), plural(m, "min")),
    }
}

fn plural(n: u64, unit: &str) -> String {
    format!("{n} {unit}{}", if n == 1 { "" } else { "s" })
}

fn gmtime(secs: u64) -> Option<libc::tm> {
    let secs = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    (!unsafe { libc::gmtime_r(&secs, &mut tm) }.is_null()).then_some(tm)
}

fn date(secs: u64) -> String {
    gmtime(secs).map_or("-".into(), |tm| {
        format!(
            "{} {} {}",
            tm.tm_mday,
            MONTHS[tm.tm_mon as usize],
            tm.tm_year + 1900
        )
    })
}

fn utc(secs: u64) -> String {
    let Some(tm) = gmtime(secs) else {
        return "-".into();
    };
    format!(
        "{}-{:02}-{:02} {:02}:{:02} UTC",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min
    )
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "../tests/status.rs"]
mod tests;
