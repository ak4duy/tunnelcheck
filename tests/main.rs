use super::{Alert, State};
use std::time::{Duration, Instant};

#[test]
fn alerts_on_down_and_recovery_only() {
    let t = Instant::now();
    let mut s = State::default();
    assert_eq!(s.step(true, t, 1, None, false), (false, None));
    assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
    assert_eq!(s.step(false, t, 1, None, false), (false, None));
    assert_eq!(s.step(true, t, 1, None, false), (true, Some(Alert::Up)));
    assert_eq!(s.step(true, t, 1, None, false), (false, None));
}

#[test]
fn waits_for_consecutive_failures() {
    let t = Instant::now();
    let mut s = State::default();
    assert_eq!(s.step(false, t, 3, None, false), (false, None));
    assert_eq!(s.step(false, t, 3, None, false), (false, None));
    assert_eq!(s.step(true, t, 3, None, false), (false, None));
    assert_eq!(s.step(false, t, 3, None, false), (false, None));
    assert_eq!(s.step(false, t, 3, None, false), (false, None));
    assert_eq!(s.step(false, t, 3, None, false), (true, Some(Alert::Down)));
}

#[test]
fn reminds_while_down() {
    let t = Instant::now();
    let r = Some(Duration::from_secs(60));
    let mut s = State::default();
    assert_eq!(s.step(false, t, 1, r, false), (true, Some(Alert::Down)));
    let later = t + Duration::from_secs(30);
    assert_eq!(s.step(false, later, 1, r, false), (false, None));
    let later = t + Duration::from_secs(60);
    assert_eq!(
        s.step(false, later, 1, r, false),
        (false, Some(Alert::StillDown))
    );
}

#[test]
fn quiet_hours_hold_down_alert_until_they_end() {
    let t = Instant::now();
    let mut s = State::default();
    assert_eq!(s.step(false, t, 1, None, true), (true, None));
    assert_eq!(s.step(false, t, 1, None, false), (false, Some(Alert::Down)));

    let mut s = State::default();
    assert_eq!(s.step(false, t, 1, None, true), (true, None));
    assert_eq!(s.step(true, t, 1, None, true), (true, None));
    assert_eq!(s.step(true, t, 1, None, false), (false, None));
}

#[test]
fn quiet_hours_hold_recovery_until_they_end() {
    let t = Instant::now();
    let mut s = State::default();
    assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
    assert_eq!(s.step(true, t, 1, None, true), (true, None));
    assert_eq!(s.step(true, t, 1, None, true), (false, None));
    assert_eq!(s.step(true, t, 1, None, false), (false, Some(Alert::Up)));
    assert_eq!(s.step(true, t, 1, None, false), (false, None));
}

#[test]
fn flap_during_quiet_hours_sends_nothing_new() {
    let t = Instant::now();
    let mut s = State::default();
    assert_eq!(s.step(false, t, 1, None, false), (true, Some(Alert::Down)));
    assert_eq!(s.step(true, t, 1, None, true), (true, None));
    assert_eq!(s.step(false, t, 1, None, true), (true, None));
    assert_eq!(s.step(false, t, 1, None, false), (false, None));
}
