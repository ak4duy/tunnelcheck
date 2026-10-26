use crate::config::Target;
use std::{
    io::ErrorKind,
    net::{TcpStream, ToSocketAddrs},
    process::Command,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use x509_parser::prelude::{FromDer, X509Certificate};

pub struct Outcome {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub elapsed: Duration,
}

pub fn check(
    t: &Target,
    default_timeout_secs: u64,
    default_retries: u64,
    cert_warn_days: i64,
) -> Outcome {
    let retries = t.retries.unwrap_or(default_retries);
    let mut outcome = attempt(t, default_timeout_secs, cert_warn_days);
    for _ in 0..retries {
        if outcome.ok {
            break;
        }
        outcome = attempt(t, default_timeout_secs, cert_warn_days);
    }
    outcome
}

fn cert_days_left(resp: &reqwest::blocking::Response) -> Option<i64> {
    let der = resp
        .extensions()
        .get::<reqwest::tls::TlsInfo>()?
        .peer_certificate()?;
    let (_, cert) = X509Certificate::from_der(der).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((cert.validity().not_after.timestamp() - now) / 86400)
}

fn attempt(t: &Target, default_timeout_secs: u64, cert_warn_days: i64) -> Outcome {
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

    if let Some(host) = t.url.strip_prefix("icmp://") {
        let secs = timeout.as_secs().max(1).to_string();
        return match Command::new("ping")
            .args(["-n", "-c", "1", "-W", &secs, "--", host])
            .output()
        {
            Ok(o) if o.status.success() => pass(t, "reply".into(), start),
            Ok(o) => match String::from_utf8_lossy(&o.stderr).trim() {
                "" => fail(t, "no reply".into(), start),
                err => fail(t, err.to_string(), start),
            },
            Err(e) => fail(t, format!("ping: {e}"), start),
        };
    }

    if let Some(name) = t.url.strip_prefix("dns://") {
        let (tx, rx) = mpsc::channel();
        let host = name.to_string();
        std::thread::spawn(move || {
            let _ = tx.send((host.as_str(), 0).to_socket_addrs().map(|mut a| a.next()));
        });
        return match rx.recv_timeout(timeout) {
            Ok(Ok(Some(sa))) => pass(t, sa.ip().to_string(), start),
            Ok(Ok(None)) => fail(t, format!("resolve: no address for {name}"), start),
            Ok(Err(e)) => fail(t, format!("resolve: {e}"), start),
            Err(_) => fail(t, "timeout".into(), start),
        };
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(timeout)
        .tls_info(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => return fail(t, format!("client: {e}"), start),
    };

    match client.get(&t.url).send() {
        Ok(resp) if resp.status().as_u16() == t.expect_status => {
            let mut status = resp.status().to_string();
            if let Some(days) = cert_days_left(&resp) {
                if days < cert_warn_days {
                    return fail(t, format!("certificate expires in {days}d"), start);
                }
                status = format!("{status}, cert {days}d");
            }
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

#[cfg(test)]
mod tests {
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
}
