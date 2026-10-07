use super::*;
use muxy_protocol::ChannelId;

#[gpui::test]
#[ignore = "requires MUXY_TEST_TUI and a fresh MUXY_DIR under /tmp/muxy-tui-desktop-"]
fn nested_tui_and_desktop_share_a_session_with_independent_layouts(cx: &mut TestAppContext) {
    walkthrough(cx).expect("desktop/TUI coexistence");
}

fn walkthrough(cx: &mut TestAppContext) -> Result {
    let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
    assert!(
        directory
            .to_string_lossy()
            .starts_with("/tmp/muxy-tui-desktop-")
    );
    let executable = PathBuf::from(std::env::var("MUXY_TEST_TUI")?).canonicalize()?;
    let boot = Boot::load()?;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_tab);
    wait_live(cx, &view)?;
    shell(cx, "printf '\\nHOST_READY\\n'");
    wait_text(cx, &view, "HOST_READY")?;
    let hosting = active_session(&view, cx)?;
    let probe = Client::connect(&directory.join("server.sock"))?;
    let channel = probe
        .attach(
            hosting,
            Size {
                cols: 100,
                rows: 26,
            },
        )?
        .channel;
    shell(
        cx,
        &format!(
            "'{}'; printf '\\nTUI_DETACHED\\n'",
            executable.display().to_string().replace('\'', "'\\''")
        ),
    );
    wait_text(cx, &view, "Ctrl-B ? help")?;
    wait(cx, &view, |_, _| {
        probe
            .list_sessions()
            .is_ok_and(|sessions| sessions.len() == 2)
    })?;
    let catalog = probe.catalog()?;
    let session = probe
        .project_sessions(catalog.home, None, None)?
        .sessions
        .into_iter()
        .find(|session| session.info.id != hosting)
        .ok_or("TUI session")?;
    let state = saved_layout(&directory, catalog.home, session.info.id)?;
    check_picker(cx, &view, &probe, channel, hosting)?;
    let project = view.read_with(cx, |model, _| model.state.home().id);
    view.update(cx, |model, cx| {
        model.open_existing_session(project, &session, cx);
    });
    wait_live(cx, &view)?;
    assert_eq!(active_session(&view, cx)?, session.info.id);
    probe.send_input(channel, b"printf '\\nTUI_TO_DESKTOP\\n'\r")?;
    wait_text(cx, &view, "TUI_TO_DESKTOP")?;
    shell(cx, "printf '\\nDESKTOP_TO_TUI\\n'");
    wait_text(cx, &view, "DESKTOP_TO_TUI")?;
    view.update(cx, |model, cx| {
        model.select_tab(model.state.home().tabs[0].id, cx);
    });
    wait(cx, &view, |model, cx| {
        active_grid(model, cx).is_some_and(|grid| {
            grid.rows
                .iter()
                .enumerate()
                .any(|(index, _)| grid.row_text(index).contains("DESKTOP_TO_TUI"))
        })
    })?;
    assert_eq!(active_session(&view, cx)?, hosting);
    probe.send_input(channel, b"\x02d")?;
    wait(cx, &view, |model, cx| {
        active_grid(model, cx).is_some_and(|grid| {
            (0..grid.rows.len()).any(|index| grid.row_text(index).trim() == "TUI_DETACHED")
        })
    })?;
    assert_eq!(probe.list_sessions()?.len(), 2);
    assert_eq!(tui_layout(&directory)?, state);
    assert_eq!(
        view.read_with(cx, |model, _| model.state.home().tabs.len()),
        2
    );
    shared_closes(cx, &view, &probe, channel, &session, &executable)?;
    detach::verify_live_detach(cx, &view, &probe)?;
    view.update(cx, |model, _| model.stop_workers());
    crate::server::stop_server(&probe, &directory.join("server.sock"))?;
    report(
        "Nested live TUI excludes its desktop host; bidirectional shared-session output and independent layouts survive detach: PASS",
    )
}

fn saved_layout(
    directory: &std::path::Path,
    home: ProjectId,
    session: SessionId,
) -> Result<serde_json::Value> {
    let state = tui_layout(directory)?;
    let panes = &state["projects"][home.to_string()]["tabs"][0]["panes"];
    assert_eq!(panes.as_object().ok_or("panes")?.len(), 1);
    assert!(
        panes
            .as_object()
            .ok_or("panes")?
            .values()
            .all(|pane| pane["session"] == session.get())
    );
    Ok(state)
}

/// The TUI's saved state without the catalog revision it last saw, which
/// moves with every session change on the server.
fn tui_layout(directory: &std::path::Path) -> Result<serde_json::Value> {
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("tui-state.json"))?)?;
    state
        .as_object_mut()
        .ok_or("TUI state")?
        .remove("catalog_revision");
    Ok(state)
}

fn check_picker(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    probe: &Client,
    channel: ChannelId,
    hosting: SessionId,
) -> Result {
    probe.send_input(channel, b"\x02w")?;
    wait_text(cx, view, "Existing terminals")?;
    assert!(!screen(view, cx).contains(&hosting.get().to_string()));
    probe.send_input(channel, b"\x1b")?;
    wait(cx, view, |model, cx| {
        active_grid(model, cx).is_some_and(|grid| {
            !grid
                .rows
                .iter()
                .enumerate()
                .any(|(index, _)| grid.row_text(index).contains("Existing terminals"))
        })
    })?;
    Ok(())
}

fn shared_closes(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    probe: &Client,
    channel: ChannelId,
    session: &muxy_protocol::ProjectSession,
    executable: &std::path::Path,
) -> Result {
    shell(
        cx,
        &format!(
            "'{}'; printf '\\nSECOND_DETACHED\\n'",
            executable.display().to_string().replace('\'', "'\\''")
        ),
    );
    wait_text(cx, view, "Ctrl-B ? help")?;
    let project = view.read_with(cx, |model, _| model.state.home().id);
    let host_tab = view.read_with(cx, |model, _| model.state.home().tabs[0].id);
    let original = view.read_with(cx, |model, _| model.state.home().tabs[1].id);
    add_duplicate_tab(cx, view, project, session);
    wait_live(cx, view)?;
    let duplicate = view.read_with(cx, |model, _| model.active_tab().expect("tab"));
    view.update(cx, |model, cx| model.close_tab(duplicate, cx));
    wait(cx, view, |model, _| model.state.home().tabs.len() == 2)?;
    assert!(
        probe
            .list_sessions()?
            .iter()
            .any(|info| info.id == session.info.id)
    );

    probe.send_input(channel, b"\x02c")?;
    wait(cx, view, |_, _| {
        probe
            .list_sessions()
            .is_ok_and(|sessions| sessions.len() == 3)
    })?;
    view.update(cx, |model, cx| model.close_tab(original, cx));
    wait(cx, view, |model, _| {
        model.state.home().tabs.len() == 1
            && model.state.pending_discards(ServerId::local()).is_empty()
    })?;
    assert!(
        probe
            .list_sessions()?
            .iter()
            .any(|info| info.id == session.info.id)
    );

    view.update(cx, |model, cx| {
        model.open_existing_session(project, session, cx);
    });
    wait_live(cx, view)?;
    let last = view.read_with(cx, |model, _| model.active_tab().expect("last desktop tab"));
    view.update(cx, |model, cx| model.select_tab(host_tab, cx));
    wait_live(cx, view)?;
    probe.send_input(channel, b"\x020")?;
    wait(cx, view, |_, _| {
        let directory = std::env::var("MUXY_DIR").expect("isolated profile");
        let Ok(bytes) = std::fs::read(std::path::Path::new(&directory).join("tui-state.json"))
        else {
            return false;
        };
        let Ok(state) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return false;
        };
        state["projects"][state["active"].as_str().unwrap_or_default()]["active"] == 0
    })?;
    probe.send_input(channel, b"\x02x")?;
    wait_text(cx, view, "Ctrl-B ? help")?;
    wait(cx, view, |_, _| {
        let directory = std::env::var("MUXY_DIR").expect("isolated profile");
        let Ok(bytes) = std::fs::read(std::path::Path::new(&directory).join("tui-state.json"))
        else {
            return false;
        };
        let Ok(state) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return false;
        };
        state["projects"][state["active"].as_str().unwrap_or_default()]["tabs"]
            .as_array()
            .is_some_and(|tabs| tabs.len() == 1)
    })?;
    assert!(
        probe
            .list_sessions()?
            .iter()
            .any(|info| info.id == session.info.id)
    );
    view.update(cx, |model, cx| model.close_tab(last, cx));
    wait(cx, view, |model, _| {
        model.state.pending_discards(ServerId::local()).is_empty()
            && model.state.home().tabs.len() == 1
    })?;
    assert!(
        !probe
            .list_sessions()?
            .iter()
            .any(|info| info.id == session.info.id)
    );
    probe.send_input(channel, b"\x02d")?;
    wait_text(cx, view, "SECOND_DETACHED")?;
    report(
        "Duplicate desktop tabs and hidden desktop/TUI tabs preserve a shared session; final tab close ends it: PASS",
    )
}

fn add_duplicate_tab(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    project: ProjectId,
    session: &muxy_protocol::ProjectSession,
) {
    view.update(cx, |model, cx| {
        model.open_existing_session(project, session, cx);
        assert_eq!(model.state.home().tabs.len(), 2);
        model
            .state
            .open_terminal_tab(project)
            .expect("duplicate tab");
        let pane = model.state.window().active_pane.expect("duplicate pane");
        model
            .state
            .set_pane_session(pane, Some(session.info.id))
            .expect("duplicate session");
        assert!(model.save(cx));
        model.sync_visible(cx);
    });
}
