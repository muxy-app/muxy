//! `muxy config`: reads and changes the desktop app's settings in this
//! computer's profile, then has a running app apply them.

use std::error::Error;
use std::ffi::OsString;
use std::io::{self, Write};
use std::time::Duration;

use muxy_app_core::browser::api::CallError;
use muxy_app_core::settings::keys::{self, Profile};
use serde_json::{Map, Value, json};

use crate::args::Command;
use crate::manage::help;

const RELOAD: &str = "settings.reload";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Action {
    List,
    Get(String),
    Set(String, String),
    Unset(String),
    Reload,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    action: Action,
    json: bool,
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    if arguments.is_empty()
        || arguments.first().is_some_and(|word| word == "help")
        || arguments
            .iter()
            .any(|word| word == "--help" || word == "-h")
    {
        return Ok(Command::Usage(help::CONFIG));
    }
    let mut words: Vec<&str> = arguments
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| invalid("arguments must be UTF-8"))
        })
        .collect::<io::Result<_>>()?;
    let json = words.last() == Some(&"--json");
    if json {
        words.pop();
    }
    if words.get(1) == Some(&"--") {
        words.remove(1);
    }
    let action = match words.as_slice() {
        ["list"] => Action::List,
        ["get", key] => Action::Get(key_argument(key)?),
        ["set", key, value] => Action::Set(key_argument(key)?, (*value).to_owned()),
        ["unset", key] => Action::Unset(key_argument(key)?),
        ["reload"] if !json => Action::Reload,
        _ => return Err(invalid("unknown config command; run muxy config --help")),
    };
    Ok(Command::Config(Invocation { action, json }))
}

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let profile = Profile::new(muxy_core::dirs::muxy_dir()?);
    let mut stdout = io::stdout().lock();
    match invocation.action {
        Action::List => {
            let entries = profile.list()?;
            if invocation.json {
                let object: Map<String, Value> = entries
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), serde_json::to_value(value)?)))
                    .collect::<Result<_, serde_json::Error>>()?;
                writeln!(stdout, "{}", Value::Object(object))?;
            } else {
                for (key, value) in &entries {
                    writeln!(stdout, "{key} = {value}")?;
                }
            }
        }
        Action::Get(key) => {
            let value = profile.get(&key)?;
            if invocation.json {
                writeln!(stdout, "{}", serde_json::to_value(&value)?)?;
            } else {
                writeln!(stdout, "{}", keys::display(&value))?;
            }
        }
        Action::Set(key, value) => {
            let saved = profile.set(&key, &value)?;
            let applied = reload_app()?;
            if invocation.json {
                writeln!(
                    stdout,
                    "{}",
                    json!({ "key": key, "value": serde_json::to_value(&saved)?, "applied": applied })
                )?;
            } else {
                writeln!(stdout, "{key} = {saved}")?;
            }
        }
        Action::Unset(key) => {
            profile.unset(&key)?;
            let applied = reload_app()?;
            if invocation.json {
                writeln!(stdout, "{}", json!({ "key": key, "applied": applied }))?;
            } else {
                writeln!(stdout, "{key} is back to its default")?;
            }
        }
        Action::Reload => {
            if reload_app()? {
                writeln!(stdout, "reloaded")?;
            } else {
                writeln!(stdout, "the Muxy desktop app is not running")?;
            }
        }
    }
    Ok(())
}

/// Has a running desktop app apply the files. Whether one did; the app is
/// never started for this, since it reads them when it starts.
fn reload_app() -> Result<bool, Box<dyn Error>> {
    match crate::browser::call_app(RELOAD, Map::new(), false, Duration::ZERO) {
        Ok(_) => Ok(true),
        Err(error) => match error.downcast_ref::<CallError>() {
            Some(CallError::NotRunning) => Ok(false),
            _ => Err(format!("saved, but the running app couldn't apply it: {error}").into()),
        },
    }
}

fn key_argument(key: &str) -> io::Result<String> {
    keys::parse_key(key).map_err(|error| invalid(&error.to_string()))?;
    Ok(key.to_owned())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(words: &[&str]) -> io::Result<Command> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    fn action(words: &[&str]) -> (Action, bool) {
        match parsed(words) {
            Ok(Command::Config(invocation)) => (invocation.action, invocation.json),
            other => panic!("{words:?} parsed as {other:?}"),
        }
    }

    #[test]
    fn commands_parse_to_their_action() {
        assert_eq!(action(&["list"]), (Action::List, false));
        assert_eq!(action(&["list", "--json"]), (Action::List, true));
        assert_eq!(
            action(&["get", "terminal.font_size"]),
            (Action::Get("terminal.font_size".into()), false)
        );
        assert_eq!(
            action(&["set", "appearance.dark_theme", "Builtin Dark", "--json"]),
            (
                Action::Set("appearance.dark_theme".into(), "Builtin Dark".into()),
                true
            )
        );
        assert_eq!(
            action(&["set", "--", "terminal.font_size", "-1"]),
            (Action::Set("terminal.font_size".into(), "-1".into()), false)
        );
        assert_eq!(
            action(&["unset", "panes.new_pane_directory"]),
            (Action::Unset("panes.new_pane_directory".into()), false)
        );
        assert_eq!(action(&["reload"]), (Action::Reload, false));
    }

    #[test]
    fn every_command_in_the_settings_skill_parses() {
        let commands: Vec<_> = muxy_app_core::settings::agent::SKILL
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix("muxy config "))
            .collect();
        assert!(commands.len() > 8, "{commands:?}");
        for command in commands {
            let words: Vec<&str> = if let Some((head, quoted)) = command.split_once(" \"") {
                head.split_whitespace()
                    .chain([quoted.trim_end_matches('"')])
                    .collect()
            } else {
                command.split_whitespace().collect()
            };
            assert!(
                matches!(parsed(&words), Ok(Command::Config(_))),
                "muxy config {command}"
            );
        }
        assert!(
            crate::args::parse(&["--host", "devbox", "config", "list"].map(OsString::from))
                .is_err()
        );
    }

    #[test]
    fn help_and_mistakes_are_reported() {
        for words in [&[][..], &["--help"], &["help"], &["set", "-h"]] {
            assert_eq!(parsed(words).ok(), Some(Command::Usage(help::CONFIG)));
        }
        for words in [
            &["get"][..],
            &["get", "a..b"],
            &["set", "panes.new_pane_directory"],
            &["reload", "--json"],
            &["remove", "x"],
        ] {
            assert!(parsed(words).is_err(), "{words:?}");
        }
    }
}
