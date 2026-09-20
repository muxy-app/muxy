#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn profiling_is_opt_in_and_zero_disables_it() {
    assert!(output_parent(None).is_none());
    assert!(output_parent(Some("".into())).is_none());
    assert!(output_parent(Some("0".into())).is_none());
    assert_eq!(output_parent(Some("1".into())), Some(std::env::temp_dir()));
    assert_eq!(
        output_parent(Some("/tmp/profiles".into())),
        Some(PathBuf::from("/tmp/profiles"))
    );
    assert!(span(Metric::BootLoad).0.is_none());
    assert!(!bypass_row_cache());
}

#[test]
fn intervals_report_and_reset_counts_total_and_maximum_duration() {
    let profile = Profile::new(false);
    let stat = &profile.stats[Metric::TerminalPaint as usize];
    stat.record(Duration::from_millis(3));
    stat.record(Duration::from_millis(7));
    profile.stats[Metric::RowCacheHit as usize]
        .calls
        .fetch_add(20, Ordering::Relaxed);
    let snapshot = profile.take();
    assert_eq!(
        snapshot["terminal.paint"],
        json!({"calls": 2, "total_ms": 10.0, "max_ms": 7.0})
    );
    assert_eq!(snapshot["terminal.row_cache_hit"]["calls"], 20);
    let next = profile.take();
    assert_eq!(
        next["terminal.paint"],
        json!({"calls": 0, "total_ms": 0.0, "max_ms": 0.0})
    );
    assert_eq!(next["terminal.row_cache_hit"]["calls"], 0);
}

#[test]
fn scope_records_on_drop_and_disabled_scope_does_nothing() {
    let stat = Stat::default();
    drop(Span(None));
    {
        let _scope = Span(Some((&stat, Instant::now())));
        assert_eq!(stat.calls.load(Ordering::Relaxed), 0);
    }
    assert_eq!(stat.calls.load(Ordering::Relaxed), 1);
}
