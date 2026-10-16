use metin_core::{Session, duration};
#[test]
fn paused_time_is_excluded_and_snapshots_remain_independent() {
    let mut session = Session::default();
    session.advance(10., true);
    session.advance(13., true);
    let before = session.snapshot();
    session.pause(None);
    session.advance(100., false);
    session.advance(101., true);
    session.advance(105., true);
    assert_eq!(before.active_seconds, 3.);
    assert_eq!(session.snapshot().active_seconds, 7.);
    session.reset();
    assert_eq!(session.snapshot().active_seconds, 0.);
}
#[test]
fn explicit_pause_accounts_for_last_active_interval() {
    let mut session = Session::default();
    session.advance(10., true);
    session.pause(Some(12.5));
    session.advance(100., false);
    assert_eq!(session.snapshot().active_seconds, 2.5);
}
#[test]
fn duration_formats_hours_and_clamps_negative_time() {
    assert_eq!(duration(3661.), "01:01:01");
    assert_eq!(duration(-1.), "00:00:00");
    assert_eq!(duration(14400.), "04:00:00");
}
