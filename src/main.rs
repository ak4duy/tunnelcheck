mod check;
mod config;
mod notify;

use anyhow::{Context, Result};
use clap::Parser;
use std::{
    num::NonZeroU64,
    path::PathBuf,
    process,
    time::{Duration, Instant},
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let path = cli.config.clone().unwrap_or_else(config::default_path);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: config::Config = toml::from_str(&text).context("parsing config")?;

    if cli.test_webhook {
        let url = cfg
            .webhook_url
            .as_deref()
            .context("no webhook_url in config")?;
        notify::discord(url, "✅ tunnelcheck webhook test").context("sending test message")?;
        println!("test message sent");
        return Ok(());
    }

    let targets: Vec<_> = cfg
        .targets
        .iter()
        .filter(|t| cli.only.as_ref().is_none_or(|n| *n == t.name))
        .cloned()
        .collect();

    let w = targets
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);

    let mut next_due = vec![Instant::now(); targets.len()];
    let mut last_ok: Vec<Option<bool>> = vec![None; targets.len()];

    loop {
        let now = Instant::now();
        let mut any_fail = false;

        println!("{:<w$} {:<6} {:>8}  {}", "NAME", "STATUS", "TIME", "DETAIL");
        let due: Vec<usize> = (0..targets.len()).filter(|&i| next_due[i] <= now).collect();
        let results: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = due
                .iter()
                .map(|&i| {
                    let (t, cfg) = (&targets[i], &cfg);
                    s.spawn(move || check::check(t, cfg.timeout_secs, cfg.retries))
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        for (&i, r) in due.iter().zip(results) {
            let t = &targets[i];
            println!(
                "{:<w$} {:<6} {:>6}ms  {}",
                r.name,
                if r.ok { "OK" } else { "FAIL" },
                r.elapsed.as_millis(),
                r.detail
            );

            if let (true, Some(url)) = (should_alert(last_ok[i], r.ok), &cfg.webhook_url) {
                let msg = if r.ok {
                    format!("🟢 **{}** is back UP ({}ms)", r.name, r.elapsed.as_millis())
                } else {
                    format!("🔴 **{}** is DOWN: {}", r.name, r.detail)
                };
                if let Err(e) = notify::discord(url, &msg) {
                    eprintln!("webhook failed for {}: {e}", r.name);
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
        std::thread::sleep(wake.saturating_duration_since(Instant::now()));
    }
}

fn should_alert(prev_ok: Option<bool>, ok: bool) -> bool {
    match prev_ok {
        None => !ok,
        Some(prev) => prev != ok,
    }
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
