use super::{QuietHours, problems};

#[test]
fn quiet_hours_parse_and_wrap_midnight() {
    let q = QuietHours::try_from("23:00-07:30".to_string()).unwrap();
    assert!(q.contains(23 * 60));
    assert!(q.contains(3 * 60));
    assert!(!q.contains(7 * 60 + 30));
    assert!(!q.contains(12 * 60));
    let day = QuietHours::try_from("09:00-17:00".to_string()).unwrap();
    assert!(day.contains(9 * 60) && !day.contains(17 * 60));
    assert!(QuietHours::try_from("25:00-07:00".to_string()).is_err());
    assert!(QuietHours::try_from("nope".to_string()).is_err());
}

#[test]
fn reports_typos_and_duplicates() {
    let text = r#"
        timout_secs = 5
        [[target]]
        name = "a"
        url = "https://a"
        expect_staus = 200
        [[target]]
        name = "a"
        url = "htps://b"
    "#;
    assert_eq!(
        problems(text).unwrap(),
        [
            "unknown key: timout_secs",
            "unknown key: target.0.expect_staus",
            "duplicate target name: a",
            "a: unsupported url \"htps://b\"",
        ]
    );
}

#[test]
fn clean_config_has_no_problems() {
    let text = "[[target]]\nname = \"a\"\nurl = \"tcp://a:22\"\n";
    assert!(problems(text).unwrap().is_empty());
}
