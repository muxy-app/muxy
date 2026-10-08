#[path = "tui/applications.rs"]
mod applications;
#[path = "tui/faults.rs"]
mod faults;
#[path = "tui/fixture.rs"]
mod fixture;
#[path = "tui/host.rs"]
mod host;
#[path = "tui/mouse.rs"]
mod mouse;
#[path = "tui/proxy.rs"]
mod proxy;
#[path = "support/ssh.rs"]
mod remote;
mod support;

use fixture::{Fixture, Result, Tui};

#[test]
fn shell_exit_closes_the_tab_in_every_connected_tui() -> Result {
    let fixture = Fixture::new()?;
    let mut first = Tui::start(&fixture, &[])?;
    first.ready()?;
    let mut second = Tui::start(&fixture, &[])?;
    second.ready()?;
    first.write(b"exit\r")?;
    first.output("No tabs.")?;
    second.output("No tabs.")?;
    first.wait(|tui| Ok(tui.tabs()?.is_empty()))?;
    first.detach()?;
    second.detach()?;
    let mut restored = Tui::start(&fixture, &[])?;
    restored.output("No tabs.")?;
    assert!(fixture.client()?.list_sessions()?.is_empty());
    restored.detach()?;
    Ok(())
}

#[test]
fn external_exit_removes_hidden_tabs_and_preserves_the_live_split() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let client = fixture.client()?;
    let hidden = client.list_sessions()?[0].id;
    tui.write(b"\x02c")?;
    tui.wait(|tui| Ok(tui.tabs()?.len() == 2 && client.list_sessions()?.len() == 2))?;
    tui.ready()?;
    let active = tui.active_tab()?["focus"].clone();
    client.end_session(hidden)?;
    tui.wait(|tui| Ok(tui.tabs()?.len() == 1))?;
    assert_eq!(tui.active_tab()?["focus"], active);
    tui.write(b"\x02%")?;
    tui.wait(|tui| {
        Ok(tui.active_tab()?["panes"].as_object().is_some_and(|panes| {
            panes.len() == 2 && panes.values().all(|pane| !pane["session"].is_null())
        }) && client.list_sessions()?.len() == 2)
    })?;
    let split = tui.active_tab()?;
    let focused = split["focus"].as_str().ok_or("focused pane")?;
    let session = muxy_protocol::SessionId::new(
        split["panes"][focused]["session"]
            .as_u64()
            .ok_or("session")?,
    )
    .ok_or("session ID")?;
    client.end_session(session)?;
    // Keys follow the drawn layout, which can lag the saved state; wait for
    // one pane on screen, which has no frame.
    tui.wait(|tui| {
        let framed_panes: usize = tui.text()?.iter().map(|row| row.matches('┌').count()).sum();
        Ok(framed_panes == 0
            && tui.active_tab()?["panes"]
                .as_object()
                .is_some_and(|panes| panes.len() == 1))
    })?;
    assert_eq!(tui.active_tab()?["focus"], active);
    tui.write(b"printf '\nSURVIVING_SPLIT\n'\r")?;
    tui.output("SURVIVING_SPLIT")?;
    tui.detach()?;
    Ok(())
}

#[test]
fn keyboard_layout_restores_without_respawning_and_detach_preserves_sessions() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    tui.write(b"printf '\\nTUI_FIRST_READY\\n'\r")?;
    tui.output("TUI_FIRST_READY")?;
    tui.write(b"\x02c")?;
    tui.wait(|tui| Ok(tui.tabs()?.len() == 2))?;
    tui.ready()?;
    tui.write(b"\x02%")?;
    tui.wait(|tui| {
        Ok(tui.active_tab()?["panes"]
            .as_object()
            .is_some_and(|panes| panes.len() == 2))
    })?;
    tui.ready()?;
    let right = tui.active_tab()?["focus"].clone();
    tui.write(b"\x02\x1b[D")?;
    tui.wait(|tui| Ok(tui.active_tab()?["focus"] != right))?;
    tui.write(b"\x02\x1b[1;5C")?;
    tui.wait(|tui| {
        Ok(tui.active_tab()?["layout"]["Split"]["ratio"]
            .as_f64()
            .is_some_and(|ratio| ratio > 0.5))
    })?;
    tui.write(b"\x02z")?;
    tui.wait(|tui| Ok(tui.active_tab()?["zoom"] == true))?;
    tui.detach()?;
    let state = fixture.state()?;
    let client = fixture.client()?;
    let ids: Vec<_> = client
        .list_sessions()?
        .iter()
        .map(|session| session.id)
        .collect();
    assert_eq!(ids.len(), 3);
    let mut restored = Tui::start(&fixture, &[])?;
    restored.ready()?;
    assert_eq!(fixture.state()?, state);
    assert_eq!(
        client
            .list_sessions()?
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>(),
        ids
    );
    restored.detach()?;
    assert_eq!(client.list_sessions()?.len(), 3);
    Ok(())
}

#[test]
fn simultaneous_first_launch_creates_one_shell_and_concurrent_layout_edits_are_preserved() -> Result
{
    let fixture = Fixture::new()?;
    let mut first = Tui::start(&fixture, &[])?;
    let mut second = Tui::start(&fixture, &[])?;
    first.ready()?;
    second.ready()?;
    assert_eq!(fixture.client()?.list_sessions()?.len(), 1);
    first.write(b"\x02c")?;
    second.write(b"\x02%")?;
    first.wait(|tui| {
        second.pump()?;
        Ok(tui.tabs()?.len() == 2
            && tui
                .tabs()?
                .iter()
                .map(|tab| tab["panes"].as_object().map_or(0, serde_json::Map::len))
                .sum::<usize>()
                == 3)
    })?;
    first.wait(|tui| {
        second.pump()?;
        Ok(fixture.client()?.list_sessions()?.len() == 3
            && tui.tabs()?.iter().all(|tab| {
                tab["panes"].as_object().is_some_and(|panes| {
                    panes
                        .values()
                        .all(|pane| !pane["session"].is_null() && pane["creation"].is_null())
                })
            }))
    })?;
    let layout = fixture.state()?;
    let mut third = Tui::start(&fixture, &[])?;
    third.ready()?;
    assert_eq!(fixture.state()?, layout);
    assert_eq!(fixture.client()?.list_sessions()?.len(), 3);
    first.detach()?;
    second.detach()?;
    third.detach()?;
    Ok(())
}

#[test]
fn close_confirms_a_foreground_program_and_does_not_confirm_background_work() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let client = fixture.client()?;
    let session = client.list_sessions()?[0].id;
    tui.write(b"stty -echo; printf '\\nCLOSE_READY\\n'; cat\r")?;
    tui.output("CLOSE_READY")?;
    tui.wait(|_| {
        let attached = client.attach(session, muxy_protocol::Size { cols: 80, rows: 24 })?;
        let foreground = attached
            .process
            .is_some_and(|process| !process.is_shell && process.name == "cat");
        client.detach(attached.channel)?;
        Ok(foreground)
    })?;
    tui.write(b"\x02x")?;
    tui.output("Close terminal?")?;
    assert_eq!(client.list_sessions()?.len(), 1);
    tui.write(b"\x1b")?;
    tui.wait(|tui| {
        Ok(!tui
            .text()?
            .iter()
            .any(|row| row.contains("Close terminal?")))
    })?;
    tui.write(b"\x02x")?;
    tui.output("Close terminal?")?;
    tui.write(b"\r")?;
    tui.wait(|tui| Ok(tui.tabs()?.is_empty() && client.list_sessions()?.is_empty()))?;
    tui.write(b"\x02c")?;
    tui.ready()?;
    tui.write(b"sleep 30 &\nprintf '\\nBACKGROUND_READY\\n'\r")?;
    tui.output("BACKGROUND_READY")?;
    tui.write(b"\x02x")?;
    tui.wait(|tui| Ok(tui.tabs()?.is_empty() && client.list_sessions()?.is_empty()))?;
    tui.detach()?;
    let mut restored = Tui::start(&fixture, &[])?;
    restored.output("No tabs.")?;
    assert!(client.list_sessions()?.is_empty());
    restored.detach()?;
    Ok(())
}

#[test]
fn shell_identity_is_replaced_and_the_hosting_session_cannot_attach_to_itself() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(
        &fixture,
        &[("MUXY_SERVER_ID", "inherited"), ("MUXY_SESSION_ID", "999")],
    )?;
    tui.ready()?;
    let client = fixture.client()?;
    let catalog = client.catalog()?;
    let session = client.list_sessions()?[0].id;
    tui.write(b"printf '\\nSTAMP:%s:%s\\n' \"$MUXY_SERVER_ID\" \"$MUXY_SESSION_ID\"\r")?;
    tui.output(&format!("STAMP:{}:{}", catalog.server, session.get()))?;
    tui.detach()?;
    let server = catalog.server.to_string();
    let id = session.get().to_string();
    let mut nested = Tui::start(
        &fixture,
        &[("MUXY_SERVER_ID", &server), ("MUXY_SESSION_ID", &id)],
    )?;
    nested.output("Hosting terminal excluded")?;
    nested.write(b"\x02x")?;
    nested.output("Cannot close the terminal hosting this TUI")?;
    let tab = nested.locate(" 1 ")?;
    nested.menu((tab.0 + 1, tab.1), "Close tab")?;
    nested.output("Cannot close the tab hosting this TUI")?;
    assert!(nested.find("Close tab?")?.is_none());
    assert_eq!(client.list_sessions()?.len(), 1);
    nested.write(b"\x02c")?;
    nested.ready()?;
    nested.detach()?;
    assert_eq!(client.list_sessions()?.len(), 2);
    Ok(())
}

#[test]
fn projects_existing_terminals_and_other_client_changes_share_sessions_without_sharing_layouts()
-> Result {
    use muxy_protocol::{
        OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation, ProjectPatch,
        ServerPath, Size,
    };
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let client = fixture.client()?;
    let project = ProjectId::new();
    let directory = fixture.directory.path().join("home");
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(ProjectDescriptor {
            id: project,
            home: false,
            directory: ServerPath(directory.as_os_str().as_bytes().into()),
            name: "Shared project".into(),
            icon: None,
            logo: None,
            color: "#123456".into(),
            kind: None,
            parent_id: None,
        }),
    })?;
    let size = Size { cols: 80, rows: 24 };
    let shared = client.create_project_session(project, OperationId::new(), &directory, size)?;
    // The sidebar lists the project once the TUI knows it; only then can the finder match it.
    tui.output("Shared project")?;
    tui.write(b"\x02s")?;
    tui.output("type to filter")?;
    tui.write(b"shared\r")?;
    tui.wait(|tui| {
        Ok(fixture.state()?["active"] == project.to_string()
            && tui.active_tab()?["panes"]
                .as_object()
                .is_some_and(|panes| panes.values().all(|pane| !pane["session"].is_null())))
    })?;
    let index = client
        .project_sessions(project, None, None)?
        .sessions
        .iter()
        .position(|session| session.info.id == shared.id)
        .ok_or("shared session")?;
    // The tab bar counts the project's terminals that no tab shows; clicking it lists them.
    let (column, row) = tui.locate("▤ 1")?;
    tui.click(column, row)?;
    tui.output(&shared.id.get().to_string())?;
    for _ in 0..index {
        tui.write(b"\x1b[B")?;
    }
    tui.write(b"\r")?;
    tui.wait(|tui| Ok(tui.tabs()?.len() == 2))?;
    let attachment = client.attach(shared.id, size)?;
    client.send_input(attachment.channel, b"printf '\\nOTHER_CLIENT_OUTPUT\\n'\n")?;
    tui.output("OTHER_CLIENT_OUTPUT")?;
    assert_eq!(client.list_sessions()?.len(), 3);
    let layout = fixture.state()?;
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Patch {
            project,
            patch: ProjectPatch::Name("Renamed elsewhere".into()),
        },
    })?;
    tui.output("Renamed elsewhere")?;
    assert_eq!(fixture.state()?, layout);
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Delete(project),
    })?;
    tui.wait(|tui| {
        Ok(
            fixture.state()?["active"] == client.catalog()?.home.to_string()
                && !tui
                    .text()?
                    .iter()
                    .any(|line| line.contains("Renamed elsewhere"))
                && client.list_sessions()?.len() == 1,
        )
    })?;
    assert!(directory.is_dir());
    let remaining = client.list_sessions()?[0].id;
    client.end_session(remaining)?;
    tui.output("No tabs.")?;
    tui.wait(|tui| {
        Ok(tui.tabs()?.is_empty()
            && fixture.state()?["discards"]
                .as_array()
                .is_some_and(Vec::is_empty))
    })?;
    tui.detach()?;
    let state = fixture.state()?;
    let mut restored = Tui::start(&fixture, &[])?;
    restored.output("No tabs.")?;
    assert_eq!(fixture.state()?, state);
    assert!(client.list_sessions()?.is_empty());
    restored.detach()?;
    Ok(())
}

#[test]
fn large_bracketed_paste_preserves_bytes_and_does_not_parse_prefix_shortcuts() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let text = format!(
        "{}界́\x02d{}",
        "a".repeat(muxy_protocol::MAX_INPUT - 1),
        "b".repeat(64)
    );
    let received = fixture.directory.path().join("received.bin");
    let script = fixture.directory.path().join("paste.py");
    std::fs::write(
        &script,
        format!(
            "import os,sys,termios,tty\nold=termios.tcgetattr(0)\ntty.setraw(0)\nos.write(1,b'\\r\\nPASTE_READY\\r\\n')\ntry:\n data=sys.stdin.buffer.read({})\nfinally:\n termios.tcsetattr(0,termios.TCSANOW,old)\nopen({:?},'wb').write(data)\nos.write(1,b'\\r\\nPASTE_COMPLETE\\r\\n')\n",
            text.len(),
            received
        ),
    )?;
    tui.write(format!("python3 '{}'\r", script.display()).as_bytes())?;
    tui.output("PASTE_READY")?;
    let before = fixture.state()?;
    let bytes = [
        b"\x1b[200~".as_slice(),
        text.as_bytes(),
        b"\x1b[201~".as_slice(),
    ]
    .concat();
    tui.write(&bytes)?;
    tui.output("PASTE_COMPLETE")?;
    assert_eq!(std::fs::read(received)?, text.as_bytes());
    assert_eq!(fixture.state()?, before);
    tui.detach()?;
    Ok(())
}

#[test]
fn handled_signals_and_suspend_restore_the_original_terminal_modes() -> Result {
    for signal in ["TERM", "INT", "HUP"] {
        let fixture = Fixture::new()?;
        let mut tui = Tui::start(&fixture, &[])?;
        tui.ready()?;
        let client = fixture.client()?;
        let sessions = client.list_sessions()?;
        tui.signal(signal)?;
        assert_eq!(tui.exit()?.code, Some(0), "{signal}");
        tui.assert_restored()?;
        assert_eq!(client.list_sessions()?, sessions);
    }
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let state = fixture.state()?;
    tui.signal("TSTP")?;
    tui.wait(|tui| tui.stopped())?;
    tui.assert_restored()?;
    tui.signal("CONT")?;
    tui.ready()?;
    tui.write(b"printf '\\nRESUMED_READY\\n'\r")?;
    tui.output("RESUMED_READY")?;
    assert_eq!(fixture.state()?, state);
    tui.detach()?;
    Ok(())
}
