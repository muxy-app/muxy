//! Copies text to the clipboard: the system's, when this UI runs on the
//! computer in front of the user, otherwise the terminal's (OSC 52), which
//! reaches that computer through SSH.

use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Copied {
    /// The system clipboard has the text.
    System,
    /// The terminal was asked to copy it, which it may decline.
    Terminal,
}

pub(crate) fn copy(text: &str) -> io::Result<Copied> {
    if !crate::terminal::over_ssh() && system(text) {
        return Ok(Copied::System);
    }
    let mut stdout = io::stdout();
    stdout.write_all(sequence(text).as_bytes())?;
    stdout.flush()?;
    Ok(Copied::Terminal)
}

/// Hands `text` to the platform's clipboard tool, if it has one.
fn system(text: &str) -> bool {
    let tools: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        &[("wl-copy", &[])]
    } else if std::env::var_os("DISPLAY").is_some() {
        &[
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    } else {
        &[]
    };
    tools.iter().any(|(program, arguments)| {
        let Ok(mut child) = Command::new(program)
            .args(*arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let written = child
            .stdin
            .take()
            .is_some_and(|mut input| input.write_all(text.as_bytes()).is_ok());
        child.wait().is_ok_and(|status| status.success()) && written
    })
}

fn sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | u32::from(*byte) << (16 - 8 * index)
            });
        for index in 0..4 {
            if index <= chunk.len() {
                encoded.push(char::from(
                    ALPHABET[(value >> (18 - 6 * index)) as usize & 63],
                ));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}
