use std::sync::Arc;

use super::*;
use crate::views::terminal::insert_path::{Entry, Source};
use crate::views::terminal::pane::TerminalPane;
use muxy_core::shortcuts::ShortcutId;

fn entry(name: &str, is_directory: bool) -> Entry {
    Entry {
        name: name.to_owned(),
        is_directory,
    }
}

fn source(recents: &[String]) -> Source {
    Source {
        list: Ok(Arc::new(|directory| match directory {
            "/tmp" => Ok(vec![
                entry(".cache", true),
                entry("docs", true),
                entry("my file.txt", false),
                entry("notes.md", false),
            ]),
            "/tmp/docs" => Ok(vec![entry("guide.md", false)]),
            "/" => Ok(vec![entry("etc", true), entry("tmp", true)]),
            other => Err(format!("{other} does not exist")),
        })),
        home: Some("/home/dev".to_owned()),
        recents: recents.to_vec(),
    }
}

fn terminal(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Entity<TerminalPane> {
    view.update(cx, |model, _| {
        model
            .terminal(&model.active_pane().expect("pane"))
            .expect("terminal")
            .view
            .clone()
    })
}

fn attached(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
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
                    pane,
                    session: SessionId::new(42).expect("ID"),
                    attachment: attachment(),
                    created: true,
                },
            ),
            cx,
        );
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx, requests)
}

fn open(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(ShortcutId::InsertPath.definition().keys[0]);
    cx.run_until_parked();
    let recents = view.update(cx, |model, _| {
        model.state.recent_paths(ServerId::local()).to_vec()
    });
    terminal(view, cx).update(cx, |pane, cx| pane.browse_paths(source(&recents), cx));
    cx.run_until_parked();
}

fn is_open(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> bool {
    terminal(view, cx).read_with(cx, |pane, _| pane.insert_path.is_some())
}

fn rows(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<String> {
    terminal(view, cx).read_with(cx, |pane, cx| {
        let picker = pane.insert_path.as_ref().expect("open").picker.read(cx);
        picker
            .items()
            .iter()
            .cloned()
            .filter_map(muxy_ui::picker::PickerItem::row_data)
            .map(|row| row.title.to_string())
            .collect()
    })
}

fn typed(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> Vec<u8> {
    requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Input(_, bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect()
}

#[gpui::test]
fn the_shortcut_lists_the_terminals_folder_and_return_types_the_chosen_path(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached(cx);
    assert!(!is_open(&view, cx));
    open(&view, cx);
    assert!(is_open(&view, cx));
    assert!(cx.debug_bounds("insert-path").is_some());
    assert_eq!(
        rows(&view, cx),
        ["docs", "my file.txt", "notes.md"],
        "dot names wait for a dot"
    );
    assert!(cx.debug_bounds("picker-row-docs").is_some());

    requests.try_iter().for_each(drop);
    type_text(cx, "no");
    assert_eq!(rows(&view, cx), ["notes.md"]);
    assert!(
        typed(&requests).is_empty(),
        "the filter stays out of the shell"
    );

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"notes.md");
    assert!(!is_open(&view, cx));
    view.update(cx, |model, _| {
        assert_eq!(
            model.state.recent_paths(ServerId::local()),
            ["/tmp/notes.md"]
        );
    });

    type_text(cx, "x");
    assert_eq!(
        typed(&requests),
        b"x",
        "the terminal has the keyboard again"
    );
}

#[gpui::test]
fn tab_opens_folders_backspace_leaves_them_and_recent_folders_come_first(cx: &mut TestAppContext) {
    let (view, cx, requests) = attached(cx);
    open(&view, cx);
    type_text(cx, "do");
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(rows(&view, cx), ["guide.md"]);

    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(
        rows(&view, cx),
        ["docs", "my file.txt", "notes.md"],
        "an empty filter's backspace goes up"
    );
    assert!(is_open(&view, cx));

    requests.try_iter().for_each(drop);
    type_text(cx, "docs/gu");
    assert_eq!(rows(&view, cx), ["guide.md"]);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"docs/guide.md");

    open(&view, cx);
    let selected = terminal(&view, cx).read_with(cx, |pane, cx| {
        let picker = pane.insert_path.as_ref().expect("open").picker.read(cx);
        picker.selected_row_id().map(str::to_owned)
    });
    assert_eq!(
        selected.as_deref(),
        Some("docs"),
        "it holds the last choice"
    );
    assert!(cx.debug_bounds("picker-trailing-icon-docs").is_some());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        typed(&requests),
        b"docs/",
        "a folder is typed with its slash"
    );
}

#[gpui::test]
fn paths_outside_the_terminals_folder_are_absolute_and_spaces_are_quoted(cx: &mut TestAppContext) {
    let (view, cx, requests) = attached(cx);
    open(&view, cx);
    requests.try_iter().for_each(drop);
    type_text(cx, "my");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"'my file.txt'");

    open(&view, cx);
    type_text(cx, "/et");
    assert_eq!(rows(&view, cx), ["etc"]);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"/etc/");
}

#[gpui::test]
fn a_path_that_cannot_be_quoted_for_the_program_stays_untyped(cx: &mut TestAppContext) {
    let (view, cx, requests) = attached(cx);
    terminal(&view, cx).update(cx, |pane, _| {
        pane.process = Some(muxy_protocol::ForegroundProcess {
            name: "ssh".into(),
            is_shell: false,
        });
    });
    open(&view, cx);
    requests.try_iter().for_each(drop);
    type_text(cx, "my");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(typed(&requests).is_empty());
    assert!(is_open(&view, cx), "the popover stays to say why");

    cx.simulate_keystrokes("backspace backspace");
    type_text(cx, "no");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"notes.md", "a bare path needs no quotes");
}

#[gpui::test]
fn escape_and_the_shortcut_close_it_and_turning_recents_off_forgets_choices(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached(cx);
    open(&view, cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!is_open(&view, cx));
    requests.try_iter().for_each(drop);
    type_text(cx, "x");
    assert_eq!(typed(&requests), b"x");

    open(&view, cx);
    cx.simulate_keystrokes(ShortcutId::InsertPath.definition().keys[0]);
    cx.run_until_parked();
    assert!(!is_open(&view, cx), "the shortcut closes what it opened");

    view.update(cx, |model, _| {
        model.settings.panes.insert_path_recents = false;
    });
    open(&view, cx);
    type_text(cx, "no");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"notes.md");
    view.update(cx, |model, _| {
        assert!(model.state.recent_paths(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn a_folder_that_cannot_be_listed_says_why_and_closing_focus_closes_it(cx: &mut TestAppContext) {
    let (view, cx, _requests) = attached(cx);
    cx.simulate_keystrokes(ShortcutId::InsertPath.definition().keys[0]);
    cx.run_until_parked();
    assert!(is_open(&view, cx), "it opens before the app answers");
    terminal(&view, cx).update(cx, |pane, cx| {
        pane.browse_paths(
            Source {
                list: Err("Still connecting".to_owned()),
                home: None,
                recents: Vec::new(),
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(rows(&view, cx).is_empty());
    assert!(is_open(&view, cx));

    terminal(&view, cx).update_in(cx, |pane, window, _| pane.focus.focus(window));
    cx.run_until_parked();
    assert!(
        !is_open(&view, cx),
        "it closes when the keyboard goes elsewhere"
    );
}

#[gpui::test]
fn a_new_filter_selects_its_first_row_whatever_was_selected_before(cx: &mut TestAppContext) {
    let (view, cx, requests) = attached(cx);
    open(&view, cx);
    requests.try_iter().for_each(drop);
    cx.simulate_keystrokes("down down");
    type_text(cx, "o");
    assert_eq!(rows(&view, cx), ["docs", "notes.md"]);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(typed(&requests), b"docs/");
}

#[gpui::test]
fn a_terminal_that_ended_meanwhile_gets_nothing_and_remembers_nothing(cx: &mut TestAppContext) {
    use crate::views::terminal::pane::PaneState;

    let (view, cx, requests) = attached(cx);
    open(&view, cx);
    requests.try_iter().for_each(drop);
    terminal(&view, cx).update(cx, |pane, _| pane.state = PaneState::Disconnected);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(typed(&requests).is_empty());
    assert!(!is_open(&view, cx));
    view.update(cx, |model, _| {
        assert!(model.state.recent_paths(ServerId::local()).is_empty());
    });
}
