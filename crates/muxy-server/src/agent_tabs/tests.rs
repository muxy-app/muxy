use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use muxy_protocol::{
    AgentProvider, AgentState, MAX_AGENT_TAB_HOSTS, MAX_AGENT_TAB_ID, MAX_AGENT_TABS,
    MAX_AGENT_TITLE, ProjectId, SessionId,
};

use super::opencode::{OpenCode, parse_tabs, read_tabs};
use super::{Pane, Reconciled, Report, Tab, TabSource, Tracker, http, reconcile, sources};

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
        provider: AgentProvider::OpenCode,
        project: ProjectId::from_u128(7),
        directory: directory.into(),
        title: title.into(),
    }
}

fn opencode(tabs: Vec<Tab>, busy: &[&str]) -> HashMap<AgentProvider, Report> {
    HashMap::from([(
        AgentProvider::OpenCode,
        Report {
            tabs: HashMap::from([("/work/app".to_owned(), tabs)]),
            busy: busy.iter().map(|id| (*id).to_owned()).collect(),
            viewed: HashMap::new(),
        },
    )])
}

#[test]
fn every_source_is_a_supported_provider() {
    let providers: Vec<_> = sources().iter().map(|source| source.provider()).collect();
    assert_eq!(providers, super::PROVIDERS);
}

#[test]
fn terminals_claim_the_tab_they_show_and_every_tab_lists_its_hosts() {
    let long = "Investigate the flaky upload retry test on Linux";
    let reports = opencode(
        vec![
            tab("a", "Same title"),
            tab("b", "Same title"),
            tab("c", long),
            tab("d", "Write the release notes"),
        ],
        &["c", "d"],
    );
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
    let reconciled = reconcile(&sources(), &panes, &reports, &Reconciled::default());
    let shown = |id| {
        reconciled
            .shown
            .get(&session(id))
            .map(|(tab, _)| tab.as_str())
    };
    assert_eq!(
        [shown(1), shown(2), shown(3), shown(4), shown(5)],
        [Some("a"), Some("b"), Some("c"), None, None],
        "each duplicate title claims its own tab, the home screen shows none, \
         and tabs are scoped to their directory"
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
        ]
    );
    let hosts = [session(1), session(2), session(3), session(4)];
    assert!(reconciled.tabs.iter().all(|tab| tab.hosts == hosts));
    let d = &reconciled.tabs[3];
    assert_eq!(d.switch.as_deref(), Some(&[0x18, b'4'][..]));
    assert_eq!(d.resume.as_deref(), Some("opencode -s d"));
}

#[test]
fn duplicate_titles_keep_the_terminal_that_showed_them() {
    let tabs = vec![tab("a", "Same title"), tab("b", "Same title")];
    let panes = BTreeMap::from([
        (session(1), pane("/work/app", "OC | Same title")),
        (session(2), pane("/work/app", "OC | Same title")),
    ]);
    let mut previous = Reconciled::default();
    previous
        .shown
        .insert(session(1), ("b".into(), "Same title".into()));
    previous
        .shown
        .insert(session(2), ("a".into(), "Same title".into()));
    let reconciled = reconcile(&sources(), &panes, &opencode(tabs, &["b"]), &previous);
    assert_eq!(reconciled.shown[&session(1)].0, "b");
    assert_eq!(reconciled.shown[&session(2)].0, "a");
    assert_eq!(
        reconciled.busy,
        HashSet::from([session(1)]),
        "work stays with the terminal showing it"
    );
}

#[test]
fn a_tab_lists_no_more_hosts_than_the_protocol_allows() {
    let panes: BTreeMap<_, _> = (1..=u64::try_from(MAX_AGENT_TAB_HOSTS).expect("fits") + 5)
        .map(|id| (session(id), pane("/work/app", "OpenCode")))
        .collect();
    let reconciled = reconcile(
        &sources(),
        &panes,
        &opencode(vec![tab("a", "One")], &[]),
        &Reconciled::default(),
    );
    assert_eq!(reconciled.tabs[0].hosts.len(), MAX_AGENT_TAB_HOSTS);
}

#[test]
fn a_closed_terminal_hands_its_tab_to_another_without_waiting_for_a_poll() {
    let activity = std::sync::Arc::new(crate::activity::Activity::default());
    activity.set_agent_tabs_report(
        AgentProvider::OpenCode,
        Report {
            tabs: HashMap::from([(
                "/work/app".to_owned(),
                vec![tab("x", "First"), tab("y", "Second")],
            )]),
            busy: HashSet::new(),
            viewed: HashMap::new(),
        },
    );
    activity.observe_agent_tabs(session(1), pane("/work/app", "OC | First"));
    activity.observe_agent_tabs(session(2), pane("/work/app", "OC | Second"));
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
fn a_failed_poll_keeps_tabs_and_clearing_busy_ends_their_work() {
    let mut tracker = Tracker::default();
    tracker.observe(session(1), pane("/work/app", "OC | First"));
    for (provider, report) in opencode(vec![tab("x", "First"), tab("y", "Second")], &["x", "y"]) {
        tracker.set_report(provider, report);
    }
    tracker.reconcile();
    assert!(tracker.reconciled().busy.contains(&session(1)));
    tracker.clear_busy(AgentProvider::OpenCode);
    tracker.reconcile();
    assert!(tracker.reconciled().busy.is_empty());
    assert_eq!(tracker.reconciled().tabs.len(), 2, "the tabs stay listed");
    assert!(
        tracker
            .reconciled()
            .tabs
            .iter()
            .all(|tab| tab.state == AgentState::Idle)
    );
}

#[test]
fn opencode_switches_to_its_first_ten_tabs_and_resumes_only_safe_ids() {
    let opencode = OpenCode::default();
    assert_eq!(opencode.switch_input(0), Some(vec![0x18, b'1']));
    assert_eq!(opencode.switch_input(9), Some(vec![0x18, b'0']));
    assert_eq!(opencode.switch_input(10), None);
    assert_eq!(
        opencode.resume_command("ses_e5533ee34ffe"),
        Some("opencode -s ses_e5533ee34ffe".into())
    );
    for unsafe_id in ["", "-s x", "a b", "x;rm -rf ~", "$(id)"] {
        assert_eq!(opencode.resume_command(unsafe_id), None, "{unsafe_id:?}");
    }
    assert_eq!(
        opencode.shown_title("OC | Fix the build…"),
        Some(("Fix the build", true))
    );
    assert_eq!(opencode.shown_title("OpenCode"), None);
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
fn a_service_that_trickles_its_reply_cannot_hold_the_poller() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connection");
        let mut request = [0; 1024];
        let _ = stream.read(&mut request);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n");
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(300));
            if stream.write_all(b"x").is_err() {
                return;
            }
        }
    });
    let started = Instant::now();
    let later = Instant::now() + Duration::from_secs(60);
    assert!(http::get(address, "/api/session/active", "secret", later).is_none());
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "one deadline bounds the whole reply: {:?}",
        started.elapsed()
    );
}

#[test]
fn each_directory_takes_its_tabs_from_the_newest_channel_that_has_it() {
    let directory = std::env::temp_dir().join(format!(
        "muxy-agent-tabs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let write = |channel: &str, value: serde_json::Value, age: u64| {
        let path = directory.join(channel).join("tui");
        std::fs::create_dir_all(&path).expect("channel");
        let file = path.join("tabs.json");
        std::fs::write(&file, value.to_string()).expect("tabs");
        let modified = std::time::SystemTime::now() - Duration::from_secs(age);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|file| file.set_modified(modified))
            .expect("modified time");
    };
    write(
        "beta",
        serde_json::json!({ "cwd": {
            "/work/app": { "tabs": [{ "sessionID": "closed", "title": "Closed since" }] },
            "/work/only-beta": { "tabs": [{ "sessionID": "b", "title": "Beta" }] },
        }}),
        600,
    );
    write(
        "latest",
        serde_json::json!({ "cwd": {
            "/work/app": { "tabs": [{ "sessionID": "a", "title": "One" }, { "title": "no id" }] },
            "/work/other": { "tabs": [{ "sessionID": "c", "title": "Two" }] },
        }}),
        60,
    );
    let watched = HashSet::from(["/work/app/".to_owned(), "/work/only-beta".to_owned()]);
    let tabs = read_tabs(&directory, &watched);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(tabs.len(), 2, "only watched directories");
    assert_eq!(
        tabs["/work/app"],
        vec![tab("a", "One")],
        "an older channel's closed tab doesn't come back"
    );
    assert_eq!(tabs["/work/only-beta"], vec![tab("b", "Beta")]);
    assert!(parse_tabs(&serde_json::json!({}), &watched).is_empty());
}

#[test]
fn a_shortened_title_follows_the_tab_viewed_last() {
    let prefix = "Investigate the flaky upload retry te";
    let tabs = vec![
        tab("x", &format!("{prefix}st on Linux")),
        tab("y", &format!("{prefix}st on macOS")),
    ];
    let title = format!("OC | {prefix}…");
    let mut reports = opencode(tabs, &[]);
    let report = reports.get_mut(&AgentProvider::OpenCode).expect("report");
    report.viewed = HashMap::from([("x".to_owned(), 1), ("y".to_owned(), 2)]);
    let one = BTreeMap::from([(session(1), pane("/work/app", &title))]);
    let mut previous = Reconciled::default();
    previous
        .shown
        .insert(session(1), ("x".into(), String::new()));
    let reconciled = reconcile(&sources(), &one, &reports, &previous);
    assert_eq!(
        reconciled.shown[&session(1)].0,
        "y",
        "switching between tabs whose titles look the same is seen through their view times"
    );

    let two = BTreeMap::from([
        (session(1), pane("/work/app", &title)),
        (session(2), pane("/work/app", &title)),
    ]);
    previous
        .shown
        .insert(session(2), ("y".into(), String::new()));
    let reconciled = reconcile(&sources(), &two, &reports, &previous);
    assert_eq!(
        (
            reconciled.shown[&session(1)].0.as_str(),
            reconciled.shown[&session(2)].0.as_str()
        ),
        ("x", "y"),
        "a tab another terminal shows isn't taken from it"
    );
}

#[test]
fn reports_are_cut_to_the_protocol_limits() {
    let long = "é".repeat(MAX_AGENT_TITLE);
    let many: Vec<_> = (0..MAX_AGENT_TABS + 10)
        .map(|index| tab(&format!("ses_{index}"), &long))
        .chain([tab(&"x".repeat(MAX_AGENT_TAB_ID + 1), "Too long an ID")])
        .collect();
    let panes = BTreeMap::from([(session(1), pane("/work/app", "OpenCode"))]);
    let reconciled = reconcile(
        &sources(),
        &panes,
        &opencode(many, &[]),
        &Reconciled::default(),
    );
    assert_eq!(reconciled.tabs.len(), MAX_AGENT_TABS);
    assert!(
        reconciled
            .tabs
            .iter()
            .all(|tab| tab.title.len() <= MAX_AGENT_TITLE && tab.id.len() <= MAX_AGENT_TAB_ID)
    );
}
