//! Tells the terminal what a script is doing with the program status
//! protocol (OSC 7501). It needs no server, so it works over SSH and in any
//! terminal that supports the protocol; others ignore it.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, Write};

use crate::manage::help::STATUS;

const STATES: [&str; 6] = ["idle", "working", "done", "blocked", "error", "clear"];
const KINDS: [&str; 3] = ["permission", "question", "auth"];
const MAX_TITLE: usize = 192;
const MAX_MESSAGE: usize = 2048;

/// Reads `<state> [options]` into the report to send.
pub(crate) fn parse(arguments: &[OsString]) -> io::Result<String> {
    let text = arguments
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .ok_or_else(|| invalid("arguments must be UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let Some((state, options)) = text.split_first() else {
        return Err(invalid(STATUS));
    };
    if !STATES.contains(state) {
        return Err(invalid(&format!(
            "unknown state {state}; use {}",
            STATES.join(", ")
        )));
    }
    let mut keys = Keys::default();
    for option in options.chunks(2) {
        let [flag, value] = option else {
            return Err(invalid(STATUS));
        };
        let slot = match *flag {
            "--id" => &mut keys.id,
            "--app" => &mut keys.app,
            "--progress" => &mut keys.progress,
            "--kind" => &mut keys.kind,
            "--title" => &mut keys.title,
            "--msg" => &mut keys.msg,
            _ => return Err(invalid(STATUS)),
        };
        if slot.replace(*value).is_some() {
            return Err(invalid(&format!("duplicate option {flag}")));
        }
    }
    keys.report(state)
}

/// Writes the report to this process's terminal, even when its output is
/// redirected, or to its output when it has no terminal.
pub(crate) fn run(report: &str) -> io::Result<()> {
    if let Ok(mut terminal) = OpenOptions::new().write(true).open("/dev/tty") {
        return terminal.write_all(report.as_bytes());
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(report.as_bytes())?;
    stdout.flush()
}

#[derive(Default)]
struct Keys<'a> {
    id: Option<&'a str>,
    app: Option<&'a str>,
    progress: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    msg: Option<&'a str>,
}

impl Keys<'_> {
    fn report(&self, state: &str) -> io::Result<String> {
        let mut pairs = vec![format!("state={state}")];
        if let Some(id) = self.id {
            let segments: Vec<_> = id.split('/').collect();
            if id.len() > 128 || segments.len() > 8 || !segments.iter().all(|s| name(s)) {
                return Err(invalid(
                    "--id is up to 8 parts joined by /, each up to 32 of A-Z a-z 0-9 _ . + -",
                ));
            }
            pairs.push(format!("id={id}"));
        }
        if state == "clear" {
            if self
                .app
                .or(self.progress)
                .or(self.kind)
                .or(self.title)
                .or(self.msg)
                .is_some()
            {
                return Err(invalid("clear takes only --id"));
            }
            return Ok(sequence(&pairs));
        }
        if let Some(app) = self.app {
            if !name(app) {
                return Err(invalid("--app is up to 32 of A-Z a-z 0-9 _ . + -"));
            }
            pairs.push(format!("app={app}"));
        }
        if let Some(progress) = self.progress {
            if !matches!(state, "working" | "blocked") {
                return Err(invalid("--progress is only for working or blocked"));
            }
            let percent = progress
                .parse::<u8>()
                .ok()
                .filter(|percent| *percent <= 100)
                .ok_or_else(|| invalid("--progress must be a number from 0 to 100"))?;
            pairs.push(format!("progress={percent}"));
        }
        if let Some(kind) = self.kind {
            if state != "blocked" {
                return Err(invalid("--kind is only for blocked"));
            }
            if !KINDS.contains(&kind) {
                return Err(invalid("--kind must be permission, question or auth"));
            }
            pairs.push(format!("kind={kind}"));
        }
        for (key, text, limit) in [
            ("title", self.title, MAX_TITLE),
            ("msg", self.msg, MAX_MESSAGE),
        ] {
            let Some(text) = text else { continue };
            if text.len() > limit || text.chars().any(char::is_control) {
                return Err(invalid(&format!(
                    "--{key} must be one line of at most {limit} bytes"
                )));
            }
            pairs.push(format!(
                "{key}={}",
                crate::clipboard::base64(text.as_bytes())
            ));
        }
        Ok(sequence(&pairs))
    }
}

fn sequence(pairs: &[String]) -> String {
    format!("\x1b]7501;{}\x1b\\", pairs.join(":"))
}

fn name(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(words: &[&str]) -> io::Result<String> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn reports_encode_text_and_keep_a_fixed_key_order() -> io::Result<()> {
        assert_eq!(report(&["idle"])?, "\x1b]7501;state=idle\x1b\\");
        assert_eq!(
            report(&[
                "blocked",
                "--msg",
                "Apply?",
                "--kind",
                "permission",
                "--app",
                "terraform",
                "--progress",
                "40",
                "--id",
                "deploy/us-east",
                "--title",
                "US",
            ])?,
            "\x1b]7501;state=blocked:id=deploy/us-east:app=terraform:progress=40\
             :kind=permission:title=VVM=:msg=QXBwbHk/\x1b\\"
        );
        assert_eq!(
            report(&["clear", "--id", "a"])?,
            "\x1b]7501;state=clear:id=a\x1b\\"
        );
        Ok(())
    }

    #[test]
    fn values_the_protocol_rejects_are_errors() {
        for words in [
            vec![],
            vec!["sleeping"],
            vec!["working", "--app"],
            vec!["working", "--app", "a:b"],
            vec!["working", "--app", &"a".repeat(33)],
            vec!["working", "--id", "a//b"],
            vec!["working", "--id", "a/b/c/d/e/f/g/h/i"],
            vec!["working", "--progress", "101"],
            vec!["done", "--progress", "50"],
            vec!["working", "--kind", "auth"],
            vec!["blocked", "--kind", "money"],
            vec!["working", "--msg", "two\nlines"],
            vec!["working", "--title", &"t".repeat(193)],
            vec!["working", "--app", "a", "--app", "b"],
            vec!["working", "--color", "red"],
            vec!["clear", "--app", "build"],
        ] {
            assert!(report(&words).is_err(), "{words:?}");
        }
    }
}
