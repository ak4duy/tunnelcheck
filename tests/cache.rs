use super::*;

#[test]
fn uptime_counts_only_window() {
    let c = |ok, ts| Check {
        name: "a".into(),
        ok,
        ms: 1,
        ts,
    };
    let checks = [c(false, 10), c(true, 100), c(true, 200), c(false, 300)];
    let refs: Vec<&Check> = checks.iter().collect();
    assert_eq!(uptime(&refs, 0), "50.00%");
    assert_eq!(uptime(&refs, 100), "66.67%");
    assert_eq!(uptime(&refs, 1000), "-");
}

#[test]
fn sparkline_scales_min_to_max() {
    assert_eq!(sparkline(&[10, 45, 80]), "▁▄█");
    assert_eq!(sparkline(&[5, 5]), "▁▁");
    assert_eq!(sparkline(&[]), "");
}
