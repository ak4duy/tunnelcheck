# tunnelcheck

A small uptime monitor for the command line, configured with one TOML file. Think Uptime Kuma without the web app.

- HTTP(S) checks with expected status code and optional keyword match
- TCP port checks (`tcp://host:port`)
- Ping checks (`icmp://host`, uses the system `ping`)
- DNS resolution checks (`dns://name`)
- TLS certificate expiry warnings
- Discord alerts when a target goes down or comes back up, with a failure threshold, reminders and quiet hours
- Terminal dashboard with heartbeat bar, 24h/30d uptime and latency sparkline
- Up/down event history

## Install

```sh
cargo install --path .
```

## Configure

The config lives at `~/.config/tunnelcheck/config.toml` (override with `-c`). See [`contrib/config.example.toml`](contrib/config.example.toml).

```toml
timeout_secs = 5      # per-check timeout (default 5)
retries = 1           # extra attempts before a check counts as failed (default 0)
cert_warn_days = 7    # fail HTTPS checks whose cert expires sooner (default 7, 0 disables)
webhook_url = "https://discord.com/api/webhooks/..."
failures_before_alert = 3   # consecutive failures before a target counts as down (default 1)
remind_every_secs = 3600    # resend the down alert while it stays down (default: never)
quiet_hours = "23:00-07:00" # local time; down alerts wait until it ends (default: none)

[[target]]
name = "website"
url = "https://example.com"

[[target]]
name = "api-health"
url = "https://example.com/health"
expect_status = 200   # default 200
keyword = "ok"        # response body must contain this
interval_secs = 60    # overrides --watch for this target
timeout_secs = 10     # overrides the global timeout
retries = 2           # overrides the global retries
failures_before_alert = 5   # overrides the global alert settings
remind_every_secs = 600
quiet_hours = "00:00-06:00"

[[target]]
name = "ssh"
url = "tcp://example.com:22"

[[target]]
name = "gateway"
url = "icmp://192.168.1.1"

[[target]]
name = "dns"
url = "dns://example.com"
```

## Usage

```sh
tunnelcheck                    # check everything once, exit 1 if anything failed
tunnelcheck --only website     # check one target
tunnelcheck --watch 300        # keep checking every 300s, alert on changes
tunnelcheck --status           # dashboard from recorded checks
tunnelcheck --status --watch 10  # live dashboard, redraws every 10s
tunnelcheck status --out status.html  # write a static HTML status page
tunnelcheck --history          # browse up/down events, newest first
tunnelcheck --test-webhook     # send a test message to Discord
tunnelcheck --check-config     # report unknown keys, duplicate names, bad URLs; exit 1 if any
```

`--status` only reads the recorded history and runs no checks. It is meant to be used alongside a running `--watch` process:

```
NAME         NOW   LAST CHECKS                         24H      30D      AVG  LATENCY
website      UP                              ████  100.00%  100.00%    174ms  █▁▃▁
ssh          DOWN                            ▁▁▁▁    0.00%    0.00%        -
```

Colors are used when printing to a terminal; set `NO_COLOR` to turn them off.

### HTML status page

`tunnelcheck status --out <path>` writes a single self-contained HTML file (default `status.html`) from the recorded checks: current up/down state, 24h uptime, last latency and time since the last check for each target. It has no scripts or external assets, follows the system light/dark theme and fits phone screens, so any static file server can host it. Like `--status` it runs no checks; regenerate it periodically (for example from cron) while `--watch` is running. `--only` limits it to one target.

Quiet hours hold back every Discord message. When they end, you get the alert for the target's current state: "down" if it went down and is still down, "back up" if it recovered from an outage you were alerted about. An outage that starts and ends inside quiet hours sends nothing. A "back up" message is only sent for outages that were alerted.

## Run as a service

A systemd user unit is included:

```sh
cp contrib/tunnelcheck.service ~/.config/systemd/user/
systemctl --user enable --now tunnelcheck
```

It runs `tunnelcheck --watch 300` from `~/.cargo/bin`. Edits to the config file are picked up automatically in watch mode; a config that fails to parse is ignored and the previous one stays active.

## Data

Stored in `~/.cache/tunnelcheck/`:

- `checks.jsonl`: every check result, used by `--status` and `status`. Entries older than 30 days are removed on startup.
- `events.json`: up/down transitions, used by `--history` and to avoid repeating alerts after a restart.
