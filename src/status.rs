use crate::{
    cache::{self, BEATS, Check},
    config::Target,
};
use anyhow::Context;
use std::path::Path;

const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#f5f6f8;--fg:#1d2128;--card:#fff;--line:#e3e6ea;--muted:#5f6875;--up:#1a7f37;--down:#cf222e;--idle:#d5dae0}
@media (prefers-color-scheme:dark){:root{--bg:#0f1115;--fg:#e6e8eb;--card:#181b21;--line:#2a2f37;--muted:#9aa3ae;--up:#3fb950;--down:#f85149;--idle:#2a2f37}}
body{margin:0;padding:24px 16px;background:var(--bg);color:var(--fg);font:15px/1.4 system-ui,sans-serif}
main{max-width:960px;margin:0 auto}
header{display:flex;flex-wrap:wrap;justify-content:space-between;align-items:baseline;gap:4px 16px;margin:0 0 16px}
h1{font-size:1.5em;font-weight:500;margin:0}
header p{margin:0;color:var(--muted);font-size:.85em}
.banner{margin:0 0 16px;padding:12px 16px;border-radius:10px;border-left:4px solid var(--up);background:var(--card);font-weight:600}
.banner.down{border-color:var(--down);color:var(--down)}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,340px),1fr));gap:12px}
article{min-width:0;padding:16px;border:1px solid var(--line);border-radius:10px;background:var(--card)}
.head{display:flex;justify-content:space-between;align-items:center;gap:8px}
b{font-weight:500;overflow-wrap:anywhere}
i{flex:none;display:grid;place-items:center;width:22px;height:22px;border-radius:50%;background:var(--idle);color:#fff;font:700 13px/1 system-ui,sans-serif}
.up i{background:var(--up)}
.down i{background:var(--down)}
small{display:block;margin:2px 0 12px;color:var(--muted);font-size:.8em;overflow-wrap:anywhere}
.beats{display:flex;gap:2px;height:32px}
.beats span{flex:1;border-radius:3px;background:var(--idle)}
.beats .ok{background:var(--up);opacity:.5}
.beats .ok:last-child{opacity:1}
.beats .fail{background:var(--down)}
.axis{display:flex;align-items:center;gap:8px;margin:8px 0 6px;color:var(--muted);font-size:.8em}
.axis hr{flex:1;margin:0;border:0;border-top:1px solid var(--line)}
.state{margin:0;color:var(--muted);font-size:.85em}
footer{margin:24px 0 0;color:var(--muted);font-size:.8em;text-align:center}
footer a{color:inherit}
.up .state{color:var(--up)}
.down .state{color:var(--down)}
";

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
            let recent = &mine[mine.len().saturating_sub(BEATS)..];
            let beats: String = "<span></span>".repeat(BEATS - recent.len())
                + &recent
                    .iter()
                    .map(|c| {
                        if c.ok {
                            "<span class=\"ok\"></span>"
                        } else {
                            "<span class=\"fail\"></span>"
                        }
                    })
                    .collect::<String>();
            format!(
                "<article class=\"{class}\"><div class=\"head\"><b>{}</b><i title=\"{state}\">{mark}</i></div><small>{}</small><div class=\"beats\">{beats}</div><div class=\"axis\"><span>{BEATS} checks ago</span><hr><span>{} uptime 24h</span><hr><span>now</span></div><p class=\"state\">{state} · {latency} · {checked}</p></article>\n",
                escape(&t.name),
                escape(&t.url),
                cache::uptime(&mine, now.saturating_sub(86400)),
            )
        })
        .collect();
    let banner = match down {
        0 => "<p class=\"banner\">All systems up</p>".to_string(),
        n => format!(
            "<p class=\"banner down\">{n} of {} targets down</p>",
            targets.len()
        ),
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>tunnelcheck status</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n<main>\n<header><h1>Current status</h1><p>Last {BEATS} checks per target, uptime over 24h</p></header>\n{banner}\n<section class=\"grid\">\n{cards}</section>\n<footer>Generated {} by tunnelcheck v{}<br><a href=\"{}\">Source</a></footer>\n</main>\n</body>\n</html>\n",
        utc(now),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY"),
    )
}

fn utc(secs: u64) -> String {
    let secs = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::gmtime_r(&secs, &mut tm) }.is_null() {
        return "-".into();
    }
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
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn renders_state_uptime_and_escapes() {
        let cfg: Config = toml::from_str(
            r#"
            [[target]]
            name = "<web> & co"
            url = "https://example.com/?a=1&b=\"2\""
            [[target]]
            name = "db"
            url = "tcp://db:5432"
            "#,
        )
        .unwrap();
        let checks: Vec<Check> = r#"{"name":"<web> & co","ok":false,"ms":5000,"ts":0}
{"name":"<web> & co","ok":false,"ms":5000,"ts":99000}
{"name":"db","ok":true,"ms":12,"ts":99000}
{"name":"<web> & co","ok":true,"ms":120,"ts":99500}
{"name":"db","ok":false,"ms":5000,"ts":99900}"#
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let html = render(&cfg.targets, &checks, 100_000);
        assert!(html.contains(
            "<article class=\"up\"><div class=\"head\"><b>&lt;web&gt; &amp; co</b><i title=\"UP\">✓</i></div><small>https://example.com/?a=1&amp;b=&quot;2&quot;</small>"
        ));
        assert!(html.contains(
            "<article class=\"down\"><div class=\"head\"><b>db</b><i title=\"DOWN\">✕</i></div><small>tcp://db:5432</small>"
        ));
        assert!(html.contains("<span class=\"ok\"></span><span class=\"fail\"></span></div>"));
        assert_eq!(html.matches("<span>50.00% uptime 24h</span>").count(), 2);
        assert!(html.contains("UP · 120ms · 8m ago"));
        assert!(html.contains("DOWN · - · 1m ago"));
        assert!(html.contains("1 of 2 targets down"));
        assert!(html.contains("Generated 1970-01-02 03:46 UTC by tunnelcheck v"));
        assert!(!html.contains("<web>"));
    }
}
