use super::worktrees::{Sent, connected, pruning_fixture};
use super::*;
use crate::model::agents_panel::{AgentStatus, Entry, EntryKey, Placement, RunningAgent};
use gpui::Modifiers;
use muxy_app_core::settings::{AgentsPanelDensity, AgentsPanelDock, AgentsPanelGrouping};
use muxy_core::shortcuts::ShortcutId;
use muxy_protocol::{
    ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity, AgentProvider, AgentState,
};

struct Fixture {
    root: ProjectId,
    worktree: ProjectId,
    panes: [PaneId; 4],
    sent: Sent,
}

fn agents(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext, Fixture) {
    let (mut state, root, [worktree, ..], _directory) = pruning_fixture();
    let home = state.home().id;
    let first_pane = |state: &AppState, project: ProjectId, tab: TabId| {
        state
            .project(project)
            .and_then(|project| project.tabs.iter().find(|entry| entry.id == tab))
            .map(|tab| tab.panes[0].id)
            .expect("pane")
    };
    let root_tab = state.open_terminal_tab(root).expect("tab");
    let root_pane = first_pane(&state, root, root_tab);
    let mirror = state
        .split_pane(root_pane, Direction::Right)
        .expect("split");
    let worktree_tab = state.open_terminal_tab(worktree).expect("tab");
    let worktree_pane = first_pane(&state, worktree, worktree_tab);
    let home_tab = state.open_terminal_tab(home).expect("tab");
    let home_pane = first_pane(&state, home, home_tab);
    state.select_project(root).expect("root");
    state.select_tab(root, root_tab).expect("tab");
    let session = |id| SessionId::new(id).expect("session");
    let agent = |id, state| AgentActivity {
        session: session(id),
        project: root,
        provider: AgentProvider::Claude,
        state,
        title: None,
    };
    let finished = |id, session, read| ActivityEvent {
        id,
        session,
        project: home,
        provider: AgentProvider::Claude,
        timestamp: 0,
        kind: ActivityKind::Completed,
        read,
    };
    let (view, cx, sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        let local = ServerId::local();
        model.receive_attached(local, root_pane, session(1), attachment(), true, cx);
        model.receive_attached(local, mirror, session(1), attachment(), false, cx);
        model.receive_attached(local, worktree_pane, session(2), attachment(), false, cx);
        model.receive_attached(local, home_pane, session(3), attachment(), false, cx);
        model.servers.local.activity.loaded = true;
        model.receive_activity(
            local,
            Ok(ActivitySnapshot {
                agents: vec![
                    agent(1, AgentState::Working),
                    agent(2, AgentState::Blocked),
                    agent(3, AgentState::Idle),
                    agent(4, AgentState::Blocked),
                ],
                events: vec![
                    finished(1, session(1), false),
                    finished(2, session(3), false),
                ],
                ..ActivitySnapshot::default()
            }),
            cx,
        );
    });
    (
        view,
        cx,
        Fixture {
            root,
            worktree,
            panes: [root_pane, mirror, worktree_pane, home_pane],
            sent,
        },
    )
}

#[gpui::test]
fn the_panel_lists_each_shown_session_once_grouped_by_project(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    let [root_pane, _, worktree_pane, home_pane] = fixture.panes;
    view.read_with(cx, |model, _| {
        let agents: Vec<_> = model
            .running_agents()
            .into_iter()
            .map(|agent| (agent.pane, agent.status))
            .collect();
        assert_eq!(
            agents,
            [
                (worktree_pane, AgentStatus::Blocked),
                (root_pane, AgentStatus::Working),
                (home_pane, AgentStatus::Completed),
            ],
            "a session in two panes counts once, a working agent's old finish is not \
             Completed, and an agent no pane shows is left out"
        );
    });
    view.update(cx, |model, _| {
        let home = model.state.home().id;
        let entries = model.agents_panel_entries();
        let panes: Vec<_> = entries
            .iter()
            .map(|entry| match entry {
                Entry::Agent(agent) => agent.pane,
                Entry::Worktree { .. } => unreachable!("agent rows"),
            })
            .collect();
        assert_eq!(
            panes,
            [home_pane, root_pane, worktree_pane],
            "agents follow sidebar order, with a worktree after its project"
        );

        let resting = |pane, project, session, status| RunningAgent {
            server: ServerId::local(),
            session: SessionId::new(session).expect("session"),
            project,
            tab: model.state.project(project).expect("project").tabs[0].id,
            pane,
            provider: AgentProvider::Codex,
            status,
            finished_at: None,
            hidden: None,
            order: (0, 0),
        };
        let entries = model.agents_panel_entries_from(vec![
            resting(root_pane, fixture.root, 1, AgentStatus::Idle),
            resting(home_pane, home, 3, AgentStatus::Idle),
            resting(worktree_pane, fixture.root, 2, AgentStatus::Blocked),
        ]);
        let order: Vec<_> = entries
            .iter()
            .map(|entry| match entry {
                Entry::Agent(agent) => (agent.project, agent.status),
                Entry::Worktree { .. } => unreachable!("agent rows"),
            })
            .collect();
        assert_eq!(
            order,
            [
                (home, AgentStatus::Idle),
                (fixture.root, AgentStatus::Blocked),
                (fixture.root, AgentStatus::Idle),
            ],
            "the most urgent agent leads its project"
        );

        model.appearance.agents_panel_grouping = AgentsPanelGrouping::Worktrees;
        let entries = model.agents_panel_entries();
        assert!(matches!(
            entries.as_slice(),
            [
                Entry::Worktree { project: first, counts: [0, 0, 1, 0] },
                Entry::Worktree { project: second, counts: [0, 1, 0, 0] },
                Entry::Worktree { project: third, counts: [1, 0, 0, 0] },
            ] if *first == home && *second == fixture.root && *third == fixture.worktree
        ));
    });
}

#[gpui::test]
fn idle_time_comes_from_the_last_finish_of_resting_agents(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    let [root_pane, _, worktree_pane, home_pane] = fixture.panes;
    let finished_at = |model: &AppModel| {
        model
            .running_agents()
            .into_iter()
            .map(|agent| (agent.pane, agent.status, agent.finished_at))
            .collect::<Vec<_>>()
    };
    view.read_with(cx, |model, _| {
        assert_eq!(
            finished_at(model),
            [
                (worktree_pane, AgentStatus::Blocked, None),
                (root_pane, AgentStatus::Working, None),
                (home_pane, AgentStatus::Completed, Some(0)),
            ],
            "a working agent's earlier finish is not its idle time"
        );
    });
    let receive = |view: &Entity<AppModel>,
                   cx: &mut VisualTestContext,
                   revision,
                   home: AgentState,
                   events: Vec<ActivityEvent>| {
        view.update(cx, |model, cx| {
            let agent = |id, state| AgentActivity {
                session: SessionId::new(id).expect("session"),
                project: fixture.root,
                provider: AgentProvider::Claude,
                state,
                title: None,
            };
            model.receive_activity(
                ServerId::local(),
                Ok(ActivitySnapshot {
                    revision,
                    agents: vec![
                        agent(1, AgentState::Idle),
                        agent(2, AgentState::Idle),
                        agent(3, home),
                    ],
                    events,
                    tabs: Vec::new(),
                }),
                cx,
            );
        });
    };
    let attention = ActivityEvent {
        id: 3,
        session: SessionId::new(1).expect("session"),
        project: fixture.root,
        provider: AgentProvider::Claude,
        timestamp: 300,
        kind: ActivityKind::Attention,
        read: false,
    };
    receive(&view, cx, 2, AgentState::Idle, vec![attention]);
    view.read_with(cx, |model, _| {
        assert_eq!(
            finished_at(model),
            [
                (home_pane, AgentStatus::Idle, Some(0)),
                (root_pane, AgentStatus::Idle, None),
                (worktree_pane, AgentStatus::Idle, None),
            ],
            "an acknowledged finish keeps its time, and an attention request is not a finish"
        );
    });
    receive(&view, cx, 3, AgentState::Working, Vec::new());
    receive(&view, cx, 4, AgentState::Idle, Vec::new());
    view.read_with(cx, |model, _| {
        assert_eq!(
            finished_at(model).first(),
            Some(&(home_pane, AgentStatus::Idle, None)),
            "work that ends without a finish does not reuse the previous one"
        );
    });
}

#[gpui::test]
fn the_active_card_prefers_the_focused_session_in_a_split_tab(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    view.update(cx, |model, cx| {
        let active = |model: &AppModel| model.active_agents_entry(&model.agents_panel_entries());
        let local = ServerId::local();
        let session = |id| SessionId::new(id).expect("session");
        let key = |id| EntryKey::Agent(local, session(id));
        let [root_pane, mirror, ..] = fixture.panes;
        model.focus_pane(mirror, cx);
        assert_eq!(active(model), Some(key(1)), "mirrored panes share a card");
        model.receive_attached(local, mirror, session(4), attachment(), false, cx);
        assert_eq!(active(model), Some(key(4)));
        model.focus_pane(root_pane, cx);
        assert_eq!(
            active(model),
            Some(key(1)),
            "focus takes priority over urgency"
        );
        model.detach_terminal(mirror, cx);
        model.focus_pane(mirror, cx);
        assert_eq!(
            active(model),
            Some(key(1)),
            "a non-agent split still follows its tab"
        );
    });
}

#[gpui::test]
fn the_active_card_follows_a_mirrored_session_from_another_tab(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    view.update(cx, |model, cx| {
        let active = |model: &AppModel| model.active_agents_entry(&model.agents_panel_entries());
        let local = ServerId::local();
        let session = SessionId::new(1).expect("session");
        model.state.open_terminal_tab(fixture.root).expect("tab");
        let mirror = model.active_pane().expect("pane");
        model.receive_attached(local, mirror, session, attachment(), true, cx);
        assert_eq!(active(model), Some(EntryKey::Agent(local, session)));

        let plain = model
            .state
            .split_pane(mirror, Direction::Right)
            .expect("split");
        model.receive_attached(
            local,
            plain,
            SessionId::new(5).expect("session"),
            attachment(),
            true,
            cx,
        );
        assert_eq!(model.active_pane(), Some(plain));
        assert_eq!(
            active(model),
            Some(EntryKey::Agent(local, session)),
            "the active tab's sessions take priority over the card's representative tab"
        );
    });
}

#[gpui::test]
fn the_shortcut_shows_and_focuses_then_hides_and_keys_open_only_live_panes(
    cx: &mut TestAppContext,
) {
    let (view, cx, fixture) = agents(cx);
    let [root_pane, _, worktree_pane, _] = fixture.panes;
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = false;
        model.focus_owner.request_active_pane();
        cx.notify();
    });
    cx.run_until_parked();
    let focused = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.update_in(cx, |model, window, cx| {
            model.agents_panel.focus.contains_focused(window, cx)
        })
    };

    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.read_with(cx, |model, _| {
        assert_eq!(model.agents_panel_placement(), Placement::Docked);
        assert!(model.agents_panel_shown());
    });
    assert!(focused(&view, cx), "a hidden panel shows with focus");
    cx.run_until_parked();
    assert!(
        focused(&view, cx),
        "focus is not handed back on the next frame"
    );

    cx.simulate_keystrokes("up down enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, fixture.root);
        assert_eq!(model.active_pane(), Some(root_pane));
    });
    assert!(!focused(&view, cx), "opening an agent focuses its pane");

    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    assert!(focused(&view, cx), "a shown panel takes focus first");
    view.read_with(cx, |model, _| assert!(model.agents_panel_shown()));
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.read_with(cx, |model, _| {
        assert!(!model.agents_panel_shown(), "a focused panel hides");
        assert!(!model.appearance.agents_panel_visible);
    });

    view.update(cx, |model, cx| {
        model.select_project(fixture.root, cx);
        model.detach_terminal(worktree_pane, cx);
    });
    view.update_in(cx, |model, window, cx| {
        model.open_agents_entry(
            EntryKey::Agent(ServerId::local(), SessionId::new(2).expect("session")),
            window,
            cx,
        );
        assert_eq!(
            model.state.current_project().id,
            fixture.root,
            "an agent whose pane is gone opens nothing"
        );
    });
}

#[gpui::test]
fn a_collapsed_sidebar_opens_the_panel_as_a_popup_that_escape_closes(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = false;
        cx.notify();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("agents-panel-button").is_some(),
        "the visible panel has a button while the sidebar is collapsed"
    );
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.read_with(cx, |model, _| {
        assert_eq!(model.agents_panel_placement(), Placement::Popup);
        assert!(matches!(model.overlay, Some(Overlay::Agents)));
        assert!(
            model.appearance.agents_panel_visible,
            "showing the popup keeps its button until the panel is hidden"
        );
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("agents-panel-button").is_some(),
        "escape keeps the button after the first popup opening"
    );
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(
            model.appearance.agents_panel_visible,
            "escape closes the popup but keeps the button"
        );
    });
    view.update_in(cx, AppModel::hide_agents_panel);
    view.read_with(cx, |model, _| {
        assert!(
            !model.appearance.agents_panel_visible,
            "hiding the panel removes the button"
        );
    });
    view.update(cx, |model, cx| {
        model.change_preference(crate::model::preferences::Change::AgentsPanel(true), cx);
        assert!(model.overlay.is_none());
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let button = cx
        .debug_bounds("agents-panel-button")
        .expect("button")
        .center();
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.update(cx, |model, cx| {
        assert!(model.agents_panel_shown());
        model.change_preference(crate::model::preferences::Change::AgentsPanel(false), cx);
        assert!(!model.agents_panel_shown());
        assert!(model.overlay.is_none());
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.simulate_click(button, Modifiers::none());
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.agents_panel_visible);
        assert!(
            !model.agents_panel_shown(),
            "the hidden button cannot reopen the popup"
        );
    });
}

#[gpui::test]
fn right_agents_share_the_panel_host_without_losing_the_composer_draft(cx: &mut TestAppContext) {
    use muxy_app_core::settings::ComposerPresentation;
    use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition};

    let (view, cx, fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_dock(AgentsPanelDock::Right, window, cx);
        model.settings.composer.presentation = ComposerPresentation::Panel;
        model.toggle_composer(window, cx);
        assert!(!model.agents_panel_shown());
        assert!(!model.settings.appearance.agents_panel_visible);
    });
    cx.run_until_parked();
    cx.simulate_input("keep this draft");
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.update_in(cx, |model, window, cx| {
        assert!(model.agents_panel_shown());
        assert!(model.composer.view.is_none());
        assert_eq!(model.panels.len(), 1);
        model.toggle_composer(window, cx);
        let composer = model
            .composer
            .view
            .as_ref()
            .expect("reopened composer")
            .read(cx);
        assert_eq!(composer.input.read(cx).text(), "keep this draft");
        model.toggle_agents_panel(window, cx);
        let extension =
            PanelPlacement::new("test-extension", PanelPosition::Right, PanelMode::Pinned);
        model.place_panel(extension, cx);
        assert!(!model.agents_panel_shown());
        assert!(!model.appearance.agents_panel_visible);
        model.toggle_agents_panel(window, cx);
        assert!(
            model
                .panels
                .placement(&PanelId::new("test-extension"))
                .is_none()
        );
        assert!(model.agents_panel.focus.contains_focused(window, cx));
        model.select_project(fixture.worktree, cx);
        assert!(
            model.agents_panel_shown(),
            "Agents is global across project switches"
        );
        let home = model.state.home().id;
        let saved = crate::model::webviews::panel_sessions::SavedPanel {
            owner: "test-extension".into(),
            kind: "right".into(),
            position: PanelPosition::Right,
            data: serde_json::Value::Null,
        };
        model.webviews.panel_sessions.save(home, saved.clone());
        model.select_project(home, cx);
        assert!(model.agents_panel_shown());
        assert!(model.webviews.panel_sessions.pending.is_none());
        assert_eq!(model.webviews.panel_sessions.take(home), Some(saved));
    });
    cx.run_until_parked();
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
    view.read_with(cx, |model, _| {
        assert!(!model.agents_panel_shown());
        assert!(model.panels.is_empty());
    });
}

#[gpui::test]
fn appearance_merges_keep_the_panel_host_in_sync_with_another_windows_dock_choice(
    cx: &mut TestAppContext,
) {
    let (view, cx, _fixture) = agents(cx);
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        model.save_appearance(cx);
        for dock in [AgentsPanelDock::Right, AgentsPanelDock::Left] {
            let mut other = model.appearance.clone();
            other.agents_panel_dock = dock;
            other
                .save(&model.path.with_file_name("settings.toml"))
                .expect("other window's choice");
            model.appearance.tips_visible = !model.appearance.tips_visible;
            model.save_appearance(cx);
            assert_eq!(model.appearance.agents_panel_dock, dock);
            assert!(
                model.agents_panel_shown(),
                "merging a dock choice must not lose the panel"
            );
            assert_eq!(model.panels.is_empty(), dock == AgentsPanelDock::Left);
        }
    });
}

#[gpui::test]
fn a_saved_floating_frame_stays_inside_a_smaller_window(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.simulate_resize(size(px(500.0), px(400.0)));
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_detached(true, window, cx);
        assert_eq!(model.agents_panel_placement(), Placement::Floating);
        model.appearance.agents_panel_frame = Some([2000.0, -50.0, 5000.0, 10.0]);
        let [x, y, width, height] = model.agents_panel_frame(window.viewport_size());
        assert!(x >= 0.0 && y >= 0.0);
        assert!(x + width <= 500.0 && y + height <= 400.0);
        assert!(height >= crate::model::agents_panel::MIN_HEIGHT);

        model.appearance.agents_panel_frame = Some([f32::NAN, 0.0, f32::NAN, f32::INFINITY]);
        let frame = model.agents_panel_frame(window.viewport_size());
        assert!(
            frame.iter().all(|value| value.is_finite()),
            "a hand-edited nan in settings.toml falls back instead of crashing: {frame:?}"
        );
        model.appearance.agents_panel_frame = Some([2000.0, -50.0, 5000.0, 10.0]);

        let origin = gpui::point(px(100.0), px(100.0));
        model.begin_agents_panel_drag(None, origin, window, cx);
        model.move_agents_panel_drag(gpui::point(px(-900.0), px(900.0)), window, cx);
        model.finish_agents_panel_drag(cx);
        let [x, y, _, height] = model.agents_panel_frame(window.viewport_size());
        assert!(x.abs() < 0.01, "dragged past the left edge, it stops there");
        assert!(
            (y + height - 400.0).abs() < 0.01,
            "dragged past the edge, it stops there"
        );
        assert_eq!(
            model.settings.appearance.agents_panel_frame, model.appearance.agents_panel_frame,
            "the dropped frame is saved"
        );

        model.set_agents_panel_detached(false, window, cx);
        assert!(!model.settings.appearance.agents_panel_detached);
    });
}

#[gpui::test]
fn the_floating_panel_moves_by_its_header_and_resizes_by_its_corner(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_detached(true, window, cx);
        model.appearance.agents_panel_frame = Some([100.0, 100.0, 300.0, 300.0]);
    });
    let drag =
        |selector: &'static str, by: gpui::Point<gpui::Pixels>, cx: &mut VisualTestContext| {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let bounds = cx.debug_bounds(selector).expect(selector);
            let start = bounds.center();
            cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(start + by, Some(gpui::MouseButton::Left), Modifiers::none());
            cx.simulate_mouse_up(start + by, gpui::MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
        };
    let frame = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| model.settings.appearance.agents_panel_frame)
            .expect("saved frame")
    };
    drag("agents-panel-resize", gpui::point(px(80.0), px(40.0)), cx);
    let [x, y, width, height] = frame(&view, cx);
    assert!(
        (x - 100.0).abs() < 0.5 && (y - 100.0).abs() < 0.5,
        "resizing keeps the origin"
    );
    assert!(
        (width - 380.0).abs() < 0.5 && (height - 340.0).abs() < 0.5,
        "the corner resizes: {width}x{height}"
    );
    drag("agents-panel-header", gpui::point(px(50.0), px(20.0)), cx);
    let [x, y, width, _] = frame(&view, cx);
    assert!(
        (x - 150.0).abs() < 0.5 && (y - 120.0).abs() < 0.5,
        "the header moves it: {x},{y}"
    );
    assert!((width - 380.0).abs() < 0.5, "moving keeps the size");
}

#[gpui::test]
fn compact_rows_name_their_project_and_normal_rows_sit_under_headings(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let long = "Investigate why the upload retry test is flaky on Linux runners";
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = true;
        model.appearance.agents_panel_density = AgentsPanelDensity::Compact;
        let mut snapshot = model.servers.local.activity.snapshot.clone();
        snapshot.revision += 1;
        snapshot.agents[0].title = Some(long.into());
        model.receive_activity(ServerId::local(), Ok(snapshot), cx);
        cx.notify();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let key = EntryKey::Agent(ServerId::local(), SessionId::new(1).expect("session"));
    let bounds = |cx: &mut VisualTestContext, selector: String| {
        cx.debug_bounds(selector.leak()).expect("bounds")
    };
    let row = bounds(cx, format!("agents-panel-row-{key}"));
    let project = bounds(cx, format!("agents-panel-project-label-{key}"));
    let title = bounds(cx, format!("agents-panel-title-{key}"));
    let status = bounds(cx, format!("agents-panel-status-{key}"));
    assert!(row.size.height <= px(24.0), "rows are dense: {row:?}");
    assert!(
        project.right() <= title.left(),
        "the project leads the title"
    );
    assert!(
        title.size.width > (row.size.width * 0.4),
        "a long title gets the row's room instead of a few letters: {title:?} in {row:?}"
    );
    assert!(status.left() >= title.right() && status.right() <= row.right());
    let heading = format!("agents-panel-heading-{}", fixture.root);
    assert!(
        cx.debug_bounds(heading.clone().leak()).is_none(),
        "projects are named on the rows, without heading lines"
    );
    view.read_with(cx, |model, _| {
        let agent = model
            .running_agents()
            .into_iter()
            .find(|agent| agent.session.get() == 1)
            .expect("agent");
        assert_eq!(model.agent_title(&agent), long);
    });
    assert!(
        cx.debug_bounds(format!("agents-panel-{key}-progress").leak())
            .is_some(),
        "a working agent shows the tab's progress glyph"
    );

    view.update(cx, |model, cx| {
        model.appearance.agents_panel_density = AgentsPanelDensity::Normal;
        cx.notify();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let heading = bounds(cx, heading);
    let row = bounds(cx, format!("agents-panel-row-{key}"));
    assert!(
        heading.bottom() <= row.top(),
        "normal rows sit under project headings"
    );
    assert!(
        row.size.height > px(24.0),
        "normal rows are as tall as sidebar tabs"
    );
    assert!(
        bounds(cx, format!("agents-panel-title-{key}")).left() < title.left(),
        "the heading names the project instead of each row"
    );
}

#[gpui::test]
fn the_docked_panel_lines_up_with_the_agents_focused_sidebar(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let tab = view.update(cx, |model, cx| {
        model.appearance.layout = muxy_app_core::settings::AppLayout::AgentsFocused;
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = true;
        model
            .appearance
            .tab_focused_expanded
            .insert(fixture.root, true);
        cx.notify();
        let root = model.state.project(fixture.root).expect("root");
        model.sidebar_tabs(root).next().expect("agent tab").id
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let key = EntryKey::Agent(ServerId::local(), SessionId::new(1).expect("session"));
    let bounds = |cx: &mut VisualTestContext, selector: String| {
        cx.debug_bounds(selector.leak()).expect("bounds")
    };
    let sidebar_row = bounds(cx, format!("sidebar-tab-{tab}"));
    let panel_row = bounds(cx, format!("agents-panel-row-{key}"));
    assert_eq!(panel_row.left(), sidebar_row.left());
    assert_eq!(panel_row.right(), sidebar_row.right());
}

/// Gives the root pane's agent `OpenCode` tabs: the one it shows, one its
/// shortcut reaches, one only resumable, one with an unsafe ID, one shown by
/// a terminal this window lost, and one whose only host no pane shows.
fn with_background_tabs(view: &Entity<AppModel>, cx: &mut VisualTestContext, root: ProjectId) {
    use muxy_protocol::AgentTab;
    let host = SessionId::new(1).expect("session");
    let gone = SessionId::new(99).expect("session");
    let tab = |id: &str, index, state| AgentTab {
        id: id.into(),
        project: root,
        provider: AgentProvider::OpenCode,
        title: format!("Tab {id}"),
        state,
        shown_by: None,
        index: Some(index),
        hosts: vec![host],
    };
    view.update(cx, |model, cx| {
        let mut snapshot = model.servers.local.activity.snapshot.clone();
        snapshot.revision += 1;
        snapshot.agents[0].provider = AgentProvider::OpenCode;
        snapshot.agents[0].title = Some("Tab ses_shown".into());
        snapshot.tabs = vec![
            AgentTab {
                shown_by: Some(host),
                ..tab("ses_shown", 0, AgentState::Working)
            },
            tab("ses_near", 1, AgentState::Working),
            tab("ses_far", 12, AgentState::Idle),
            tab("bad id; rm -rf ~", 13, AgentState::Idle),
            AgentTab {
                shown_by: Some(gone),
                hosts: vec![gone, host],
                ..tab("ses_handed", 3, AgentState::Idle)
            },
            AgentTab {
                hosts: vec![gone],
                ..tab("ses_unhosted", 2, AgentState::Working)
            },
        ];
        model.receive_activity(ServerId::local(), Ok(snapshot), cx);
    });
}

#[gpui::test]
fn background_agent_tabs_join_their_host_without_taking_its_place(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    let [root_pane, ..] = fixture.panes;
    with_background_tabs(&view, cx, fixture.root);
    view.update(cx, |model, cx| {
        let mut rows: Vec<_> = model
            .running_agents()
            .into_iter()
            .filter(|agent| agent.project == fixture.root && agent.pane == root_pane)
            .map(|agent| {
                (
                    model.agent_title(&agent),
                    agent.hidden.is_some(),
                    agent.status,
                )
            })
            .collect();
        rows.sort();
        assert_eq!(
            rows,
            [
                ("Tab bad id; rm -rf ~".to_owned(), true, AgentStatus::Idle),
                ("Tab ses_far".to_owned(), true, AgentStatus::Idle),
                ("Tab ses_handed".to_owned(), true, AgentStatus::Idle),
                ("Tab ses_near".to_owned(), true, AgentStatus::Working),
                ("Tab ses_shown".to_owned(), false, AgentStatus::Working),
            ],
            "the shown tab is the pane's own row, a lost terminal's tab moves under the host, \
             and a tab no pane here can reach is left out"
        );
        model.focus_pane(root_pane, cx);
        assert_eq!(
            model.active_agents_entry(&model.agents_panel_entries()),
            Some(EntryKey::Agent(
                ServerId::local(),
                SessionId::new(1).expect("session")
            )),
            "the focused pane marks the tab it shows, not its background tabs"
        );
    });
}

#[gpui::test]
fn background_agent_tabs_open_through_their_host_or_a_new_tab(cx: &mut TestAppContext) {
    use crate::boot::Work;
    use crate::model::agents_panel::hidden_key;

    let (view, cx, mut fixture) = agents(cx);
    let [root_pane, ..] = fixture.panes;
    with_background_tabs(&view, cx, fixture.root);
    fixture.sent.take();
    let tabs = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            model.state.project(fixture.root).expect("root").tabs.len()
        })
    };
    let before = tabs(&view, cx);
    let open = |view: &Entity<AppModel>, cx: &mut VisualTestContext, id: &str| {
        view.update_in(cx, |model, window, cx| {
            let key = EntryKey::Hidden(ServerId::local(), hidden_key(id));
            model.open_agents_entry(key, window, cx);
        });
    };

    open(&view, cx, "ses_near");
    assert!(
        fixture
            .sent
            .take()
            .iter()
            .any(|work| matches!(work, Work::Input(_, keys) if keys == &[0x18, b'2'])),
        "the host switches to its second tab with OpenCode's leader shortcut"
    );
    assert_eq!(
        view.read_with(cx, |model, _| model.active_pane()),
        Some(root_pane)
    );
    assert_eq!(tabs(&view, cx), before);

    open(&view, cx, "ses_far");
    view.read_with(cx, |model, _| {
        let project = model.state.project(fixture.root).expect("root");
        assert_eq!(project.tabs.len(), before + 1, "no shortcut reaches tab 13");
        let pane = project.tabs.last().expect("new tab").panes[0].id;
        assert_eq!(
            model.state.startup_command(pane),
            Some("opencode -s ses_far")
        );
    });

    open(&view, cx, "bad id; rm -rf ~");
    assert_eq!(
        tabs(&view, cx),
        before + 1,
        "an ID that isn't safe on a command line opens nothing"
    );
}

#[gpui::test]
fn a_background_tab_in_another_project_switches_once_its_host_attaches(cx: &mut TestAppContext) {
    use crate::boot::Work;
    use crate::model::agents_panel::hidden_key;

    let (view, cx, mut fixture) = agents(cx);
    let [root_pane, ..] = fixture.panes;
    with_background_tabs(&view, cx, fixture.root);
    let before = view.read_with(cx, |model, _| {
        model.state.project(fixture.root).expect("root").tabs.len()
    });
    view.update_in(cx, |model, window, cx| {
        model.grids.remove(&root_pane);
        let home = model.state.home().id;
        model.select_project(home, cx);
        let key = EntryKey::Hidden(ServerId::local(), hidden_key("ses_near"));
        model.open_agents_entry(key, window, cx);
        assert_eq!(model.state.current_project().id, fixture.root);
        assert_eq!(model.active_pane(), Some(root_pane));
        assert_eq!(
            model.state.project(fixture.root).expect("root").tabs.len(),
            before,
            "the host switches instead of a new tab opening"
        );
    });
    fixture.sent.take();
    view.update(cx, |model, cx| {
        let session = SessionId::new(1).expect("session");
        model.receive_attached(
            ServerId::local(),
            root_pane,
            session,
            attachment(),
            true,
            cx,
        );
    });
    assert!(
        fixture
            .sent
            .take()
            .iter()
            .any(|work| matches!(work, Work::Input(_, keys) if keys == &[0x18, b'2'])),
        "the switch waits for the host to attach"
    );
}

#[gpui::test]
fn switching_agent_tabs_or_reading_a_finish_keeps_rows_in_place(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    with_background_tabs(&view, cx, fixture.root);
    let titles = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            model
                .agents_panel_entries()
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Agent(agent) => Some(model.agent_title(agent)),
                    Entry::Worktree { .. } => None,
                })
                .collect::<Vec<_>>()
        })
    };
    let before = titles(&view, cx);
    view.update(cx, |model, cx| {
        let host = SessionId::new(1).expect("session");
        let mut snapshot = model.servers.local.activity.snapshot.clone();
        snapshot.revision += 1;
        snapshot.agents[0].title = Some("Tab ses_near".into());
        for tab in &mut snapshot.tabs {
            match tab.id.as_str() {
                "ses_shown" => tab.shown_by = None,
                "ses_near" => tab.shown_by = Some(host),
                _ => {}
            }
        }
        for event in &mut snapshot.events {
            event.read = true;
        }
        model.receive_activity(ServerId::local(), Ok(snapshot), cx);
    });
    assert_eq!(
        titles(&view, cx),
        before,
        "the host showing another tab, or a finish being read, moves no row"
    );
}

#[gpui::test]
fn pressing_a_row_highlights_it_instead_of_the_agent_being_left(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    let [root_pane, _, _, home_pane] = fixture.panes;
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = true;
        model.focus_pane(root_pane, cx);
        cx.notify();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let local = ServerId::local();
    let home = EntryKey::Agent(local, SessionId::new(3).expect("session"));
    let row = cx
        .debug_bounds(format!("agents-panel-row-{home}").leak())
        .expect("row")
        .center();
    cx.simulate_mouse_down(row, gpui::MouseButton::Left, Modifiers::none());
    view.read_with(cx, |model, _| {
        let entries = model.agents_panel_entries();
        assert_eq!(
            model.selected_agents_entry(&entries),
            Some(home),
            "the pressed row lights up, not the active agent"
        );
    });
    cx.simulate_mouse_up(row, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(home_pane));
        assert_eq!(
            model.active_agents_entry(&model.agents_panel_entries()),
            Some(home)
        );
    });
}

#[gpui::test]
fn a_switching_tab_stays_active_instead_of_the_tab_it_replaces(cx: &mut TestAppContext) {
    use crate::model::agents_panel::hidden_key;

    let (view, cx, fixture) = agents(cx);
    let [_, _, _, home_pane] = fixture.panes;
    with_background_tabs(&view, cx, fixture.root);
    let local = ServerId::local();
    let host = SessionId::new(1).expect("session");
    let near = EntryKey::Hidden(local, hidden_key("ses_near"));
    let active = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            model.active_agents_entry(&model.agents_panel_entries())
        })
    };
    view.update_in(cx, |model, window, cx| {
        model.open_agents_entry(near, window, cx);
    });
    assert_eq!(
        active(&view, cx),
        Some(near),
        "the clicked tab is active while its host still shows the old one"
    );

    view.update(cx, |model, cx| {
        let mut snapshot = model.servers.local.activity.snapshot.clone();
        snapshot.revision += 1;
        snapshot.agents[0].title = Some("Tab ses_near".into());
        for tab in &mut snapshot.tabs {
            match tab.id.as_str() {
                "ses_shown" => tab.shown_by = None,
                "ses_near" => tab.shown_by = Some(host),
                _ => {}
            }
        }
        model.receive_activity(local, Ok(snapshot), cx);
    });
    assert_eq!(
        active(&view, cx),
        Some(EntryKey::Agent(local, host)),
        "once shown, the tab is its terminal's own row"
    );

    view.update_in(cx, |model, window, cx| {
        let shown = EntryKey::Hidden(local, hidden_key("ses_shown"));
        model.open_agents_entry(shown, window, cx);
        let home = model.state.home().id;
        model.select_project(home, cx);
        model.focus_pane(home_pane, cx);
    });
    assert_eq!(
        active(&view, cx),
        Some(EntryKey::Agent(local, SessionId::new(3).expect("session"))),
        "focusing another terminal mid-switch moves the highlight there"
    );
}
