//! The mouse and scrollback, driven through SGR mouse reports as a host
//! terminal sends them.

use std::fs;

use super::fixture::{Fixture, Result, Tui};

/// Where a lone pane's terminal starts: right of the 26-column sidebar, below
/// the tab bar.
const PANE: (u16, u16) = (26, 1);

#[test]
fn the_wheel_and_scroll_mode_page_through_history_and_typing_returns_live() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    tui.write(b"i=1; while [ $i -le 300 ]; do echo row-$i; i=$((i+1)); done\r")?;
    tui.output("row-300")?;
    for _ in 0..4 {
        tui.wheel(50, 10, true)?;
    }
    tui.output("↑ 12/")?;
    tui.wait(|tui| Ok(tui.find("row-300")?.is_none() && tui.find("row-288")?.is_some()))?;

    tui.write(b"printf '\\nLIVE_%s\\n' AGAIN\r")?;
    tui.output("LIVE_AGAIN")?;
    tui.wait(|tui| Ok(tui.find("↑ ")?.is_none()))?;

    tui.write(b"\x02\x1b[5~")?;
    tui.output("SCROLL")?;
    tui.output("↑ 24/")?;
    tui.write(b"q")?;
    tui.wait(|tui| Ok(tui.find("SCROLL")?.is_none() && tui.find("↑ ")?.is_none()))?;

    tui.write(b"\x02[")?;
    tui.output("SCROLL")?;
    tui.write(b"g")?;
    tui.wait(|tui| Ok(tui.find("row-1 ")?.is_some() && tui.find("row-300")?.is_none()))?;
    tui.write(b"G")?;
    tui.wait(|tui| Ok(tui.find("SCROLL")?.is_none() && tui.find("LIVE_AGAIN")?.is_some()))?;
    tui.detach()?;
    Ok(())
}

#[test]
fn apps_that_use_the_mouse_get_clicks_and_the_wheel_in_their_own_cells() -> Result {
    let fixture = Fixture::new()?;
    let mut tui = Tui::start(&fixture, &[])?;
    tui.ready()?;
    tui.write(b"i=1; while [ $i -le 60 ]; do echo old-$i; i=$((i+1)); done\r")?;
    tui.output("old-60")?;
    let received = fixture.directory.path().join("mouse.bin");
    let script = fixture.directory.path().join("mouse.py");
    fs::write(
        &script,
        format!(
            "import os,termios,tty\nold=termios.tcgetattr(0)\ntty.setraw(0)\nos.write(1,b'\\x1b[?1000h\\x1b[?1006h\\r\\nMOUSE_READY\\r\\n')\ndata=b''\ntry:\n while not data.endswith(b'q'):\n  data+=os.read(0,4096)\nfinally:\n termios.tcsetattr(0,termios.TCSANOW,old)\nopen({received:?},'wb').write(data)\nos.write(1,b'\\x1b[?1000l\\x1b[?1006l\\r\\nMOUSE_DONE\\r\\n')\n"
        ),
    )?;
    tui.write(format!("python3 '{}'\r", script.display()).as_bytes())?;
    tui.output("MOUSE_READY")?;
    // A help round trip lets the input-mode metadata arrive without sending child input.
    tui.write(b"\x02?")?;
    tui.output("Keyboard help")?;
    tui.write(b"\r")?;
    tui.wait(|tui| Ok(tui.find("Keyboard help")?.is_none()))?;
    tui.click(PANE.0 + 4, PANE.1 + 6)?;
    tui.wheel(PANE.0 + 9, PANE.1 + 2, true)?;
    // A right-click opens the menu; the app gets it only when asked to.
    tui.menu((PANE.0 + 4, PANE.1 + 6), "Right-click the app")?;
    tui.wait(|tui| Ok(tui.find("Right-click the app")?.is_none()))?;
    // History shown over the app is the UI's: clicking it selects, and q
    // leaves scroll mode instead of reaching the app.
    tui.write(b"\x02\x1b[5~")?;
    tui.output("↑ 24/")?;
    tui.click(PANE.0 + 4, PANE.1 + 6)?;
    tui.write(b"q")?;
    tui.wait(|tui| Ok(tui.find("↑ ")?.is_none()))?;
    tui.write(b"q")?;
    tui.output("MOUSE_DONE")?;
    assert_eq!(
        fs::read(received)?,
        b"\x1b[<0;5;7M\x1b[<0;5;7m\x1b[<64;10;3M\x1b[<2;5;7M\x1b[<2;5;7mq"
    );
    tui.detach()?;
    Ok(())
}

#[test]
fn dragging_or_double_clicking_text_copies_it_to_the_clipboard() -> Result {
    let fixture = Fixture::new()?;
    // Over SSH the terminal is asked to copy, rather than this computer's
    // clipboard, which a test must not touch.
    let mut tui = Tui::start(
        &fixture,
        &[("SSH_CONNECTION", "10.0.0.2 50000 10.0.0.1 22")],
    )?;
    tui.ready()?;
    tui.write(b"printf '\\nCOPY_%s %s\\n' ME_PLEASE SECOND_WORD\r")?;
    let (column, row) = tui.locate("COPY_ME_PLEASE")?;
    tui.drag((column, row), (column + 13, row))?;
    tui.wait(|tui| Ok(copied(tui, "Q09QWV9NRV9QTEVBU0U=")))?;
    tui.output("Sent 14 characters to the terminal's clipboard")?;
    let (column, row) = tui.locate("SECOND_WORD")?;
    tui.click(column + 3, row)?;
    tui.click(column + 3, row)?;
    tui.wait(|tui| Ok(copied(tui, "U0VDT05EX1dPUkQ=")))?;
    tui.detach()?;
    Ok(())
}

fn copied(tui: &Tui<'_>, encoded: &str) -> bool {
    let sequence = format!("\x1b]52;c;{encoded}\x07");
    tui.raw
        .windows(sequence.len())
        .any(|bytes| bytes == sequence.as_bytes())
}
