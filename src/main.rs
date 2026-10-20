mod check;
mod config;
mod notify;

use anyhow::{Context, Result};
use clap::Parser;
use std::{num::NonZeroU64, path::PathBuf, process, time::{Duration, Instant}};

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let path = cli.config.clone().unwrap_or_else(config::default_path);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let cfg: config::Config = toml::from_str(&text).context("parsing config")?;

    let targets: Vec<_> = cfg
        .targets
        .iter()
        .filter(|t| cli.only.as_ref().map_or(true, |n| *n == t.name))
        .cloned()
        .collect();

    let mut next_due = vec![Instant::now(); targets.len()];
    let mut last_ok: Vec<Option<bool>> = vec![None; targets.len()];

    loop {
        let now = Instant::now();
        let mut any_fail = false;

        println!("{:<14} {:<6} {:>8}  {}", "NAME", "STATUS", "TIME", "DETAIL");
        for (i, t) in targets.iter().enumerate() {
            if next_due[i] > now {
                continue;
            }
            let r = check::check(t, cfg.timeout_ms);
            println!(
                "{:<14} {:<6} {:>6}ms  {}",
                r.name,
                if r.ok { "OK" } else { "FAIL" },
                r.elapsed.as_millis(),
                r.detail
            );

            if should_alert(last_ok[i], r.ok) {
                if let Some(url) = &cfg.webhook_url {
                    notify::discord(url, &r);
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

/// Alert on the first failure only, not on every failed check while down.
fn should_alert(prev_ok: Option<bool>, ok: bool) -> bool {
    !ok && prev_ok != Some(false)
}

#[cfg(test)]
mod tests {
    use super::should_alert;

    #[test]
    fn alerts_only_on_transition_to_down() {
        assert!(should_alert(None, false));
        assert!(should_alert(Some(true), false));
        assert!(!should_alert(Some(false), false));
        assert!(!should_alert(Some(true), true));
        assert!(!should_alert(Some(false), true));
    }
}
