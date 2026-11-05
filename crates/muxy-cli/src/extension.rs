//! `muxy extension`: manages the extensions of the desktop app on this
//! computer. Each command is one request to the app over its socket.

use std::error::Error;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::args::Command;
use crate::browser::call_app;
use crate::manage::help;

/// How much longer than a quick reply reading and restarting an extension,
/// which may have to ask its open pages first, can take.
const CHANGE_TIME: Duration = Duration::from_secs(30);
/// How long the user has to approve a publish, and Git and GitHub to do it.
const PUBLISH_TIME: Duration = Duration::from_secs(15 * 60);
const DEFAULT_LOG_LINES: u64 = 100;
const MAX_LOG_LINES: u64 = 10_000;

/// An extension command as the request it sends to the desktop app.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    verb: &'static str,
    args: Map<String, Value>,
    json: bool,
    extra_time: Duration,
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|word| word == "--help" || word == "-h")
        || arguments.first().is_some_and(|word| word == "help")
    {
        return Ok(Command::Usage(help::EXTENSION));
    }
    let mut words: Vec<&str> = arguments
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| invalid("arguments must be UTF-8"))
        })
        .collect::<io::Result<_>>()?;
    if words.get(1) == Some(&"--") {
        words.remove(1);
    } else if words.first() != Some(&"list")
        && words.get(1).is_some_and(|word| word.starts_with('-'))
    {
        return Err(invalid(
            "expected a name or folder; use -- before one that starts with a dash",
        ));
    }
    let invocation = match words.as_slice() {
        ["list"] => request("extension.list", Map::new(), false),
        ["list", "--json"] => request("extension.list", Map::new(), true),
        ["status", name] => named("extension.status", name, false)?,
        ["status", name, "--json"] => named("extension.status", name, true)?,
        ["logs", name] => logs(name, DEFAULT_LOG_LINES)?,
        ["logs", name, "--lines", lines] => logs(name, line_count(lines)?)?,
        ["load", folder] => load(folder)?,
        ["enable", name] => changed("extension.enable", name)?,
        ["disable", name] => changed("extension.disable", name)?,
        ["reload", name] => changed("extension.reload", name)?,
        ["unload", name] => changed("extension.unload", name)?,
        ["validate", target] => validate(target, false)?,
        ["validate", target, "--json"] => validate(target, true)?,
        ["publish", name] => publish(name, false)?,
        ["publish", name, "--json"] => publish(name, true)?,
        _ => {
            return Err(invalid(
                "unknown extension command; run muxy extension --help",
            ));
        }
    };
    Ok(Command::Extension(Box::new(invocation)))
}

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let reply = call_app(
        invocation.verb,
        invocation.args,
        true,
        invocation.extra_time,
    )?;
    let mut stdout = io::stdout().lock();
    if invocation.json {
        writeln!(stdout, "{reply}")?;
    } else {
        write!(stdout, "{}", text(invocation.verb, &reply))?;
    }
    let problems = problem_count(invocation.verb, &reply);
    if problems > 0 {
        return Err(format!("{problems} thing(s) to fix before publishing").into());
    }
    Ok(())
}

fn request(verb: &'static str, args: Map<String, Value>, json: bool) -> Invocation {
    Invocation {
        verb,
        args,
        json,
        extra_time: Duration::ZERO,
    }
}

fn name_argument(name: &str) -> io::Result<Map<String, Value>> {
    if name.is_empty() {
        return Err(invalid("expected an extension name"));
    }
    Ok(Map::from_iter([("name".to_owned(), json!(name))]))
}

fn named(verb: &'static str, name: &str, json: bool) -> io::Result<Invocation> {
    Ok(request(verb, name_argument(name)?, json))
}

fn changed(verb: &'static str, name: &str) -> io::Result<Invocation> {
    Ok(Invocation {
        extra_time: CHANGE_TIME,
        ..named(verb, name, false)?
    })
}

fn logs(name: &str, lines: u64) -> io::Result<Invocation> {
    let mut invocation = named("extension.logs", name, false)?;
    invocation.args.insert("lines".to_owned(), json!(lines));
    Ok(invocation)
}

fn line_count(lines: &str) -> io::Result<u64> {
    lines
        .parse()
        .ok()
        .filter(|lines| (1..=MAX_LOG_LINES).contains(lines))
        .ok_or_else(|| invalid("--lines must be a number from 1 to 10000"))
}

fn folder_argument(folder: &str) -> io::Result<String> {
    if folder.is_empty() {
        return Err(invalid("expected a folder"));
    }
    std::path::absolute(Path::new(folder))?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("the folder's path must be UTF-8"))
}

fn load(folder: &str) -> io::Result<Invocation> {
    Ok(Invocation {
        verb: "extension.load",
        args: Map::from_iter([("path".to_owned(), json!(folder_argument(folder)?))]),
        json: false,
        extra_time: CHANGE_TIME,
    })
}

/// A bare word names a loaded extension, and anything with a path in it a folder.
fn validate(target: &str, json: bool) -> io::Result<Invocation> {
    let args = if target.contains('/') || target == "." || target == ".." {
        Map::from_iter([("path".to_owned(), json!(folder_argument(target)?))])
    } else {
        name_argument(target)?
    };
    Ok(Invocation {
        extra_time: CHANGE_TIME,
        ..request("extension.validate", args, json)
    })
}

fn publish(name: &str, json: bool) -> io::Result<Invocation> {
    Ok(Invocation {
        extra_time: PUBLISH_TIME,
        ..named("extension.publish", name, json)?
    })
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn strings<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn state(extension: &Value) -> &'static str {
    if extension.get("enabled").and_then(Value::as_bool) == Some(true) {
        "enabled"
    } else {
        "disabled"
    }
}

fn source(extension: &Value) -> &'static str {
    if extension.get("unpacked").and_then(Value::as_bool) == Some(true) {
        "unpacked"
    } else {
        "marketplace"
    }
}

fn list_row(extension: &Value) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        string(extension, "name"),
        string(extension, "version"),
        state(extension),
        source(extension),
        string(extension, "path"),
    )
}

fn text(verb: &str, reply: &Value) -> String {
    let mut output = String::new();
    match verb {
        "extension.list" => {
            for extension in reply
                .get("extensions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                output.push_str(&list_row(extension));
            }
            for error in strings(reply, "load_errors") {
                let _ = writeln!(output, "load error: {error}");
            }
        }
        "extension.status" => {
            output.push_str(&list_row(reply));
            for error in strings(reply, "load_errors") {
                let _ = writeln!(output, "load error: {error}");
            }
            for line in strings(reply, "log") {
                output.push_str(line);
                output.push('\n');
            }
        }
        "extension.logs" => {
            for line in strings(reply, "lines") {
                output.push_str(line);
                output.push('\n');
            }
        }
        "extension.load" => {
            output.push_str(string(reply, "name"));
            output.push('\n');
        }
        "extension.validate" => {
            let mut any = false;
            for problem in strings(reply, "problems") {
                any = true;
                output.push_str(problem);
                output.push('\n');
            }
            match github_gap(reply) {
                Some(gap) => {
                    let _ = writeln!(output, "{gap}");
                }
                None if !any => output.push_str("ready to publish\n"),
                None => {}
            }
        }
        "extension.publish" => {
            output.push_str(string(reply, "url"));
            output.push('\n');
        }
        _ => output.push_str("ok\n"),
    }
    output
}

/// What stops a package that is otherwise ready from being published.
fn github_gap(reply: &Value) -> Option<&'static str> {
    match string(reply, "github") {
        "missing" => {
            Some("publishing needs the GitHub CLI (gh): install it from https://cli.github.com")
        }
        "signed-out" => Some("the GitHub CLI (gh) isn't signed in: run gh auth login"),
        "unreachable" => Some("GitHub couldn't be reached, so the store wasn't checked"),
        _ => None,
    }
}

fn problem_count(verb: &str, reply: &Value) -> usize {
    if verb == "extension.validate" {
        strings(reply, "problems").count() + usize::from(github_gap(reply).is_some())
    } else {
        0
    }
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

    fn invocation(words: &[&str]) -> io::Result<Invocation> {
        match parsed(words)? {
            Command::Extension(invocation) => Ok(*invocation),
            other => Err(invalid(&format!("unexpected {other:?}"))),
        }
    }

    #[test]
    fn commands_become_app_requests() -> io::Result<()> {
        assert_eq!(invocation(&["list"])?.verb, "extension.list");
        assert!(invocation(&["list", "--json"])?.json);
        let reload = invocation(&["reload", "hello"])?;
        assert_eq!(reload.verb, "extension.reload");
        assert_eq!(reload.args["name"], "hello");
        assert_eq!(reload.extra_time, CHANGE_TIME);
        assert_eq!(
            invocation(&["logs", "hello"])?.args["lines"],
            DEFAULT_LOG_LINES
        );
        assert_eq!(
            invocation(&["logs", "hello", "--lines", "5"])?.args["lines"],
            5
        );
        assert_eq!(invocation(&["publish", "hello"])?.extra_time, PUBLISH_TIME);
        Ok(())
    }

    #[test]
    fn folders_reach_the_app_as_absolute_paths() -> io::Result<()> {
        let load = invocation(&["load", "extensions/hello"])?;
        let path = load.args["path"].as_str().unwrap_or_default();
        assert!(Path::new(path).is_absolute(), "{path}");
        assert!(path.ends_with("extensions/hello"), "{path}");
        Ok(())
    }

    #[test]
    fn validate_takes_a_name_or_a_folder() -> io::Result<()> {
        assert_eq!(invocation(&["validate", "hello"])?.args["name"], "hello");
        assert!(
            invocation(&["validate", "./hello"])?
                .args
                .contains_key("path")
        );
        assert!(invocation(&["validate", "."])?.args.contains_key("path"));
        Ok(())
    }

    #[test]
    fn a_double_dash_allows_a_name_or_folder_that_starts_with_a_dash() -> io::Result<()> {
        assert_eq!(invocation(&["reload", "--", "-odd"])?.args["name"], "-odd");
        let load = invocation(&["load", "--", "-odd"])?;
        assert!(
            load.args["path"]
                .as_str()
                .unwrap_or_default()
                .ends_with("-odd")
        );
        assert!(parsed(&["reload", "-odd"]).is_err());
        Ok(())
    }

    #[test]
    fn bad_commands_are_refused() {
        for words in [
            vec!["install", "hello"],
            vec!["uninstall", "hello"],
            vec!["reload"],
            vec!["reload", "--json"],
            vec!["load", "--force"],
            vec!["logs", "hello", "--lines", "0"],
            vec!["logs", "hello", "--lines", "many"],
        ] {
            assert!(parsed(&words).is_err(), "{words:?}");
        }
    }

    #[test]
    fn help_and_host_rules_match_the_other_app_commands() -> io::Result<()> {
        assert_eq!(parsed(&[])?, Command::Usage(help::EXTENSION));
        assert_eq!(parsed(&["--help"])?, Command::Usage(help::EXTENSION));
        assert_eq!(
            parsed(&["reload", "--help"])?,
            Command::Usage(help::EXTENSION)
        );
        let host = ["--host", "example", "extension", "list"].map(OsString::from);
        assert!(crate::args::parse(&host).is_err());
        let topic = ["help", "extension"].map(OsString::from);
        assert_eq!(
            crate::args::parse(&topic)?.1,
            Command::Usage(help::EXTENSION)
        );
        Ok(())
    }

    #[test]
    fn replies_print_as_text() {
        let extension = json!({
            "name": "hello", "version": "0.1.0", "enabled": true,
            "unpacked": true, "path": "/work/hello",
        });
        assert_eq!(
            text(
                "extension.list",
                &json!({"extensions": [extension], "load_errors": ["bad"]})
            ),
            "hello\t0.1.0\tenabled\tunpacked\t/work/hello\nload error: bad\n"
        );
        assert_eq!(text("extension.reload", &json!(null)), "ok\n");
        assert_eq!(text("extension.load", &json!({"name": "hello"})), "hello\n");
        assert_eq!(
            text(
                "extension.validate",
                &json!({"problems": [], "github": "octocat"})
            ),
            "ready to publish\n"
        );
        let no_gh = json!({"problems": [], "github": "missing"});
        assert!(text("extension.validate", &no_gh).contains("GitHub CLI"));
        assert!(!text("extension.validate", &no_gh).contains("ready to publish"));
        assert_eq!(problem_count("extension.validate", &no_gh), 1);
        let signed_in = json!({"problems": [], "github": "octocat"});
        assert_eq!(problem_count("extension.validate", &signed_in), 0);
        assert_eq!(
            problem_count("extension.validate", &json!({"problems": ["a", "b"]})),
            2
        );
    }

    #[test]
    fn every_command_in_the_authoring_skill_parses() {
        let skill = muxy_app_core::extensions::workspace::SKILL;
        let commands: Vec<&str> = skill
            .lines()
            .flat_map(|line| line.split('`'))
            .chain(skill.lines())
            .filter_map(|text| text.trim().strip_prefix("muxy extension "))
            .filter(|command| !command.contains('<') && !command.contains('`'))
            .collect();
        assert!(commands.len() > 12, "{commands:?}");
        for command in commands {
            let words: Vec<_> = command.split_whitespace().collect();
            assert!(parsed(&words).is_ok(), "muxy extension {command}");
        }
    }
}
