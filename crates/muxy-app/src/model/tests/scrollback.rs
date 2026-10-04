use super::*;
use crate::views::terminal::pane::TerminalPane;
use gpui::{ScrollDelta, ScrollWheelEvent, point};

#[gpui::test]
#[ignore = "requires a fresh MUXY_DIR under /tmp/muxy-phase15-, server PID wrapper, and built attach example"]
fn phase15_scrollback_walkthrough(cx: &mut TestAppContext) {
    let result = run(cx);
    assert!(result.is_ok(), "{result:?}");
}

fn pane<'a>(model: &'a AppModel, cx: &'a gpui::App) -> &'a TerminalPane {
    model
        .terminal(&model.active_pane().expect("pane"))
        .expect("terminal")
        .view
        .read(cx)
}

fn visible(model: &AppModel, cx: &gpui::App) -> Vec<String> {
    let pane = pane(model, cx);
    let grid = pane
        .scroll
        .view
        .as_ref()
        .or(pane.grid.as_ref())
        .expect("grid");
    let height = usize::from(pane.viewport().unwrap_or(grid.size).rows);
    let start = pane.scroll.start(grid, height);
    (start..start + height)
        .filter_map(|index| grid.content_row(index))
        .map(|runs| {
            runs.iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn wheel(cx: &mut VisualTestContext, rows: f32) -> Result {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("terminal-pane")
        .ok_or("terminal bounds missing")?;
    cx.simulate_event(gpui::MouseMoveEvent {
        position: bounds.center(),
        ..gpui::MouseMoveEvent::default()
    });
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        position: bounds.center(),
        delta: ScrollDelta::Lines(point(0.0, rows)),
        ..ScrollWheelEvent::default()
    });
    Ok(())
}

fn wait_offset(cx: &mut VisualTestContext, view: &Entity<AppModel>, offset: usize) -> Result {
    let result = wait(cx, view, |model, cx| {
        pane(model, cx).scroll.offset >= f64::from(u16::try_from(offset).unwrap_or(u16::MAX))
    });
    if result.is_err() {
        report(&view.read_with(cx, |model, cx| {
            format!("offset wait failed: {:?}", pane(model, cx).scroll)
        }))?;
    }
    result
}

fn wait_line(cx: &mut VisualTestContext, view: &Entity<AppModel>, line: &str) -> Result {
    wait(cx, view, |model, cx| {
        active_grid(model, cx).is_some_and(|grid| {
            (0..grid.rows.len()).any(|index| grid.row_text(index).trim() == line)
        })
    })
}

fn numbered(rows: &[String]) -> Vec<usize> {
    rows.iter()
        .filter_map(|row| row.trim().parse().ok())
        .collect()
}

#[allow(clippy::cast_possible_truncation, clippy::float_cmp)]
fn run(cx: &mut TestAppContext) -> Result {
    let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
    assert!(
        directory
            .to_string_lossy()
            .starts_with("/tmp/muxy-phase15-")
    );
    assert!(!directory.join("state.json").exists());
    let boot = Boot::load()?;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_tab);
    wait_live(cx, &view)?;
    shell(cx, "PS1='phase15> '; clear");
    wait_text(cx, &view, "phase15>")?;
    shell(cx, "seq 1 5000");
    wait_line(cx, &view, "5000")?;
    wheel(cx, 50.0)?;
    wait_offset(cx, &view, 50)?;
    for offset in [250, 750, 1250, 1750, 2250, 2750, 3250, 3750, 4250, 4750] {
        let current = view.read_with(cx, |model, cx| pane(model, cx).scroll.offset);
        wheel(cx, f32::from(u16::try_from(offset)?) - current as f32)?;
        wait_offset(cx, &view, offset)?;
        let rows = view.read_with(cx, visible);
        let numbers = numbered(&rows);
        assert!(numbers.windows(2).all(|pair| pair[1] == pair[0] + 1));
        report(&format!(
            "15.1: offset {offset}: {}",
            numbers
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ))?;
    }
    wheel(cx, 10000.0)?;
    wait(cx, &view, |model, cx| {
        pane(model, cx)
            .scroll
            .view
            .as_ref()
            .is_some_and(|grid| grid.history_cursor.is_none())
    })?;
    view.read_with(cx, |model, cx| {
        let grid = pane(model, cx).scroll.view.as_ref().expect("scroll view");
        let rows = (0..grid.history.len() + grid.rows.len())
            .map(|index| {
                grid.content_row(index)
                    .expect("row")
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect()
            })
            .collect::<Vec<String>>();
        assert_eq!(numbered(&rows), (1..=5000).collect::<Vec<_>>());
    });
    report(&format!(
        "15.1: oldest visible: {:?}; all 5,000 numbered rows retained in exact order",
        view.read_with(cx, visible)
    ))?;
    concurrent_output(cx, &view, &directory)?;
    type_text(cx, "x");
    wait_text(cx, &view, "phase15> x")?;
    assert_eq!(
        view.read_with(cx, |model, cx| pane(model, cx).scroll.offset),
        0.0
    );
    report("15.3: key x -> offset 0, live prompt contains phase15> x")?;
    cx.simulate_keystrokes("ctrl-u");
    wait_line(cx, &view, "phase15>")?;
    wheel(cx, 100.0)?;
    wait_offset(cx, &view, 100)?;
    let prompt = screen(&view, cx);
    cx.dispatch_action(crate::views::workspace::ScrollToBottom);
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, cx| pane(model, cx).scroll.view.is_none()));
    assert_eq!(screen(&view, cx), prompt);
    report("15.3: ScrollToBottom -> live view; no terminal input")?;
    exit_closes_scrollback(cx, &view, &directory)?;
    report("Phase 15 GPUI/live-server walkthrough: PASS")
}

fn concurrent_output(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    directory: &std::path::Path,
) -> Result {
    let before = view.read_with(cx, visible);
    let size = view.read_with(cx, |model, cx| active_grid(model, cx).expect("grid").size);
    let session = active_session(view, cx)?;
    let example =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/examples/attach");
    let log = std::fs::File::create(directory.join("second-client.log"))?;
    let mut child = std::process::Command::new(example)
        .arg("attach")
        .arg(directory.join("server.sock"))
        .arg(session.get().to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(log)
        .spawn()?;
    let result = (|| -> Result {
        child
            .stdin
            .as_mut()
            .ok_or("missing stdin")?
            .write_all(b"seq 1 20; printf 'SECOND_DONE\\n'\n")?;
        wait_line(cx, view, "SECOND_DONE")?;
        assert_eq!(view.read_with(cx, visible), before);
        assert_eq!(
            view.read_with(cx, |model, cx| active_grid(model, cx).expect("grid").size),
            size
        );
        report(&format!(
            "15.2: phase 9 attach example ran seq 1 20 at {}x{}; before == after: {:?}",
            size.cols, size.rows, before
        ))
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn exit_closes_scrollback(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    directory: &std::path::Path,
) -> Result {
    shell(cx, "exit");
    wait_empty(cx, view)?;
    signal_test_server(directory, "-TERM")?;
    wait(cx, view, |model, _| {
        model.servers.local.connection == ConnectionState::Disconnected
    })?;
    reload_model(cx, view)?;
    wait_empty(cx, view)?;
    report(
        "15.4: exit closed the terminal and server restart did not restore its scrollback pane",
    )?;
    signal_test_server(directory, "-TERM")
}
