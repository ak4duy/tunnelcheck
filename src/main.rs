mod cache;
mod check;
mod config;
mod notify;

use anyhow::{Context, Result};
use clap::Parser;
use std::{
    collections::HashMap,
    num::NonZeroU64,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(name = "tunnelcheck", about = "Check services behind your tunnel")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Only check the target with this name
    #[arg(long)]
    only: Option<String>,

    /// Rerun every N seconds (targets can override with interval_secs)
    #[arg(long, value_name = "SECS")]
    watch: Option<NonZeroU64>,

    /// Send a test message to the webhook and exit
    #[arg(long)]
    test_webhook: bool,

    /// Browse recorded up/down history (newest first), filtered by --only
    #[arg(long)]
    history: bool,

    /// Show the uptime dashboard from recorded checks (refreshes with --watch)
    #[arg(long)]
    status: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.history {
        return cache::show_history(cli.only.as_deref(), 20);
    }

    let path = cli.config.clone().unwrap_or_else(config::default_path);
    let (mut cfg, mut targets) = load(&path, cli.only.as_deref())?;
    let mut mention = mention_of(&cfg);

    if cli.test_webhook {
        let url = cfg
            .webhook_url
            .as_deref()
            .context("no webhook_url in config")?;
        notify::discord(
            url,
            mention.as_deref(),
            "✅ tunnelcheck webhook test",
            0x2ECC71,
            &[("Status", "webhook is working")],
        )
        .context("sending test message")?;
        println!("test message sent");
        return Ok(());
    }

    if cli.status {
        let names: Vec<&str> = targets.iter().map(|t| t.name.as_str()).collect();
        loop {
            if cli.watch.is_some() {
                print!("\x1b[2J\x1b[H");
            }
            cache::show_status(&names)?;
            let Some(secs) = cli.watch else {
                return Ok(());
            };
            std::thread::sleep(Duration::from_secs(secs.get()));
        }
    }

    let mut next_due = vec![Instant::now(); targets.len()];
    if let Err(e) = cache::prune_checks(30 * 86400) {
        eprintln!("pruning check history failed: {e}");
    }
    let known = cache::last_states();
    let mut last_ok: Vec<Option<bool>> = targets
        .iter()
        .map(|t| known.get(&t.name).copied())
        .collect();

    let mut loaded_at = modified(&path);

    loop {
        let now = Instant::now();
        let mut any_fail = false;
        let w = targets
            .iter()
            .map(|t| t.name.chars().count())
            .max()
            .unwrap_or(0)
            .max(4);

        println!(
            "{:<w$}    {:<6}    {:>8}      {}",
            "NAME", "STATUS", "TIME", "DETAIL"
        );
        let due: Vec<usize> = (0..targets.len()).filter(|&i| next_due[i] <= now).collect();
        let results: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = due
                .iter()
                .map(|&i| {
                    let (t, cfg) = (&targets[i], &cfg);
                    s.spawn(move || {
                        check::check(t, cfg.timeout_secs, cfg.retries, cfg.cert_warn_days)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        for (&i, r) in due.iter().zip(results) {
            let t = &targets[i];
            println!(
                "{:<w$}    {:<6}    {:>6}ms      {}",
                r.name,
                if r.ok { "OK" } else { "FAIL" },
                r.elapsed.as_millis(),
                r.detail
            );
            if let Err(e) = cache::record_check(&r.name, r.ok, r.elapsed.as_millis() as u64) {
                eprintln!("cache write failed for {}: {e}", r.name);
            }

            if should_alert(last_ok[i], r.ok) {
                let color: u32 = if r.ok { 0x2ECC71 } else { 0xE74C3C };
                let title = if r.ok {
                    format!("🟢 {} is back up", r.name)
                } else {
                    format!("🔴 {} is down", r.name)
                };
                let unix_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let when = format!("<t:{unix_secs}:f>");
                let (label, outcome) = if r.ok {
                    ("Latency", format!("{} ms", r.elapsed.as_millis()))
                } else {
                    ("Error", r.detail.clone())
                };
                if let Err(e) = cache::cache_state(&r.name, r.ok, &format!("{title}: {outcome}")) {
                    eprintln!("cache write failed for {}: {e}", r.name);
                }
                if let Some(url) = &cfg.webhook_url {
                    let fields = [
                        ("Target", t.url.as_str()),
                        (label, &outcome),
                        ("Time", &when),
                    ];
                    if let Err(e) = notify::discord(url, mention.as_deref(), &title, color, &fields)
                    {
                        eprintln!("webhook failed for {}: {e}", r.name);
                    }
                }
            }
            last_ok[i] = Some(r.ok);
            any_fail |= !r.ok;

            if let Some(secs) = cli.watch {
                next_due[i] = now + Duration::from_secs(t.interval_secs.unwrap_or(secs).get());
            }
        }

        let Some(secs) = cli.watch else {
            if any_fail {
                process::exit(1);
            }
            return Ok(());
        };

        let wake = next_due
            .iter()
            .min()
            .copied()
            .unwrap_or(now + Duration::from_secs(secs.get()));
        while Instant::now() < wake {
            let left = wake.saturating_duration_since(Instant::now());
            std::thread::sleep(left.min(Duration::from_secs(1)));
            let seen = modified(&path);
            if seen == loaded_at {
                continue;
            }
            loaded_at = seen;
            match load(&path, cli.only.as_deref()) {
                Ok((new_cfg, new_targets)) => {
                    let by_name: HashMap<_, _> = targets
                        .iter()
                        .zip(&last_ok)
                        .map(|(t, ok)| (t.name.clone(), *ok))
                        .collect();
                    last_ok = new_targets
                        .iter()
                        .map(|t| by_name.get(&t.name).copied().flatten())
                        .collect();
                    next_due = vec![Instant::now(); new_targets.len()];
                    mention = mention_of(&new_cfg);
                    cfg = new_cfg;
                    targets = new_targets;
                    eprintln!("config reloaded");
                    break;
                }
                Err(e) => eprintln!("config reload failed, keeping previous: {e:#}"),
            }
        }
    }
}

fn load(path: &Path, only: Option<&str>) -> Result<(config::Config, Vec<config::Target>)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: config::Config = toml::from_str(&text).context("parsing config")?;
    let targets = cfg
        .targets
        .iter()
        .filter(|t| only.is_none_or(|n| n == t.name))
        .cloned()
        .collect();
    Ok((cfg, targets))
}

fn mention_of(cfg: &config::Config) -> Option<String> {
    let message = cfg.mention_message.as_deref().unwrap_or_default();
    cfg.mention
        .as_deref()
        .map(|id| notify::mention(id, message))
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn should_alert(prev_ok: Option<bool>, ok: bool) -> bool {
    prev_ok.unwrap_or(true) != ok
}

#[cfg(test)]
mod tests {
    use super::should_alert;

    #[test]
    fn alerts_on_down_and_recovery_only() {
        assert!(should_alert(None, false));
        assert!(!should_alert(None, true));
        assert!(should_alert(Some(true), false));
        assert!(should_alert(Some(false), true));
        assert!(!should_alert(Some(false), false));
        assert!(!should_alert(Some(true), true));
    }
}
