mod cache;
mod check;
mod config;
mod notify;
mod status;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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

    /// Validate the config (unknown keys, duplicate names, bad URLs) and exit
    #[arg(long)]
    check_config: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Write a static HTML status page from recorded checks
    Status {
        /// Path of the HTML file to write
        #[arg(long, value_name = "PATH", default_value = "status.html")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.history {
        return cache::show_history(cli.only.as_deref(), 20);
    }

    let path = cli.config.clone().unwrap_or_else(config::default_path);

    if cli.check_config {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let problems = config::problems(&text).context("parsing config")?;
        if problems.is_empty() {
            println!("{}: OK", path.display());
            return Ok(());
        }
        for p in &problems {
            eprintln!("{p}");
        }
        process::exit(1);
    }
    let (mut cfg, mut targets) = load(&path, cli.only.as_deref())?;
    let mut mention = mention_of(&cfg);

    if let Some(Command::Status { out }) = &cli.command {
        status::write_page(&targets, out)?;
        println!("wrote {}", out.display());
        return Ok(());
    }

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
    let streaks = cache::fail_streaks();
    let mut states: Vec<State> = targets
        .iter()
        .map(|t| State::restore(known.get(&t.name).copied(), streaks.get(&t.name).copied()))
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
            "{:<w$}    {:<6}    {:>8}      DETAIL",
            "NAME", "STATUS", "TIME"
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

            let quiet = t
                .quiet_hours
                .or(cfg.quiet_hours)
                .is_some_and(|q| q.contains(local_minute_of_day()));
            let remind = t
                .remind_every_secs
                .or(cfg.remind_every_secs)
                .map(|s| Duration::from_secs(s.get()));
            let (changed, alert) = states[i].step(
                r.ok,
                Instant::now(),
                t.failures_before_alert
                    .unwrap_or(cfg.failures_before_alert)
                    .get(),
                remind,
                quiet,
            );
            if changed || alert.is_some() {
                let color: u32 = if r.ok { 0x2ECC71 } else { 0xE74C3C };
                let title = match alert {
                    Some(Alert::StillDown) => format!("🔴 {} is still down", r.name),
                    _ if r.ok => format!("🟢 {} is back up", r.name),
                    _ => format!("🔴 {} is down", r.name),
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
                if changed
                    && let Err(e) =
                        cache::cache_state(&r.name, r.ok, &format!("{title}: {outcome}"))
                {
                    eprintln!("cache write failed for {}: {e}", r.name);
                }
                if let (Some(_), Some(url)) = (alert, &cfg.webhook_url) {
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
                        .zip(&states)
                        .map(|(t, s)| (t.name.clone(), *s))
                        .collect();
                    states = new_targets
                        .iter()
                        .map(|t| by_name.get(&t.name).copied().unwrap_or_default())
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

fn local_minute_of_day() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
        return 0;
    }
    (tm.tm_hour * 60 + tm.tm_min) as u32
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Alert {
    Down,
    StillDown,
    Up,
}

#[derive(Clone, Copy, Default)]
struct State {
    down: bool,
    fails: u64,
    alerted_at: Option<Instant>,
}

impl State {
    fn restore(last_ok: Option<bool>, fails: Option<u64>) -> State {
        let down = last_ok == Some(false);
        State {
            down,
            fails: fails.unwrap_or(0),
            alerted_at: down.then(Instant::now),
        }
    }

    fn step(
        &mut self,
        ok: bool,
        now: Instant,
        failures_before_alert: u64,
        remind: Option<Duration>,
        quiet: bool,
    ) -> (bool, Option<Alert>) {
        if ok {
            self.fails = 0;
            let changed = std::mem::replace(&mut self.down, false);
            if quiet || self.alerted_at.is_none() {
                return (changed, None);
            }
            self.alerted_at = None;
            return (changed, Some(Alert::Up));
        }
        self.fails += 1;
        let changed = !self.down && self.fails >= failures_before_alert;
        self.down |= changed;
        if !self.down || quiet {
            return (changed, None);
        }
        let alert = match self.alerted_at {
            None => Some(Alert::Down),
            Some(at) if remind.is_some_and(|r| now.duration_since(at) >= r) => {
                Some(Alert::StillDown)
            }
            _ => None,
        };
        if alert.is_some() {
            self.alerted_at = Some(now);
        }
        (changed, alert)
    }
}

#[cfg(test)]
mod tests {
    use super::{Alert, State};
    use std::time::{Duration, Instant};

    #[test]
    fn alerts_on_down_and_recovery_only() {
        let t = Instant::now();
        let mut s = State::default();
        assert_eq!(s.step(true, t, 1, None, false), (false, None));
        assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
        assert_eq!(s.step(false, t, 1, None, false), (false, None));
        assert_eq!(s.step(true, t, 1, None, false), (true, Some(Alert::Up)));
        assert_eq!(s.step(true, t, 1, None, false), (false, None));
    }

    #[test]
    fn waits_for_consecutive_failures() {
        let t = Instant::now();
        let mut s = State::default();
        assert_eq!(s.step(false, t, 3, None, false), (false, None));
        assert_eq!(s.step(false, t, 3, None, false), (false, None));
        assert_eq!(s.step(true, t, 3, None, false), (false, None));
        assert_eq!(s.step(false, t, 3, None, false), (false, None));
        assert_eq!(s.step(false, t, 3, None, false), (false, None));
        assert_eq!(s.step(false, t, 3, None, false), (true, Some(Alert::Down)));
    }

    #[test]
    fn reminds_while_down() {
        let t = Instant::now();
        let r = Some(Duration::from_secs(60));
        let mut s = State::default();
        assert_eq!(s.step(false, t, 1, r, false), (true, Some(Alert::Down)));
        let later = t + Duration::from_secs(30);
        assert_eq!(s.step(false, later, 1, r, false), (false, None));
        let later = t + Duration::from_secs(60);
        assert_eq!(
            s.step(false, later, 1, r, false),
            (false, Some(Alert::StillDown))
        );
    }

    #[test]
    fn quiet_hours_hold_down_alert_until_they_end() {
        let t = Instant::now();
        let mut s = State::default();
        assert_eq!(s.step(false, t, 1, None, true), (true, None));
        assert_eq!(s.step(false, t, 1, None, false), (false, Some(Alert::Down)));

        let mut s = State::default();
        assert_eq!(s.step(false, t, 1, None, true), (true, None));
        assert_eq!(s.step(true, t, 1, None, true), (true, None));
        assert_eq!(s.step(true, t, 1, None, false), (false, None));
    }

    #[test]
    fn quiet_hours_hold_recovery_until_they_end() {
        let t = Instant::now();
        let mut s = State::default();
        assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
        assert_eq!(s.step(true, t, 1, None, true), (true, None));
        assert_eq!(s.step(true, t, 1, None, true), (false, None));
        assert_eq!(s.step(true, t, 1, None, false), (false, Some(Alert::Up)));
        assert_eq!(s.step(true, t, 1, None, false), (false, None));
    }

    #[test]
    fn flap_during_quiet_hours_sends_nothing_new() {
        let t = Instant::now();
        let mut s = State::default();
        assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
        assert_eq!(s.step(true, t, 1, None, true), (true, None));
        assert_eq!(s.step(false, t, 1, None, true), (true, None));
        assert_eq!(s.step(false, t, 1, None, false), (false, None));
    }
}
