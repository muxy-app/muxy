#[path = "../src/views/terminal/input.rs"]
mod input;

use gpui::Keystroke;
use muxy_protocol::Modes;

#[cfg(target_os = "macos")]
#[test]
fn option_word_motion_edits_the_line_in_clean_zsh_and_bash()
-> Result<(), Box<dyn std::error::Error>> {
    use muxy_terminal::pty::{Pty, PtyEvent, PtySize, SpawnRequest};
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

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
            pty.write(b"alpha beta")?;
            pty.write(
                &input::encode(&Keystroke::parse("alt-left")?, Modes::default(), true)
                    .ok_or("left encoding")?,
            )?;
            pty.write(b"X")?;
            pty.write(
                &input::encode(&Keystroke::parse("alt-right")?, Modes::default(), true)
                    .ok_or("right encoding")?,
            )?;
            pty.write(b"Y\r")?;
            while let PtyEvent::Output(bytes) =
                receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))?
            {
                output.extend(bytes);
            }
            if !String::from_utf8_lossy(&output).contains("RESULT:<alpha XbetaY>") {
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
