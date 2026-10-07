#![cfg(target_os = "macos")]

use muxy_app_core::settings::{TerminalAction, TerminalBindings};
use muxy_terminal::pty::{Pty, PtyEvent, PtySize, SpawnRequest};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

#[test]
fn command_line_editing_bindings_work_in_clean_zsh_and_bash()
-> Result<(), Box<dyn std::error::Error>> {
    let bindings = TerminalBindings::default();
    let binding = |chord: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match bindings.action(&chord.parse()?) {
            Some(TerminalAction::Text(bytes)) => Ok(bytes.clone()),
            _ => Err(format!("missing editing binding: {chord}").into()),
        }
    };
    for (shell, args) in [
        (
            "/bin/zsh",
            vec![
                "-f",
                "-c",
                "bindkey -e; value=; vared -p 'INPUT_READY>' value; printf '\\nRESULT:<%s>\\n' \"$value\"",
            ],
        ),
        (
            "/bin/bash",
            vec![
                "--noprofile",
                "--norc",
                "-c",
                "IFS= read -e -r -p 'INPUT_READY>' value; printf '\\nRESULT:<%s>\\n' \"$value\"",
            ],
        ),
    ] {
        let mut pty = Pty::spawn(SpawnRequest {
            clear_env: false,
            program: shell.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: std::env::temp_dir(),
            env: vec![
                ("TERM".into(), "xterm-256color".into()),
                ("INPUTRC".into(), "/dev/null".into()),
            ],
            size: PtySize { cols: 80, rows: 24 },
        })?;
        let (sender, receiver) = channel();
        let reader = pty.start_reader(sender)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut output = Vec::new();
            while !String::from_utf8_lossy(&output).contains("INPUT_READY>") {
                match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                    PtyEvent::Output(bytes) => output.extend(bytes),
                    PtyEvent::Closed => return Err(format!("{shell} closed before input").into()),
                }
            }
            pty.write(b"alpha beta tail")?;
            pty.write(&binding("cmd-left")?)?;
            pty.write(&b"\x1b[C".repeat(6))?;
            pty.write(&binding("cmd-backspace")?)?;
            pty.write(b"X")?;
            pty.write(&binding("cmd-right")?)?;
            pty.write(b"Y")?;
            pty.write(&binding("cmd-left")?)?;
            pty.write(b"Z\r")?;
            while let PtyEvent::Output(bytes) =
                receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))?
            {
                output.extend(bytes);
            }
            // ZLE's Ctrl+U kills the whole line; Readline's kills back to the cursor.
            let expected = if shell == "/bin/zsh" {
                "RESULT:<ZXY>"
            } else {
                "RESULT:<ZXbeta tailY>"
            };
            if !String::from_utf8_lossy(&output).contains(expected) {
                return Err(format!("{shell}: {}", String::from_utf8_lossy(&output)).into());
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = pty.kill();
        }
        let status = pty.wait()?;
        reader.join().map_err(|_| "PTY reader panicked")?;
        result?;
        assert_eq!(status.code, Some(0), "{shell}");
    }
    Ok(())
}
