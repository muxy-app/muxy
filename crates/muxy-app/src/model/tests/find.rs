use super::*;
use crate::views::terminal::find::Results;
use crate::views::terminal::pane::TerminalPane;
use muxy_protocol::{ErrorCode, HistoryCursor, SearchMatch, SearchPage};

fn found(row: u64) -> SearchMatch {
    SearchMatch {
        row,
        start: 0,
        end: 1,
    }
}

fn page(matches: Vec<SearchMatch>, next: Option<HistoryCursor>, total_rows: u64) -> SearchPage {
    SearchPage {
        matches,
        next,
        total_rows,
        scanned_rows: 2000,
    }
}

#[test]
fn search_replies_ignore_old_queries_continue_empty_pages_and_restart_stale_cursors() {
    let mut results = Results::default();
    results.query = "a".into();
    let old = results.restart().expect("request");
    results.query = "b".into();
    let fresh = results.restart().expect("request");
    assert!(
        results
            .receive(&old, Ok(page(vec![found(1)], None, 50)))
            .is_none()
    );
    assert!(results.matches.is_empty());
    let older = results
        .receive(&fresh, Ok(page(vec![], Some(HistoryCursor(42)), 50)))
        .expect("continue empty page");
    let restart = results
        .receive(
            &older,
            Err(muxy_client::ClientError::Server(
                muxy_protocol::ErrorReply {
                    code: ErrorCode::StaleHistoryCursor,
                    message: "changed".into(),
                },
            )),
        )
        .expect("restart");
    assert_eq!(restart.before, HistoryCursor(0));
    assert_ne!(restart.token, older.token);
    results.receive(&restart, Ok(page(vec![found(49), found(2)], None, 50)));
    assert_eq!(results.counter(), "1 of 2");
    results.step(true);
    assert_eq!(results.current, Some(1));
    results.step(false);
    assert_eq!(results.current, Some(0));
}

#[test]
fn search_maps_reply_totals_and_keeps_frozen_highlights_separate_from_live_output() {
    let mut grid = attachment().grid;
    grid.history_total = 100;
    grid.history = (0..10)
        .map(|index| muxy_protocol::Row {
            index,
            runs: vec![muxy_protocol::Run {
                text: "a".into(),
                width: 1,
                style: muxy_protocol::Style::default(),
            }],
        })
        .collect();
    assert_eq!(grid.search_content_row(92, 100), Some(2));
    assert_eq!(grid.search_content_row(89, 100), None);
    assert_eq!(grid.search_content_row(100, 100), Some(10));
    let frozen = grid.clone();
    let mut results = Results::default();
    results.query = "a".into();
    results.matches = vec![found(92)];
    results.total_rows = 100;
    results.current = Some(0);
    assert_eq!(results.highlights(&frozen, 2), vec![(found(92), true)]);
    grid.history_total = 101;
    assert!(results.highlights(&grid, 2).is_empty());
    assert_eq!(results.highlights(&frozen, 2).len(), 1);
    grid.history_total = 100;
    grid.history[2].runs[0].text = "b".into();
    assert!(results.highlights(&grid, 2).is_empty());
}

fn pane<'a>(model: &'a AppModel, cx: &'a gpui::App) -> &'a TerminalPane {
    model
        .terminal(&model.active_pane().expect("pane"))
        .expect("terminal")
        .view
        .read(cx)
}

#[gpui::test]
fn find_searches_each_edit_immediately_ignores_case_and_keeps_input_out_of_the_shell(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        model.new_tab(cx);
        let pane = model.active_pane().expect("pane");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    sandbox: None,
                    pane,
                    session: SessionId::new(42).expect("ID"),
                    attachment: attachment(),
                    created: true,
                },
            ),
            cx,
        );
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-f");
    cx.run_until_parked();
    requests.try_iter().for_each(drop);
    for expected in ["f", "fi", "fin", "fina", "final"] {
        type_text(cx, &expected[expected.len() - 1..]);
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(!work.iter().any(|work| matches!(work, Work::Input(..))));
        let (pane_id, request) = work
            .into_iter()
            .find_map(|work| match work {
                Work::Search { pane, request, .. } => Some((pane, request)),
                _ => None,
            })
            .expect("typing sends a search without Enter or advancing the clock");
        assert_eq!(request.query, expected);
        assert!(request.ignore_case);
        view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Search {
                        pane: pane_id,
                        request,
                        result: Ok(page(vec![found(0)], None, 0)),
                    },
                ),
                cx,
            );
        });
        assert_eq!(
            view.read_with(cx, |model, cx| pane(model, cx)
                .find
                .as_ref()
                .expect("find")
                .results
                .counter()),
            "1 of 1"
        );
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, cx| pane(model, cx).find.is_none()));
    type_text(cx, "x");
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Input(_, bytes) if bytes == b"x"))
    );
}
