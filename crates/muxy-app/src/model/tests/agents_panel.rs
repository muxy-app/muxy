use super::worktrees::{Sent, connected, pruning_fixture};
use super::*;
use crate::model::agents_panel::{AgentStatus, Entry, EntryKey, Placement};
use gpui::Modifiers;
use muxy_app_core::settings::{AgentsPanelDock, AgentsPanelGrouping, AgentsPanelIcon, AppLayout};
use muxy_core::shortcuts::ShortcutId;
use muxy_protocol::{
    ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity, AgentProvider, AgentState,
};

struct Fixture {
    root: ProjectId,
    worktree: ProjectId,
    panes: [PaneId; 4],
    _sent: Sent,
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
            _sent: sent,
        },
    )
}

#[gpui::test]
fn the_panel_lists_each_shown_session_once_most_urgent_first(cx: &mut TestAppContext) {
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
        model.appearance.agents_panel_grouping = AgentsPanelGrouping::Worktrees;
        let entries = model.agents_panel_entries();
        assert!(matches!(
            entries.first(),
            Some(Entry::Worktree { project, counts: [1, 0, 0, 0] }) if *project == fixture.worktree
        ));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            Entry::Worktree { project, counts: [0, 1, 0, 0] } if *project == fixture.root
        )));
        assert_eq!(entries.len(), 3);
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
fn the_active_card_follows_tabs_and_projects_without_a_fallback(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    view.update_in(cx, |model, window, cx| {
        let active = |model: &AppModel| model.active_agents_entry(&model.agents_panel_entries());
        let local = ServerId::local();
        let key = |id| EntryKey::Agent(local, SessionId::new(id).expect("session"));
        let root_tab = model.pane_tab(fixture.panes[0]).expect("root tab");
        assert_eq!(active(model), Some(key(1)));

        model.open_agents_entry(key(2), window, cx);
        assert_eq!(active(model), Some(key(2)));
        model.select_tab(root_tab, cx);
        assert_eq!(active(model), Some(key(1)));

        let tab = model.state.open_terminal_tab(fixture.root).expect("tab");
        assert_eq!(active(model), None, "a plain terminal has no active agent");
        let pane = model.active_pane().expect("pane");
        model.receive_attached(
            local,
            pane,
            SessionId::new(4).expect("session"),
            attachment(),
            true,
            cx,
        );
        assert_eq!(active(model), Some(key(4)));
        model.select_tab(root_tab, cx);
        assert_eq!(active(model), Some(key(1)));
        model.select_tab(tab, cx);
        assert_eq!(active(model), Some(key(4)));

        model
            .servers
            .local
            .activity
            .snapshot
            .agents
            .retain(|agent| agent.session != SessionId::new(4).expect("session"));
        assert_eq!(
            active(model),
            None,
            "an agent that stops leaves no stale highlight"
        );
        model.appearance.agents_panel_grouping = AgentsPanelGrouping::Worktrees;
        assert_eq!(active(model), Some(EntryKey::Worktree(fixture.root)));
        model.select_project(fixture.worktree, cx);
        assert_eq!(active(model), Some(EntryKey::Worktree(fixture.worktree)));
        let empty = model
            .state
            .projects()
            .iter()
            .find(|project| project.parent_id == Some(fixture.root) && project.tabs.is_empty())
            .expect("empty worktree")
            .id;
        model.select_project(empty, cx);
        assert_eq!(
            active(model),
            None,
            "an unlisted project has no active card"
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
fn long_agent_labels_stay_inside_the_card_in_every_placement(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let agent = view.update(cx, |model, _| {
        let agent = model
            .running_agents()
            .into_iter()
            .find(|agent| agent.pane == fixture.panes[2])
            .expect("worktree agent");
        model
            .state
            .set_tab_title(
                agent.tab,
                Some("Investigate a very long agent task title ".repeat(10)),
            )
            .expect("title");
        model
            .state
            .rename_project(fixture.root, &"long-project-name-".repeat(10))
            .expect("project name");
        model
            .state
            .rename_project(fixture.worktree, &"long-worktree-name-".repeat(10))
            .expect("worktree name");
        agent
    });
    for (dock, expanded, detached) in [
        (AgentsPanelDock::Left, true, false),
        (AgentsPanelDock::Right, true, false),
        (AgentsPanelDock::Left, false, false),
        (AgentsPanelDock::Left, true, true),
    ] {
        for (grouping, icon, colors) in [
            (AgentsPanelGrouping::Agents, AgentsPanelIcon::Agent, true),
            (AgentsPanelGrouping::Agents, AgentsPanelIcon::Project, true),
            (AgentsPanelGrouping::Worktrees, AgentsPanelIcon::Agent, true),
            (
                AgentsPanelGrouping::Worktrees,
                AgentsPanelIcon::Project,
                false,
            ),
        ] {
            view.update(cx, |model, cx| {
                model.appearance.agents_panel_visible = true;
                model.appearance.agents_panel_dock = dock;
                model.appearance.sidebar_expanded = expanded;
                model.appearance.agents_panel_detached = detached;
                model.appearance.agents_panel_grouping = grouping;
                model.appearance.agents_panel_icon = icon;
                model.appearance.agents_panel_project_colors = colors;
                model.appearance.agents_panel_right_width = Some(220.0);
                model.appearance.agents_panel_frame = Some([80.0, 80.0, 220.0, 360.0]);
                model.overlay = (!expanded).then_some(Overlay::Agents);
                cx.notify();
            });
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let key = match grouping {
                AgentsPanelGrouping::Agents => EntryKey::Agent(agent.server, agent.session),
                AgentsPanelGrouping::Worktrees => EntryKey::Worktree(fixture.worktree),
            };
            let row = cx
                .debug_bounds(format!("agents-panel-row-{key}").leak())
                .expect("agent card");
            let panel = cx.debug_bounds("agents-panel").expect("panel");
            let max_line_height = view.read_with(cx, |model, _| model.metrics.font_body() * 2.0);
            assert!(row.right() <= panel.right(), "card fits in the panel");
            if colors {
                let stripe = cx
                    .debug_bounds(format!("agents-panel-color-{key}").leak())
                    .expect("project color");
                assert!(
                    stripe.left() >= row.left() && stripe.bottom() <= row.bottom(),
                    "project color fits inside the card"
                );
            }
            if icon == AgentsPanelIcon::Project {
                let tile = cx
                    .debug_bounds(format!("agents-panel-project-{key}").leak())
                    .expect("project icon");
                assert!(
                    tile.left() >= row.left() && tile.bottom() <= row.bottom(),
                    "project icon fits inside the card"
                );
            }
            for label in ["title", "detail"] {
                if label == "detail" && grouping == AgentsPanelGrouping::Worktrees {
                    continue;
                }
                let bounds = cx
                    .debug_bounds(format!("agents-panel-{label}-{key}").leak())
                    .expect("label");
                assert!(bounds.right() < row.right(), "{label} fits inside the card");
                assert!(bounds.size.width > px(0.0), "{label} remains visible");
                assert!(
                    bounds.size.height < max_line_height,
                    "{label} stays on one line"
                );
            }
        }
    }
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
        model.focus_requested = true;
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
fn the_layout_menu_shows_and_hides_the_panel_without_changing_layout(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    let click = |selector: &'static str, cx: &mut VisualTestContext| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let bounds = cx.debug_bounds(selector).expect(selector);
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    for detached in [false, true] {
        view.update_in(cx, |model, window, cx| {
            model.appearance.layout = AppLayout::TabFocused;
            model.appearance.sidebar_expanded = true;
            model.appearance.agents_panel_visible = false;
            model.appearance.agents_panel_detached = detached;
            model.focus_active(window, cx);
            cx.notify();
        });
        click("layout-menu", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.menu_outline()[0].last().map(String::as_str),
                Some("Show agents panel")
            );
        });
        click("menu-item-3", cx);
        view.update_in(cx, |model, window, cx| {
            assert!(model.overlay.is_none());
            assert!(model.agents_panel_shown());
            assert!(model.settings.appearance.agents_panel_visible);
            assert!(model.agents_panel.focus.contains_focused(window, cx));
            assert_eq!(model.appearance.layout, AppLayout::TabFocused);
            model.focus_active(window, cx);
        });

        click("layout-menu", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.menu_outline()[0].last().map(String::as_str),
                Some("Hide agents panel")
            );
        });
        click("menu-item-3", cx);
        view.update_in(cx, |model, window, cx| {
            assert!(model.overlay.is_none());
            assert!(!model.agents_panel_shown());
            assert!(!model.settings.appearance.agents_panel_visible);
            assert!(!model.agents_panel.focus.contains_focused(window, cx));
            assert_eq!(model.appearance.layout, AppLayout::TabFocused);
        });
    }
}

#[gpui::test]
fn the_panel_keeps_keyboard_focus_over_an_empty_project(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    for (dock, detached) in [
        (AgentsPanelDock::Left, false),
        (AgentsPanelDock::Right, false),
        (AgentsPanelDock::Right, true),
    ] {
        view.update(cx, |model, cx| {
            model.appearance.agents_panel_dock = dock;
            let empty = model
                .state
                .projects()
                .iter()
                .find(|project| project.parent_id == Some(fixture.root) && project.tabs.is_empty())
                .expect("empty worktree")
                .id;
            model.appearance.sidebar_expanded = true;
            model.appearance.agents_panel_detached = detached;
            model.appearance.agents_panel_visible = false;
            model.select_project(empty, cx);
            assert!(model.active_pane().is_none());
        });
        cx.run_until_parked();
        cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
        cx.run_until_parked();
        view.update_in(cx, |model, window, cx| {
            assert!(model.agents_panel.focus.contains_focused(window, cx));
        });
        cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
        cx.run_until_parked();
        view.read_with(cx, |model, _| assert!(!model.agents_panel_shown()));

        cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |model, _| {
            assert_eq!(model.active_pane(), Some(fixture.panes[2]));
            assert_eq!(model.state.current_project().id, fixture.worktree);
        });
        view.update_in(cx, |model, window, cx| {
            model.focus_agents_panel(window);
            model.focus_requested = true;
            cx.notify();
        });
        cx.run_until_parked();
        view.update_in(cx, |model, window, cx| {
            assert!(!model.agents_panel.focus.contains_focused(window, cx));
        });
    }
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
        model.change_preference(crate::views::settings::Change::AgentsPanel(true), cx);
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
        model.change_preference(crate::views::settings::Change::AgentsPanel(false), cx);
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
fn keyboard_navigation_keeps_the_active_card_until_confirmed(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    view.update_in(cx, |model, window, cx| {
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = true;
        model.focus_agents_panel(window);
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("up");
    view.read_with(cx, |model, _| {
        let entries = model.agents_panel_entries();
        assert_eq!(model.state.current_project().id, fixture.root);
        assert_eq!(
            model.active_agents_entry(&entries),
            Some(EntryKey::Agent(
                ServerId::local(),
                SessionId::new(1).expect("session")
            ))
        );
        assert_eq!(
            model.selected_agents_entry(&entries),
            Some(EntryKey::Agent(
                ServerId::local(),
                SessionId::new(2).expect("session")
            ))
        );
    });
    cx.simulate_keystrokes("escape");
    view.update_in(cx, |model, window, cx| {
        model.focus_agents_panel(window);
        let entries = model.agents_panel_entries();
        assert_eq!(
            model.selected_agents_entry(&entries),
            model.active_agents_entry(&entries),
            "refocusing discards the temporary cursor"
        );
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("up enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, fixture.worktree);
        assert_eq!(model.active_pane(), Some(fixture.panes[2]));
    });
    view.update_in(cx, |model, window, cx| {
        model.select_project(fixture.root, cx);
        model.focus_agents_panel(window);
        let entries = model.agents_panel_entries();
        assert_eq!(
            model.selected_agents_entry(&entries),
            model.active_agents_entry(&entries),
            "keyboard navigation restarts at the newly active tab"
        );
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, fixture.root);
        assert_eq!(model.active_pane(), Some(fixture.panes[0]));
    });
}

#[gpui::test]
fn keyboard_navigation_uses_the_highlighted_row_after_selection_changes(cx: &mut TestAppContext) {
    let (view, cx, fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    view.update_in(cx, |model, window, cx| {
        model.appearance.sidebar_expanded = true;
        model.appearance.agents_panel_visible = true;
        model.focus_agents_panel(window);
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("up enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(fixture.panes[2]));
        assert_eq!(model.state.current_project().id, fixture.worktree);
    });

    view.update_in(cx, |model, window, cx| {
        model
            .servers
            .local
            .activity
            .snapshot
            .agents
            .retain(|agent| agent.session != SessionId::new(2).expect("session"));
        model.focus_agents_panel(window);
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(fixture.panes[0]));
        assert_eq!(model.state.current_project().id, fixture.root);
    });

    view.update_in(cx, |model, window, cx| {
        model.appearance.agents_panel_grouping = AgentsPanelGrouping::Worktrees;
        model.focus_agents_panel(window);
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, model.state.home().id);
    });
}

#[gpui::test]
fn header_controls_move_between_sides_and_reattach_to_the_last_side(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let click = |selector: &'static str, cx: &mut VisualTestContext| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let bounds = cx.debug_bounds(selector).expect(selector);
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = true;
            cx.notify();
        });
        click("agents-panel-move", cx);
        view.update_in(cx, |model, window, cx| {
            assert_eq!(model.agents_panel_placement(), Placement::Right);
            assert!(model.agents_panel.focus.contains_focused(window, cx));
            assert_eq!(
                model.settings.appearance.agents_panel_dock,
                AgentsPanelDock::Right
            );
            model.appearance.sidebar_expanded = false;
            cx.notify();
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("agents-panel").expect("right panel");
        assert!(
            bounds.left() > px(800.0),
            "Agents occupies the right, not the sidebar"
        );
        view.update(cx, |model, cx| {
            assert!(crate::views::agents_panel::collapsed_button(model, cx).is_none());
        });
        click("agents-panel-dock", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(model.agents_panel_placement(), Placement::Floating);
            assert!(model.panels.is_empty());
        });
        click("agents-panel-dock", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(model.agents_panel_placement(), Placement::Right);
            assert!(model.agents_panel_shown());
        });
        click("agents-panel-move", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(model.agents_panel_placement(), Placement::Popup);
            assert!(model.panels.is_empty());
            assert_eq!(
                model.settings.appearance.agents_panel_dock,
                AgentsPanelDock::Left
            );
        });
        cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
        click("agents-panel-move", cx);
        view.read_with(cx, |model, _| {
            assert!(
                model.overlay.is_none(),
                "moving out of a popup dismisses it"
            );
            assert_eq!(model.agents_panel_placement(), Placement::Right);
        });
        view.update_in(cx, |model, window, cx| {
            model.set_agents_panel_detached(true, window, cx);
        });
        click("agents-panel-move", cx);
        view.read_with(cx, |model, _| {
            assert_eq!(model.agents_panel_placement(), Placement::Popup);
            assert!(!model.appearance.agents_panel_detached);
        });
    }
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
fn right_panel_width_is_saved_on_release_and_restored_at_startup(cx: &mut TestAppContext) {
    use muxy_app_core::settings::Settings;

    let settings = {
        let (view, cx, _fixture) = agents(cx);
        cx.simulate_resize(size(px(1200.0), px(800.0)));
        view.update_in(cx, |model, window, cx| {
            model.set_agents_panel_dock(AgentsPanelDock::Right, window, cx);
        });
        cx.run_until_parked();
        let start = cx
            .debug_bounds("panel-resize-builtin:agents")
            .expect("resize grip")
            .center();
        let end = start - gpui::point(px(80.0), px(0.0));
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.settings.appearance.agents_panel_right_width, None,
                "no disk writes during the drag"
            );
        });
        cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            let settings = Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("saved settings");
            let width = settings
                .appearance
                .agents_panel_right_width
                .expect("saved width");
            assert!((width - 400.0).abs() < 1.0);
            settings
        })
    };
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings = settings;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.agents_panel_shown());
        assert_eq!(model.agents_panel_placement(), Placement::Right);
    });
    let bounds = cx
        .debug_bounds("agents-panel")
        .expect("restored right panel");
    assert!((f32::from(bounds.size.width) - 400.0).abs() < 1.0);
    cx.simulate_resize(size(px(500.0), px(400.0)));
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("agents-panel")
        .expect("clamped right panel");
    assert!(bounds.left() >= px(100.0) && bounds.right() <= px(500.0));
    view.update(cx, |model, cx| {
        model.appearance.agents_panel_right_width = Some(f32::NAN);
        cx.notify();
    });
    cx.run_until_parked();
    let bounds = cx.debug_bounds("agents-panel").expect("finite right panel");
    assert!(f32::from(bounds.size.width).is_finite());
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
fn the_shortcut_uses_the_merged_dock_choice_with_a_collapsed_sidebar(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.update(|_, cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
    for (from, to) in [
        (AgentsPanelDock::Right, AgentsPanelDock::Left),
        (AgentsPanelDock::Left, AgentsPanelDock::Right),
    ] {
        view.update_in(cx, |model, window, cx| {
            model.appearance.sidebar_expanded = false;
            model.appearance.agents_panel_dock = from;
            model.hide_agents_panel(window, cx);
            let mut other = model.appearance.clone();
            other.agents_panel_dock = to;
            other
                .save(&model.path.with_file_name("settings.toml"))
                .expect("other window's choice");
        });
        cx.simulate_keystrokes(ShortcutId::ToggleAgentsPanel.definition().keys[0]);
        cx.run_until_parked();
        view.update_in(cx, |model, window, cx| {
            assert_eq!(model.appearance.agents_panel_dock, to);
            assert!(
                model.agents_panel_shown(),
                "the first shortcut must show the merged placement"
            );
            assert!(model.agents_panel.focus.contains_focused(window, cx));
            assert_eq!(
                matches!(model.overlay, Some(Overlay::Agents)),
                to == AgentsPanelDock::Left
            );
            assert_eq!(model.panels.is_empty(), to == AgentsPanelDock::Left);
        });
    }
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_dock(AgentsPanelDock::Left, window, cx);
        model.toggle_agents_panel(window, cx);
        assert!(matches!(model.overlay, Some(Overlay::Agents)));
        let mut other = model.appearance.clone();
        other.agents_panel_dock = AgentsPanelDock::Right;
        other
            .save(&model.path.with_file_name("settings.toml"))
            .expect("other window's choice");
        model.appearance.tips_visible = !model.appearance.tips_visible;
        model.save_appearance(cx);
        assert!(
            model.overlay.is_none(),
            "an unrelated save must not leave a duplicate popup"
        );
        assert!(model.agents_panel_shown());
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
fn the_docked_panel_resizes_by_its_top_edge_and_scrolls_by_its_thumb(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = agents(cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let drag = |selector: &'static str, by: gpui::Pixels, cx: &mut VisualTestContext| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let start = cx.debug_bounds(selector).expect(selector).center();
        let end = start + gpui::point(px(0.0), by);
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
        cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    };

    let scrolled = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| -model.agents_panel.scroll.offset().y)
    };
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = true;
            model.appearance.agents_panel_visible = true;
            model.appearance.agents_panel_docked_height =
                Some(crate::model::agents_panel::MIN_HEIGHT);
            model
                .agents_panel
                .scroll
                .set_offset(gpui::point(px(0.0), px(0.0)));
            cx.notify();
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(
            view.read_with(cx, |model, _| model.agents_panel.scroll.max_offset().height) > px(0.0),
            "the short panel overflows in {layout:?}"
        );
        drag("agents-panel-scrollbar", px(40.0), cx);
        assert!(
            scrolled(&view, cx) > px(0.0),
            "the thumb scrolls the list in {layout:?}"
        );

        drag("agents-panel-docked-resize", px(-60.0), cx);
        let height = view.read_with(cx, |model, _| {
            model.settings.appearance.agents_panel_docked_height
        });
        let expected = crate::model::agents_panel::MIN_HEIGHT + 60.0;
        assert!(
            height.is_some_and(|height| (height - expected).abs() < 1.0),
            "dragging the top edge up makes it taller and is saved in {layout:?}: {height:?}"
        );
    }
}
