## [0.2.2] - 2026-10-28

### Features

- feat(status): add footer with version, timestamp and source link e3e6576
- feat(status): add html status page subcommand 58430b7

### Refactor

- refactor(cache): expose helpers for the status page 2c9beb5

#### Full Changelog: https://github.com/ak4duy/tunnelcheck/compare/v0.2.0...v0.2.2

### Miscellaneous

- chore(ci): remove Windows latest build d205c49

## [0.2.0] - 2026-10-26

### Features

- feat(notify): add Discord mention support to webhook alerts a1ccf30 
- feat(watch): reload config changes without restarting a20aab1 
- feat(cache): store check results in checks.jsonl 062a5b4 
- feat(check): add icmp and dns target types 6177621 
- feat(config): add failure threshold, reminder and quiet hours settings a438639 
- feat(alerts): alert after consecutive failures, remind while down, honor quiet hours 70b9911 
- feat(config): add --check-config to catch typos and duplicate target names 2667266 

### Documentation

- docs(readme): document config reload in watch mode 3e8c562 
- docs(readme): rename checks cache file to checks.jsonl 83d12ca 
- docs(readme): document alert threshold, reminders and quiet hours 2637ad6 

### Miscellaneous

- ci: add Rust CI workflow for fmt, build, and clippy 300be6f 
- chore(ci): clippy check e23b041 
- ci(release): build binaries and publish checksums after release 292b5ca 
- ci(rust): add cargo test d600f03 
- ci(github): remove unused GTK and Adwaita dependencies from CI workflow ea2889a 


#### Full Changelog: https://github.com/ak4duy/tunnelcheck/compare/v0.1.0...v0.2.0
## [0.1.0] - 2026-10-21

### Features

- feat: add per-target intervals and Discord down alerts 12ba7b7 
- feat: add global and per-target retries for failed checks 57665f6 
- feat: send Discord message when a target recovers e146729 
- feat: add --test-webhook flag f031ea8 
- feat: add systemd user unit for watch mode 9d6e48a 
- feat: record alert state transitions to cache 8da1acd 
- feat(notify): send alerts as discord embeds 3de6c6d 
- feat(cache): add --history flag to browse event logs 64bb3f5 
- feat: add uptime status dashboard, TCP checks, and keyword matching 095c4a3 
- feat(cache): prune check history older than 30 days on startup 6214bd4 
- feat(check): fail https checks when certificate nears expiry c758b80 

### Bug Fixes

- fix: size name column to longest target name 8e5a406 
- fix: restore alert state from event log on startup 6ca6490 

### Refactor

- refactor(config)!: switch timeout from milliseconds to seconds 24dd6e7 

### Documentation

- docs: add readme 17eda85 

### Performance

- perf: check due targets concurrently f9ac2c0 

### Miscellaneous

- chore: time is now a non zero 3db2fa7 
- chore: add license, package metadata, and release workflow 343ebe0 

