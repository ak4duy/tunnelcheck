use crate::check::Outcome;
use serde::Serialize;

#[derive(Serialize)]
struct Message<'a> {
    content: &'a str,
}

/// Errors are printed, not returned: a broken webhook must not stop monitoring.
pub fn discord(url: &str, r: &Outcome) {
    let content = format!("🔴 **{}** is DOWN: {}", r.name, r.detail);
    let res = reqwest::blocking::Client::new()
        .post(url)
        .json(&Message { content: &content })
        .send()
        .and_then(|resp| resp.error_for_status());
    if let Err(e) = res {
        eprintln!("webhook failed for {}: {e}", r.name);
    }
}
