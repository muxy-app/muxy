#[path = "../src/views/terminal/input.rs"]
mod input;

use gpui::{Keystroke, Modifiers};
use muxy_protocol::Modes;

fn key(name: &str, text: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke {
        key: name.into(),
        key_char: text.map(str::to_owned),
        modifiers,
    }
}

#[test]
fn named_keys_encode_in_normal_and_application_modes() {
    for application in [false, true] {
        let modes = Modes {
            application_cursor_keys: application,
            bracketed_paste: false,
        };
        for (name, expected) in [
            ("enter", "\r"),
            ("return", "\r"),
            ("backspace", "\x7f"),
            ("tab", "\t"),
            ("escape", "\x1b"),
            ("pageup", "\x1b[5~"),
            ("pagedown", "\x1b[6~"),
            ("insert", "\x1b[2~"),
            ("delete", "\x1b[3~"),
            ("f1", "\x1bOP"),
            ("f2", "\x1bOQ"),
            ("f3", "\x1bOR"),
            ("f4", "\x1bOS"),
            ("f5", "\x1b[15~"),
            ("f6", "\x1b[17~"),
            ("f7", "\x1b[18~"),
            ("f8", "\x1b[19~"),
            ("f9", "\x1b[20~"),
            ("f10", "\x1b[21~"),
            ("f11", "\x1b[23~"),
            ("f12", "\x1b[24~"),
            ("space", " "),
        ] {
            assert_eq!(
                input::encode(&key(name, None, Modifiers::default()), modes, true).as_deref(),
                Some(expected.as_bytes()),
                "{name}, application={application}"
            );
        }
        for (name, suffix) in [
            ("up", 'A'),
            ("down", 'B'),
            ("right", 'C'),
            ("left", 'D'),
            ("home", 'H'),
            ("end", 'F'),
        ] {
            let expected = format!("\x1b{}{suffix}", if application { 'O' } else { '[' });
            assert_eq!(
                input::encode(&key(name, None, Modifiers::default()), modes, true).as_deref(),
                Some(expected.as_bytes())
            );
        }
    }
}

#[test]
fn control_letters_and_symbols_produce_control_bytes() {
    let modifiers = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    for letter in b'a'..=b'z' {
        for name in [
            char::from(letter).to_string(),
            char::from(letter).to_uppercase().to_string(),
        ] {
            assert_eq!(
                input::encode(&key(&name, None, modifiers), Modes::default(), true),
                Some(vec![letter - b'a' + 1])
            );
        }
    }
    for (name, expected) in [
        ("space", 0),
        ("@", 0),
        ("2", 0),
        ("[", 27),
        ("3", 27),
        ("\\", 28),
        ("4", 28),
        ("]", 29),
        ("5", 29),
        ("^", 30),
        ("6", 30),
        ("_", 31),
        ("-", 31),
        ("7", 31),
        ("/", 31),
        ("?", 127),
        ("8", 127),
    ] {
        assert_eq!(
            input::encode(&key(name, None, modifiers), Modes::default(), true),
            Some(vec![expected]),
            "{name}"
        );
    }
    assert_eq!(
        input::encode(&key("9", None, modifiers), Modes::default(), true),
        None
    );
}

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
