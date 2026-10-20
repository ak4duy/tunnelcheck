mod check;
mod config;

use anyhow::{Context, Result};
use clap::Parser;
use std::{path::PathBuf, process, time::Duration};

#[derive(Parser)]
#[command(name = "tunnelcheck", about = "Check services behind your tunnel")]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Only check the target with this name
    #[arg(long)]
    only: Option<String>,

    /// Rerun every N seconds
    #[arg(long, value_name = "SECS")]
    watch: Option<u64>,
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

    loop {
        let results: Vec<_> = targets
            .iter()
            .map(|t| check::check(t, cfg.timeout_ms))
            .collect();

        println!("{:<14} {:<6} {:>8}  {}", "NAME", "STATUS", "TIME", "DETAIL");
        for r in &results {
            println!(
                "{:<14} {:<6} {:>6}ms  {}",
                r.name,
                if r.ok { "OK" } else { "FAIL" },
                r.elapsed.as_millis(),
                r.detail
            );
        }

        if let Some(secs) = cli.watch {
            std::thread::sleep(Duration::from_secs(secs));
            continue;
        }

        if results.iter().any(|r| !r.ok) {
            process::exit(1);
        }
        return Ok(());
    }
}
