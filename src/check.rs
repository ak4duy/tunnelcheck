use crate::config::Target;
use std::{
    io::ErrorKind,
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

pub struct Outcome {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub elapsed: Duration,
}

pub fn check(t: &Target, default_timeout_secs: u64, default_retries: u64) -> Outcome {
    let retries = t.retries.unwrap_or(default_retries);
    let mut outcome = attempt(t, default_timeout_secs);
    for _ in 0..retries {
        if outcome.ok {
            break;
        }
        outcome = attempt(t, default_timeout_secs);
    }
    outcome
}

fn attempt(t: &Target, default_timeout_secs: u64) -> Outcome {
    let timeout = Duration::from_secs(t.timeout_secs.unwrap_or(default_timeout_secs));
    let start = Instant::now();

    if let Some(addr) = t.url.strip_prefix("tcp://") {
        return match addr.to_socket_addrs().map(|mut a| a.next()) {
            Ok(Some(sa)) => match TcpStream::connect_timeout(&sa, timeout) {
                Ok(_) => pass(t, "connected".into(), start),
                Err(e) if e.kind() == ErrorKind::TimedOut => fail(t, "timeout".into(), start),
                Err(e) => fail(t, format!("error: {e}"), start),
            },
            Ok(None) => fail(t, format!("resolve: no address for {addr}"), start),
            Err(e) => fail(t, format!("resolve: {e}"), start),
        };
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(t, format!("client: {e}"), start),
    };

    match client.get(&t.url).send() {
        Ok(resp) if resp.status().as_u16() == t.expect_status => {
            let status = resp.status().to_string();
            match &t.keyword {
                None => pass(t, status, start),
                Some(k) => match resp.text() {
                    Ok(body) if body.contains(k.as_str()) => pass(t, status, start),
                    Ok(_) => fail(t, format!("keyword {k:?} not found"), start),
                    Err(e) => fail(t, format!("body: {e}"), start),
                },
            }
        }
        Ok(resp) => fail(t, format!("HTTP {}", resp.status()), start),
        Err(e) if e.is_timeout() => fail(t, "timeout".into(), start),
        Err(e) => fail(t, format!("error: {e}"), start),
    }
}

fn pass(t: &Target, detail: String, start: Instant) -> Outcome {
    Outcome {
        name: t.name.clone(),
        ok: true,
        detail,
        elapsed: start.elapsed(),
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
