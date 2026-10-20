use crate::config::Target;
use std::time::{Duration, Instant};

pub struct Outcome {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub elapsed: Duration,
}

pub fn check(t: &Target, default_timeout_ms: u64, default_retries: u64) -> Outcome {
    let retries = t.retries.unwrap_or(default_retries);
    let mut outcome = attempt(t, default_timeout_ms);
    for _ in 0..retries {
        if outcome.ok {
            break;
        }
        outcome = attempt(t, default_timeout_ms);
    }
    outcome
}

fn attempt(t: &Target, default_timeout_ms: u64) -> Outcome {
    let timeout = Duration::from_millis(t.timeout_ms.unwrap_or(default_timeout_ms));
    let start = Instant::now();

    let client = match reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(t, format!("client: {e}"), start),
    };

    match client.get(&t.url).send() {
        Ok(resp) if resp.status().as_u16() == t.expect_status => Outcome {
            name: t.name.clone(),
            ok: true,
            detail: resp.status().to_string(),
            elapsed: start.elapsed(),
        },
        Ok(resp) => fail(t, format!("HTTP {}", resp.status()), start),
        Err(e) if e.is_timeout() => fail(t, "timeout".into(), start),
        Err(e) => fail(t, format!("error: {e}"), start),
    }
}

fn fail(t: &Target, detail: String, start: Instant) -> Outcome {
    Outcome {
        name: t.name.clone(),
        ok: false,
        detail,
        elapsed: start.elapsed(),
    }
}
