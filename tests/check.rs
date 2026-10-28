use super::*;

fn target(url: &str) -> Target {
    Target {
        name: "t".into(),
        url: url.into(),
        expect_status: 200,
        timeout_secs: None,
        retries: None,
        keyword: None,
        interval_secs: None,
        failures_before_alert: None,
        remind_every_secs: None,
        quiet_hours: None,
    }
}

#[test]
fn dns_resolves_localhost() {
    let o = attempt(&target("dns://localhost"), 5, 0);
    assert!(o.ok, "{}", o.detail);
}

#[test]
fn dns_fails_on_unknown_name() {
    assert!(!attempt(&target("dns://does-not-exist.invalid"), 5, 0).ok);
}
