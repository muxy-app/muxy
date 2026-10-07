use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::PathBuf;

use muxy_protocol::{HistoryCursor, SessionId, Size};

use super::help;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Invocation {
    pub(crate) json: bool,
    pub(crate) action: Action,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Help(&'static str),
    Server(Server),
    Project(Project),
    Session(Session),
    Worktree(Worktree),
    SettingsGet,
    SettingsSet {
        key: String,
        value: String,
    },
    Activity(Vec<u64>),
    Git {
        project: String,
        action: String,
    },
    Files {
        project: String,
        action: String,
    },
    Exec {
        project: String,
        argv: Vec<String>,
        timeout_ms: u32,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Server {
    Start,
    Status,
    Stop { force: bool },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Project {
    List,
    Add {
        directory: PathBuf,
        name: Option<String>,
    },
    Rename {
        project: String,
        name: String,
    },
    Color {
        project: String,
        color: String,
    },
    Icon {
        project: String,
        icon: Option<String>,
    },
    Delete(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Session {
    List {
        project: Option<String>,
        all: bool,
    },
    Create {
        project: String,
        directory: Option<PathBuf>,
        size: Size,
    },
    End(SessionId),
    Discard(SessionId),
    Send {
        session: SessionId,
        text: String,
    },
    Key {
        session: SessionId,
        bytes: Vec<u8>,
    },
    Screen {
        session: SessionId,
        lines: usize,
        saved: bool,
    },
    History {
        session: SessionId,
        before: HistoryCursor,
        limit: u16,
        saved: bool,
    },
    Search {
        session: SessionId,
        query: String,
        before: HistoryCursor,
        limit: u16,
        ignore_case: bool,
        saved: bool,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Worktree {
    List(String),
    Create {
        project: String,
        directory: PathBuf,
        branch: String,
        base: Option<String>,
    },
    Register {
        project: String,
        directory: PathBuf,
    },
    Remove(String),
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Invocation> {
    let group = arguments.first().map_or(Ok(""), |s| text(s))?;
    let usage = help::topic(group).ok_or_else(|| invalid("unknown command; run muxy --help"))?;
    if (2..=3).contains(&arguments.len())
        && arguments.last().is_some_and(|s| s == "--help" || s == "-h")
    {
        return Ok(Invocation {
            json: false,
            action: Action::Help(usage),
        });
    }
    let verb = arguments.get(1).map_or(Ok(""), |s| text(s))?;
    let rest = arguments.get(2..).unwrap_or_default();
    let (values, flags): (&[&str], &[&str]) = match (group, verb) {
        ("project", "add") => (&["--name"], &[]),
        ("project", "delete") | ("session", "end" | "discard") | ("worktree", "remove") => {
            (&[], &["--yes"])
        }
        ("server", "stop") => (&[], &["--force"]),
        ("session", "list") => (&["--project"], &["--all"]),
        ("session", "create") => (&["--directory", "--cols", "--rows"], &[]),
        ("session", "read-screen") => (&["--lines"], &["--saved"]),
        ("session", "history") => (&["--before", "--limit"], &["--saved"]),
        ("session", "search") => (&["--before", "--limit"], &["--saved", "--ignore-case"]),
        ("worktree", "create") => (&["--branch", "--base"], &["--existing"]),
        ("exec", _) => (&["--timeout-ms"], &[]),
        _ => (&[], &[]),
    };
    let mut options = Options::parse(rest, values, flags)?;
    let action = match group {
        "server" => Action::Server(server(verb, &mut options)?),
        "project" => Action::Project(project(verb, &mut options)?),
        "session" => Action::Session(session(verb, &mut options)?),
        "worktree" => Action::Worktree(worktree(verb, &mut options)?),
        "settings" => match verb {
            "get" => {
                options.count(0)?;
                Action::SettingsGet
            }
            "set" => {
                options.count(2)?;
                Action::SettingsSet {
                    key: options.word(0)?.into(),
                    value: options.word(1)?.into(),
                }
            }
            _ => return Err(invalid(usage)),
        },
        "activity" => {
            let events = match verb {
                "list" => {
                    options.count(0)?;
                    Vec::new()
                }
                "ack" if !options.words.is_empty() => options
                    .words
                    .iter()
                    .map(|s| {
                        text(s)?
                            .parse()
                            .map_err(|_| invalid("event IDs must be integers"))
                    })
                    .collect::<io::Result<Vec<_>>>()?,
                _ => return Err(invalid(usage)),
            };
            Action::Activity(events)
        }
        "git" | "files" if !verb.is_empty() && !verb.starts_with('-') => {
            options.count(1)?;
            if group == "git" {
                Action::Git {
                    project: verb.into(),
                    action: options.word(0)?.into(),
                }
            } else {
                Action::Files {
                    project: verb.into(),
                    action: options.word(0)?.into(),
                }
            }
        }
        "exec" if !verb.is_empty() && !verb.starts_with('-') && !options.words.is_empty() => {
            Action::Exec {
                project: verb.into(),
                argv: options
                    .words
                    .iter()
                    .map(|s| Ok(text(s)?.to_owned()))
                    .collect::<io::Result<_>>()?,
                timeout_ms: options.number("--timeout-ms", 30_000, 1, 300_000)?,
            }
        }
        _ => return Err(invalid(usage)),
    };
    Ok(Invocation {
        json: options.flag("--json"),
        action,
    })
}

fn server(verb: &str, options: &mut Options<'_>) -> io::Result<Server> {
    options.count(0)?;
    match verb {
        "start" => Ok(Server::Start),
        "status" => Ok(Server::Status),
        "stop" => Ok(Server::Stop {
            force: options.flag("--force"),
        }),
        _ => Err(invalid(help::SERVER)),
    }
}

fn project(verb: &str, o: &mut Options<'_>) -> io::Result<Project> {
    match verb {
        "list" => {
            o.count(0)?;
            Ok(Project::List)
        }
        "add" => {
            o.count(1)?;
            Ok(Project::Add {
                directory: o.words[0].into(),
                name: o.value("--name")?.map(str::to_owned),
            })
        }
        "rename" | "set-color" => {
            o.count(2)?;
            Ok(if verb == "rename" {
                Project::Rename {
                    project: o.word(0)?.into(),
                    name: o.word(1)?.into(),
                }
            } else {
                Project::Color {
                    project: o.word(0)?.into(),
                    color: o.word(1)?.into(),
                }
            })
        }
        "set-icon" if (1..=2).contains(&o.words.len()) => Ok(Project::Icon {
            project: o.word(0)?.into(),
            icon: o
                .words
                .get(1)
                .map(|s| text(s).map(str::to_owned))
                .transpose()?,
        }),
        "delete" => {
            o.count(1)?;
            o.confirm()?;
            Ok(Project::Delete(o.word(0)?.into()))
        }
        _ => Err(invalid(help::PROJECT)),
    }
}

fn session(verb: &str, o: &mut Options<'_>) -> io::Result<Session> {
    if verb == "list" {
        o.count(0)?;
        return Ok(Session::List {
            project: o.value("--project")?.map(str::to_owned),
            all: o.flag("--all"),
        });
    }
    if verb == "create" {
        o.count(1)?;
        return Ok(Session::Create {
            project: o.word(0)?.into(),
            directory: o.values.get("--directory").map(PathBuf::from),
            size: Size {
                cols: o.number("--cols", 80, 1, muxy_protocol::MAX_COLS)?,
                rows: o.number("--rows", 24, 1, muxy_protocol::MAX_ROWS)?,
            },
        });
    }
    o.count(if matches!(verb, "send" | "send-keys" | "search") {
        2
    } else {
        1
    })?;
    let session = o
        .word(0)?
        .parse::<u64>()
        .ok()
        .and_then(SessionId::new)
        .ok_or_else(|| invalid("session ID must be a nonzero decimal integer"))?;
    let saved = o.flag("--saved");
    match verb {
        "end" => {
            o.confirm()?;
            Ok(Session::End(session))
        }
        "discard" => {
            o.confirm()?;
            Ok(Session::Discard(session))
        }
        "send" => Ok(Session::Send {
            session,
            text: o.word(1)?.into(),
        }),
        "send-keys" => Ok(Session::Key {
            session,
            bytes: key(o.word(1)?)?.to_vec(),
        }),
        "read-screen" => Ok(Session::Screen {
            session,
            lines: o.number("--lines", 50, 1, usize::from(muxy_protocol::MAX_ROWS))?,
            saved,
        }),
        "history" => Ok(Session::History {
            session,
            before: HistoryCursor(o.number("--before", 0, 0, u64::MAX)?),
            limit: o.number("--limit", 200, 1, 500)?,
            saved,
        }),
        "search" => {
            let query = o.word(1)?.to_owned();
            let limit = o.number("--limit", 100, 1, 500)?;
            muxy_protocol::validate_search(&query, limit)
                .map_err(|_| invalid("search query must contain 1 to 256 bytes"))?;
            Ok(Session::Search {
                session,
                query,
                before: HistoryCursor(o.number("--before", 0, 0, u64::MAX)?),
                limit,
                ignore_case: o.flag("--ignore-case"),
                saved,
            })
        }
        _ => Err(invalid(help::SESSION)),
    }
}

fn worktree(verb: &str, o: &mut Options<'_>) -> io::Result<Worktree> {
    match verb {
        "list" => {
            o.count(1)?;
            Ok(Worktree::List(o.word(0)?.into()))
        }
        "register" => {
            o.count(2)?;
            Ok(Worktree::Register {
                project: o.word(0)?.into(),
                directory: o.words[1].into(),
            })
        }
        "create" => {
            o.count(2)?;
            let branch = o
                .value("--branch")?
                .ok_or_else(|| invalid("--branch is required"))?
                .into();
            if o.flag("--existing") && o.value("--base")?.is_some() {
                return Err(invalid("--base and --existing cannot be combined"));
            }
            Ok(Worktree::Create {
                project: o.word(0)?.into(),
                directory: o.words[1].into(),
                branch,
                base: if o.flag("--existing") {
                    None
                } else {
                    Some(o.value("--base")?.unwrap_or("HEAD").into())
                },
            })
        }
        "remove" => {
            o.count(1)?;
            o.confirm()?;
            Ok(Worktree::Remove(o.word(0)?.into()))
        }
        _ => Err(invalid(help::WORKTREE)),
    }
}

fn key(key: &str) -> io::Result<&'static [u8]> {
    match key.to_ascii_lowercase().as_str() {
        "escape" | "esc" => Ok(b"\x1b"),
        "enter" | "return" => Ok(b"\r"),
        "tab" => Ok(b"\t"),
        "ctrl+c" | "ctrl-c" => Ok(b"\x03"),
        "ctrl+d" | "ctrl-d" => Ok(b"\x04"),
        "ctrl+z" | "ctrl-z" => Ok(b"\x1a"),
        "backspace" => Ok(b"\x7f"),
        _ => Err(invalid(
            "unsupported key; use Escape, Enter, Tab, Ctrl+C, Ctrl+D, Ctrl+Z or Backspace",
        )),
    }
}

struct Options<'a> {
    words: Vec<&'a OsStr>,
    values: BTreeMap<&'a str, &'a OsStr>,
    flags: BTreeSet<&'a str>,
}
impl<'a> Options<'a> {
    fn parse(words: &'a [OsString], values: &[&str], flags: &[&str]) -> io::Result<Self> {
        let mut result = Self {
            words: Vec::new(),
            values: BTreeMap::new(),
            flags: BTreeSet::new(),
        };
        let mut words = words.iter().map(OsString::as_os_str);
        while let Some(word) = words.next() {
            if word == "--" {
                result.words.extend(words);
                break;
            }
            let option = word.to_str().unwrap_or("");
            if values.contains(&option) {
                let value = words
                    .next()
                    .filter(|s| !s.is_empty() && !s.as_encoded_bytes().starts_with(b"--"))
                    .ok_or_else(|| invalid(&format!("missing value for {option}")))?;
                if result.values.insert(option, value).is_some() {
                    return Err(invalid("duplicate option"));
                }
            } else if flags.contains(&option) || word == "--json" {
                if !result.flags.insert(option) {
                    return Err(invalid("duplicate option"));
                }
            } else if word.as_encoded_bytes().starts_with(b"-") {
                return Err(invalid(&format!(
                    "unknown option {option}; use -- before literal arguments"
                )));
            } else {
                result.words.push(word);
            }
        }
        Ok(result)
    }
    fn count(&self, count: usize) -> io::Result<()> {
        if self.words.len() == count {
            Ok(())
        } else {
            Err(invalid(
                "wrong number of arguments; run muxy <command> --help",
            ))
        }
    }
    fn flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
    fn word(&self, index: usize) -> io::Result<&'a str> {
        text(self.words[index])
    }
    fn value(&self, flag: &str) -> io::Result<Option<&'a str>> {
        self.values.get(flag).map(|s| text(s)).transpose()
    }
    fn confirm(&self) -> io::Result<()> {
        if self.flag("--yes") {
            Ok(())
        } else {
            Err(invalid(
                "this deletes data or ends terminals; pass --yes to confirm",
            ))
        }
    }
    fn number<T: std::str::FromStr + PartialOrd + Copy>(
        &self,
        flag: &str,
        default: T,
        min: T,
        max: T,
    ) -> io::Result<T> {
        self.value(flag)?.map_or(Ok(default), |s| {
            s.parse::<T>()
                .ok()
                .filter(|n| *n >= min && *n <= max)
                .ok_or_else(|| invalid(&format!("invalid value for {flag}")))
        })
    }
}
fn text(value: &OsStr) -> io::Result<&str> {
    value
        .to_str()
        .ok_or_else(|| invalid("text arguments must be UTF-8"))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(words: &[&str]) -> io::Result<Invocation> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }
    #[test]
    fn filesystem_operands_preserve_raw_bytes_and_text_stays_validated() -> io::Result<()> {
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(b"/tmp/raw-\xff".to_vec());
        assert_eq!(
            parse(&["project".into(), "add".into(), path.clone()])?.action,
            Action::Project(Project::Add {
                directory: path.clone().into(),
                name: None
            })
        );
        let create = parse(&[
            "session".into(),
            "create".into(),
            "Home".into(),
            "--directory".into(),
            path.clone(),
        ])?;
        assert!(
            matches!(create.action, Action::Session(Session::Create { directory: Some(directory), .. }) if directory.as_os_str() == path)
        );
        let worktree = parse(&[
            "worktree".into(),
            "register".into(),
            "Home".into(),
            path.clone(),
        ])?;
        assert!(
            matches!(worktree.action, Action::Worktree(Worktree::Register { directory, .. }) if directory.as_os_str() == path)
        );
        assert!(
            parse(&[
                "project".into(),
                "add".into(),
                "/tmp".into(),
                "--name".into(),
                path
            ])
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_or_unconfirmed_operations() {
        for args in [
            vec!["session", "create", "Home", "--cols", "0"],
            vec!["session", "send", "0", "text"],
            vec!["session", "end", "1"],
            vec!["project", "delete", "Home"],
            vec!["worktree", "remove", "feature"],
            vec!["session", "read-screen", "1", "--lines", "no"],
            vec!["session", "send-keys", "1", "F99"],
            vec!["project", "list", "--json", "--json"],
            vec![
                "worktree",
                "create",
                "Home",
                "/tmp/w",
                "--branch",
                "x",
                "--base",
                "main",
                "--existing",
            ],
            vec!["session", "history", "1", "--limit", "501"],
        ] {
            assert!(words(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn preserves_literal_text_and_exec_arguments() -> io::Result<()> {
        assert_eq!(
            words(&["session", "send", "1", "--", "--json"])?.action,
            Action::Session(Session::Send {
                session: SessionId::new(1).ok_or(io::ErrorKind::InvalidInput)?,
                text: "--json".into()
            })
        );
        assert_eq!(
            words(&["exec", "Home", "--json", "--", "printf", "%s", "--help"])?,
            Invocation {
                json: true,
                action: Action::Exec {
                    project: "Home".into(),
                    argv: vec!["printf".into(), "%s".into(), "--help".into()],
                    timeout_ms: 30_000
                }
            }
        );
        assert!(matches!(
            words(&["session", "create", "--help"])?.action,
            Action::Help(_)
        ));
        Ok(())
    }
}
