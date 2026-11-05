use std::collections::{BTreeMap, HashMap, HashSet};

use muxy_protocol::{AgentState, ProjectId, SessionId};

use super::{Pane, Report, Tab, http, reconcile};

fn session(id: u64) -> SessionId {
    SessionId::new(id).expect("nonzero")
}

fn tab(id: &str, title: &str) -> Tab {
    Tab {
        id: id.into(),
        title: title.into(),
    }
}

fn pane(directory: &str, title: &str) -> Pane {
    Pane {
        project: ProjectId::from_u128(7),
        directory: directory.into(),
        title: title.into(),
    }
}

#[test]
fn terminals_claim_the_tab_they_show_and_the_rest_are_listed_under_one_host() {
    let long = "Investigate the flaky upload retry test on Linux";
    let report = Report {
        tabs: HashMap::from([(
            "/work/app".to_owned(),
            vec![
                tab("a", "Same title"),
                tab("b", "Same title"),
                tab("c", long),
                tab("d", "Write the release notes"),
            ],
        )]),
        busy: HashSet::from(["c".to_owned(), "d".to_owned()]),
    };
    let shortened = format!("OC | {}…", &long[..37]);
    let panes = BTreeMap::from([
        (session(1), pane("/work/app/", "OC | Same title")),
        (session(2), pane("/work/app", "OC | Same title")),
        (session(3), pane("/work/app", &shortened)),
        (session(4), pane("/work/app", "OpenCode")),
        (
            session(5),
            pane("/elsewhere", "OC | Write the release notes"),
        ),
    ]);
    let reconciled = reconcile(&panes, &report);
    assert_eq!(
        reconciled.titles.get(&session(1)).map(String::as_str),
        Some("Same title")
    );
    assert_eq!(
        reconciled.titles.get(&session(2)).map(String::as_str),
        Some("Same title")
    );
    assert_eq!(
        reconciled.titles.get(&session(3)).map(String::as_str),
        Some(long)
    );
    assert!(
        !reconciled.titles.contains_key(&session(4)),
        "the home screen shows no tab"
    );
    assert!(
        !reconciled.titles.contains_key(&session(5)),
        "tabs are scoped to their directory"
    );
    assert_eq!(reconciled.busy, HashSet::from([session(3)]));
    let listed: Vec<_> = reconciled
        .tabs
        .iter()
        .map(|tab| (tab.id.as_str(), tab.shown_by, tab.index, tab.state))
        .collect();
    assert_eq!(
        listed,
        [
            ("a", Some(session(1)), Some(0), AgentState::Idle),
            ("b", Some(session(2)), Some(1), AgentState::Idle),
            ("c", Some(session(3)), Some(2), AgentState::Working),
            ("d", None, Some(3), AgentState::Working),
        ],
        "each duplicate title claims its own tab"
    );
    assert!(
        reconciled
            .tabs
            .iter()
            .all(|tab| tab.hosts == [session(1), session(2), session(3), session(4)]),
        "every terminal in the directory can switch to every tab"
    );
}

#[test]
fn a_closed_terminal_hands_its_tab_to_another_without_waiting_for_a_poll() {
    let activity = std::sync::Arc::new(crate::activity::Activity::default());
    activity.set_harness_report(Report {
        tabs: HashMap::from([(
            "/work/app".to_owned(),
            vec![tab("x", "First"), tab("y", "Second")],
        )]),
        busy: HashSet::new(),
    });
    activity.observe_harness(session(1), pane("/work/app", "OC | First"));
    activity.observe_harness(session(2), pane("/work/app", "OC | Second"));
    let shown = |activity: &crate::activity::Activity| {
        activity
            .snapshot()
            .tabs
            .iter()
            .map(|tab| (tab.id.clone(), tab.shown_by, tab.hosts.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shown(&activity),
        [
            ("x".into(), Some(session(1)), vec![session(1), session(2)]),
            ("y".into(), Some(session(2)), vec![session(1), session(2)]),
        ]
    );
    activity.remove(session(2));
    assert_eq!(
        shown(&activity),
        [
            ("x".into(), Some(session(1)), vec![session(1)]),
            ("y".into(), None, vec![session(1)]),
        ],
        "the closed terminal's tab stays listed under the one left"
    );
}

#[test]
fn the_service_password_only_goes_to_this_computer() {
    assert!(http::loopback("http://127.0.0.1:49374").is_some());
    assert!(http::loopback("http://localhost:4096/").is_some());
    assert!(http::loopback("http://[::1]:4096").is_some());
    assert!(http::loopback("http://10.0.0.5:4096").is_none());
    assert!(http::loopback("http://example.com:4096").is_none());
    assert!(http::loopback("https://127.0.0.1:4096").is_none());
}

#[test]
fn replies_are_read_by_length_or_chunks_and_errors_are_refused() {
    let plain = b"HTTP/1.1 200 OK\r\ncontent-length: 4\r\n\r\n{}{}trailing";
    assert_eq!(http::body(plain).as_deref(), Some(&b"{}{}"[..]));
    let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{\"a\r\n4;x=1\r\n\":1}\r\n0\r\n\r\n";
    assert_eq!(http::body(chunked).as_deref(), Some(&b"{\"a\":1}"[..]));
    assert!(http::body(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\n\r\n").is_none());
    assert!(http::body(b"HTTP/1.1 200 OK\r\ncontent-length: 99\r\n\r\nshort").is_none());
    assert!(
        http::body(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nff\r\nshort").is_none()
    );
    assert_eq!(
        http::query("/Users/me/My Project?#&"),
        "/Users/me/My%20Project%3F%23%26"
    );
}

#[test]
fn tabs_are_read_only_for_watched_directories() {
    let file = serde_json::json!({
        "global": { "tabs": [] },
        "cwd": {
            "/work/app": { "tabs": [{ "sessionID": "a", "title": "One" }, { "title": "no id" }] },
            "/work/other": { "tabs": [{ "sessionID": "b", "title": "Two" }] },
        }
    });
    let tabs = super::opencode::parse_tabs(&file, &HashSet::from(["/work/app/".to_owned()]));
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs["/work/app"], vec![tab("a", "One")]);
}
