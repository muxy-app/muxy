use std::fmt::Write;
use std::fs;

use super::fixture::{Fixture, Result, Tui};

#[test]
fn closing_the_host_terminal_exits_the_tui_and_releases_its_lock() -> Result {
    let fixture = Fixture::new()?;
    let result = std::process::Command::new("python3")
        .args(["-c", include_str!("hangup.py")])
        .arg(super::support::binary())
        .envs(fixture.environment())
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.directory.path().join("tui.lock"))?;
    lock.try_lock()?;
    assert_eq!(fixture.client()?.list_sessions()?.len(), 1);
    Ok(())
}

#[test]
fn vim_and_less_work_inside_the_real_terminal() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let edited = fixture.directory.path().join("edited.txt");
    tui.write(format!("vim -Nu NONE -n '{}'\r", edited.display()).as_bytes())?;
    tui.wait(|tui| {
        Ok(tui
            .text()?
            .iter()
            .any(|row| row.contains("edited.txt") && row.contains("[New")))
    })?;
    tui.write("iVIM_界_EDITED".as_bytes())?;
    tui.output("VIM_界_EDITED")?;
    tui.write(b"\x1b:wq\r")?;
    tui.ready()?;
    assert_eq!(fs::read_to_string(edited)?, "VIM_界_EDITED\n");
    let paged = fixture.directory.path().join("paged.txt");
    let mut lines = String::new();
    for index in 0..200 {
        writeln!(lines, "less-line-{index:03}")?;
    }
    fs::write(&paged, lines)?;
    tui.write(format!("LESS= LESSOPEN= less -R '{}'\r", paged.display()).as_bytes())?;
    tui.output("less-line-000")?;
    tui.write(b"G")?;
    tui.output("less-line-199")?;
    tui.write(b"g")?;
    tui.output("less-line-000")?;
    tui.write(b"q")?;
    tui.ready()?;
    tui.detach()?;
    Ok(())
}

#[test]
fn input_modes_encode_child_keys_paste_and_requested_focus_reports() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    let received = fixture.directory.path().join("modes.bin");
    let script = fixture.directory.path().join("modes.py");
    fs::write(
        &script,
        format!(
            "import os,termios,tty\nold=termios.tcgetattr(0)\ntty.setraw(0)\nos.write(1,b'\\x1b[?1h\\x1b[?2004h\\x1b[?1004h\\r\\nMODES_READY\\r\\n')\ndata=b''\ntry:\n while not data.endswith(b'\\x00'):\n  data+=os.read(0,4096)\nfinally:\n termios.tcsetattr(0,termios.TCSANOW,old)\nopen({received:?},'wb').write(data)\nos.write(1,b'\\x1b[?1l\\x1b[?2004l\\x1b[?1004l\\r\\nMODES_DONE\\r\\n')\n"
        ),
    )?;
    tui.write(format!("python3 '{}'\r", script.display()).as_bytes())?;
    tui.output("MODES_READY")?;
    // A help round trip lets the input-mode metadata arrive without sending child input.
    tui.write(b"\x02?")?;
    tui.output("Keyboard help")?;
    tui.write(b"\r")?;
    tui.wait(|tui| {
        Ok(!tui
            .text()?
            .iter()
            .any(|line| line.contains("Keyboard help")))
    })?;
    tui.write(
        "\x1b[A\x1bé\x1bOP\x03\x04\x02\x02\x1b[200~hello\x02d\x1b[201~\x1b[O\x1b[I\x00".as_bytes(),
    )?;
    tui.output("MODES_DONE")?;
    let bytes = fs::read(received)?;
    let expected =
        "\x1bOA\x1bé\x1bOP\x03\x04\x02\x1b[200~hello\x02d\x1b[201~\x1b[O\x1b[I\x00".as_bytes();
    assert!(bytes.ends_with(expected), "{bytes:?}");
    assert!(
        bytes[..bytes.len() - expected.len()].is_empty()
            || bytes[..bytes.len() - expected.len()] == *b"\x1b[I"
    );
    tui.detach()?;
    Ok(())
}

#[test]
fn server_restart_closes_dead_tabs_without_creating_another_shell() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    tui.write(b"printf '\\nBEFORE_SERVER_RESTART\\n'\r")?;
    tui.output("BEFORE_SERVER_RESTART")?;
    let before = fixture.client()?;
    let instance = before.server_info().instance;
    before.stop_server()?;
    tui.output("No tabs.")?;
    tui.wait(|_| {
        // A draining listener can still accept a connection without answering.
        // Keep each probe within the PTY wait budget and require a new server.
        let timeout = std::time::Duration::from_millis(200);
        Ok(muxy_client::Client::connect_with_timeout(
            &fixture.directory.path().join("server.sock"),
            timeout,
        )
        .is_ok_and(|client| {
            client.server_info().instance != instance
                && client
                    .with_timeout(timeout)
                    .list_sessions()
                    .is_ok_and(|sessions| sessions.is_empty())
        }))
    })?;
    tui.wait(|tui| {
        let closes_pending = fixture.state()?["close_operations"]
            .as_object()
            .is_some_and(|operations| !operations.is_empty());
        Ok(tui.tabs()?.is_empty() && !closes_pending)
    })?;
    tui.detach()?;
    let saved = fixture.state()?;
    let mut restored = Tui::start(&fixture, &[])?;
    restored.output("No tabs.")?;
    assert_eq!(fixture.state()?, saved);
    assert!(fixture.client()?.list_sessions()?.is_empty());
    restored.detach()?;
    Ok(())
}
