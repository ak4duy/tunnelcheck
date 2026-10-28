use super::*;
use crate::config::Config;

#[test]
fn renders_state_uptime_and_escapes() {
    let cfg: Config = toml::from_str(
        r#"
            [[target]]
            name = "<web> & co"
            url = "https://example.com/?a=1&b=\"2\""
            [[target]]
            name = "db"
            url = "tcp://db:5432"
            "#,
    )
    .unwrap();
    let checks: Vec<Check> = r#"{"name":"<web> & co","ok":false,"ms":5000,"ts":0}
{"name":"<web> & co","ok":false,"ms":5000,"ts":99000}
{"name":"db","ok":true,"ms":12,"ts":99000}
{"name":"<web> & co","ok":true,"ms":120,"ts":99500}
{"name":"db","ok":false,"ms":5000,"ts":99900}"#
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let html = render(&cfg.targets, &checks, 100_000);
    let cards: Vec<&str> = html.split("<article ").skip(1).collect();
    let tip = |card: &str, date: &str| {
        let rest = card
            .split(&format!("<strong>{date}</strong>"))
            .nth(1)
            .unwrap();
        rest[..rest.find("</div>").unwrap()].trim().to_string()
    };
    let (web, db) = (cards[0], cards[1]);
    assert!(web.starts_with(r#"class="up">"#));
    assert!(web.contains("<b>&lt;web&gt; &amp; co</b>"));
    assert!(web.contains(r#"<i title="UP">✓</i>"#));
    assert!(web.contains("<small>https://example.com/?a=1&amp;b=&quot;2&quot;</small>"));
    assert!(db.starts_with(r#"class="down">"#));
    assert!(db.contains("<b>db</b>"));
    assert!(db.contains(r#"<i title="DOWN">✕</i>"#));
    assert!(db.contains("<small>tcp://db:5432</small>"));
    assert!(web.contains(r#"<div class="fail" tabindex="0">"#));
    assert!(tip(web, "1 Jan 1970").contains("<span>24 hrs</span>"));
    assert!(tip(web, "2 Jan 1970").contains("<span>3 hrs 38 mins</span>"));
    assert!(tip(web, "2 Jan 1970").ends_with("<p>1 failed check</p>"));
    assert!(tip(db, "2 Jan 1970").contains("<span>1 min</span>"));
    assert_eq!(
        tip(db, "1 Jan 1970"),
        "<p>No data recorded on this day.</p>"
    );
    assert!(web.contains("<span>33.33% uptime 30d</span>"));
    assert!(db.contains("<span>50.00% uptime 30d</span>"));
    assert!(web.contains("UP · 120ms · 8m ago"));
    assert!(db.contains("DOWN · - · 1m ago"));
    assert!(html.contains("1 of 2 targets down"));
    assert!(html.contains("Generated 1970-01-02 03:46 UTC by tunnelcheck v"));
    assert!(!html.contains("<web>"));
}
