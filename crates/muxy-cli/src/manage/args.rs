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
        create: bool,
        reuse: bool,
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

/// Session commands. A missing session or project means the terminal this
/// command runs in.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Session {
    List {
        project: Option<String>,
        all: bool,
    },
    Create {
        project: Option<String>,
        directory: Option<PathBuf>,
        size: Size,
        /// Typed into the new terminal, followed by Return.
        command: Option<String>,
    },
    End(Option<SessionId>),
    Discard(Option<SessionId>),
    Send {
        session: Option<SessionId>,
        text: String,
    },
    Key {
        session: Option<SessionId>,
        bytes: Vec<u8>,
    },
    Screen {
        session: Option<SessionId>,
        lines: usize,
        saved: bool,
    },
    History {
        session: Option<SessionId>,
        before: HistoryCursor,
        limit: u16,
        saved: bool,
    },
    Search {
        session: Option<SessionId>,
        query: String,
        before: HistoryCursor,
        limit: u16,
        ignore_case: bool,
        saved: bool,
    },
    Wait {
        session: Option<SessionId>,
        until: Until,
        timeout_ms: u32,
    },
}

/// What `session wait` waits for.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Until {
    Text { text: String, ignore_case: bool },
    Exit,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Worktree {
    List(String),
    Create {
        project: String,
        name: String,
        branch: String,
        base: Option<String>,
        directory: Option<PathBuf>,
        hooks: bool,
    },
    CheckoutPullRequest {
        project: String,
        number: u64,
        name: Option<String>,
        directory: Option<PathBuf>,
        hooks: bool,
    },
    Register {
        project: String,
        directory: PathBuf,
    },
    Remove {
        worktree: String,
        force: bool,
        hooks: bool,
    },
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
    let (values, flags) = options(group, verb);
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

/// The options that take a value, and the flags, of `group verb`.
fn options(group: &str, verb: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match (group, verb) {
        ("project", "add") => (&["--name"], &["--create", "--reuse"]),
        ("project", "delete") | ("session", "end" | "discard") => (&[], &["--yes"]),
        ("worktree", "remove") => (&[], &["--yes", "--force", "--hooks"]),
        ("server", "stop") => (&[], &["--force"]),
        ("session", "list") => (&["--project"], &["--all"]),
        ("session", "create") => (&["--directory", "--cols", "--rows"], &[]),
        ("session", "read-screen") => (&["--lines"], &["--saved"]),
        ("session", "history") => (&["--before", "--limit"], &["--saved"]),
        ("session", "search") => (&["--before", "--limit"], &["--saved", "--ignore-case"]),
        ("session", "wait") => (&["--text", "--timeout-ms"], &["--exit", "--ignore-case"]),
        ("worktree", "create") => (
            &["--branch", "--base", "--directory"],
            &["--existing", "--hooks"],
        ),
        ("worktree", "checkout-pr") => (&["--name", "--directory"], &["--hooks"]),
        ("exec", _) => (&["--timeout-ms"], &[]),
        _ => (&[], &[]),
    }
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
                create: o.flag("--create"),
                reuse: o.flag("--reuse"),
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
        return create_session(o);
    }
    let arguments = match verb {
        "send" | "send-keys" | "search" => 1,
        "end" | "discard" | "read-screen" | "history" | "wait" => 0,
        _ => return Err(invalid(help::SESSION)),
    };
    // The session ID comes first and may be left out inside a Muxy terminal.
    let given = o
        .words
        .len()
        .checked_sub(arguments)
        .filter(|given| *given <= 1)
        .ok_or_else(|| invalid("wrong number of arguments; run muxy <command> --help"))?;
    let session = if given == 1 {
        Some(
            o.word(0)?
                .parse::<u64>()
                .ok()
                .and_then(SessionId::new)
                .ok_or_else(|| invalid("session ID must be a nonzero decimal integer"))?,
        )
    } else {
        None
    };
    let argument = || o.word(given);
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
            text: argument()?.into(),
        }),
        "send-keys" => Ok(Session::Key {
            session,
            bytes: key(argument()?)?.to_vec(),
        }),
        "wait" => Ok(Session::Wait {
            session,
            until: until(o)?,
            timeout_ms: o.number("--timeout-ms", 30_000, 1, 3_600_000)?,
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
            let query = argument()?.to_owned();
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

/// `session create [project] [options] [-- COMMAND...]`.
fn create_session(o: &Options<'_>) -> io::Result<Session> {
    let (project, command) = o.words.split_at(o.literal.unwrap_or(o.words.len()));
    if project.len() > 1 {
        return Err(invalid(help::SESSION));
    }
    let command = command
        .iter()
        .map(|word| text(word))
        .collect::<io::Result<Vec<_>>>()?
        .join(" ");
    if o.literal.is_some() && command.trim().is_empty() {
        return Err(invalid("missing command after --"));
    }
    Ok(Session::Create {
        project: project
            .first()
            .map(|word| text(word))
            .transpose()?
            .map(str::to_owned),
        directory: o.values.get("--directory").map(PathBuf::from),
        size: Size {
            cols: o.number("--cols", 80, 1, muxy_protocol::MAX_COLS)?,
            rows: o.number("--rows", 24, 1, muxy_protocol::MAX_ROWS)?,
        },
        command: o.literal.map(|_| command),
    })
}

fn until(o: &Options<'_>) -> io::Result<Until> {
    match (o.value("--text")?, o.flag("--exit")) {
        (Some(text), false) => Ok(Until::Text {
            text: text.into(),
            ignore_case: o.flag("--ignore-case"),
        }),
        (None, true) if !o.flag("--ignore-case") => Ok(Until::Exit),
        _ => Err(invalid("use either --text TEXT [--ignore-case] or --exit")),
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
            let name = o.word(1)?.trim();
            if name.is_empty() {
                return Err(invalid("worktree name must not be empty"));
            }
            if o.flag("--existing") && o.value("--base")?.is_some() {
                return Err(invalid("--base and --existing cannot be combined"));
            }
            Ok(Worktree::Create {
                project: o.word(0)?.into(),
                name: name.into(),
                branch: o.value("--branch")?.unwrap_or(name).into(),
                base: if o.flag("--existing") {
                    None
                } else {
                    Some(o.value("--base")?.unwrap_or("HEAD").into())
                },
                directory: o.values.get("--directory").map(PathBuf::from),
                hooks: o.flag("--hooks"),
            })
        }
        "checkout-pr" => {
            o.count(2)?;
            Ok(Worktree::CheckoutPullRequest {
                project: o.word(0)?.into(),
                number: o
                    .word(1)?
                    .trim_start_matches('#')
                    .parse()
                    .ok()
                    .filter(|number| *number > 0)
                    .ok_or_else(|| invalid("pull request number must be a positive integer"))?,
                name: o.value("--name")?.map(str::to_owned),
                directory: o.values.get("--directory").map(PathBuf::from),
                hooks: o.flag("--hooks"),
            })
        }
        "remove" => {
            o.count(1)?;
            o.confirm()?;
            Ok(Worktree::Remove {
                worktree: o.word(0)?.into(),
                force: o.flag("--force"),
                hooks: o.flag("--hooks"),
            })
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
    /// How many words came before `--`, if it was given.
    literal: Option<usize>,
}
impl<'a> Options<'a> {
    fn parse(words: &'a [OsString], values: &[&str], flags: &[&str]) -> io::Result<Self> {
        let mut result = Self {
            words: Vec::new(),
            values: BTreeMap::new(),
            flags: BTreeSet::new(),
            literal: None,
        };
        let mut words = words.iter().map(OsString::as_os_str);
        while let Some(word) = words.next() {
            if word == "--" {
                result.literal = Some(result.words.len());
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
            vec!["session", "create", "Home", "Other"],
            vec!["session", "create", "Home", "--"],
            vec!["session", "send", "1", "two", "three"],
            vec!["session", "wait", "1"],
            vec!["session", "wait", "--text", "a", "--exit"],
            vec!["session", "wait", "--exit", "--ignore-case"],
            vec!["session", "wait", "--exit", "--timeout-ms", "3600001"],
            vec!["worktree", "create", "Home", " "],
            vec!["worktree", "checkout-pr", "Home", "0"],
            vec!["worktree", "checkout-pr", "Home", "twelve"],
        ] {
            assert!(words(&args).is_err(), "{args:?}");
        }
    }

    fn action(arguments: &[&str]) -> io::Result<Action> {
        words(arguments).map(|invocation| invocation.action)
    }

    #[test]
    fn session_commands_default_to_the_calling_terminal() -> io::Result<()> {
        let id = SessionId::new(5);
        assert_eq!(
            action(&["session", "send", "hello"])?,
            Action::Session(Session::Send {
                session: None,
                text: "hello".into()
            })
        );
        assert_eq!(
            action(&["session", "send", "5", "hello"])?,
            Action::Session(Session::Send {
                session: id,
                text: "hello".into()
            })
        );
        assert_eq!(
            action(&["session", "end", "--yes"])?,
            Action::Session(Session::End(None))
        );
        assert_eq!(
            action(&["session", "search", "5", "error"])?,
            action(&["session", "search", "5", "error", "--limit", "100"])?
        );
        assert_eq!(
            action(&[
                "session",
                "create",
                "--rows",
                "30",
                "--",
                "npm",
                "run",
                "dev --watch"
            ])?,
            Action::Session(Session::Create {
                project: None,
                directory: None,
                size: Size { cols: 80, rows: 30 },
                command: Some("npm run dev --watch".into()),
            })
        );
        assert_eq!(
            action(&["session", "create", "Home"])?,
            Action::Session(Session::Create {
                project: Some("Home".into()),
                directory: None,
                size: Size { cols: 80, rows: 24 },
                command: None,
            })
        );
        assert_eq!(
            action(&["session", "wait", "5", "--text", "ready", "--ignore-case"])?,
            Action::Session(Session::Wait {
                session: id,
                until: Until::Text {
                    text: "ready".into(),
                    ignore_case: true
                },
                timeout_ms: 30_000,
            })
        );
        assert_eq!(
            action(&["session", "wait", "--exit", "--timeout-ms", "500"])?,
            Action::Session(Session::Wait {
                session: None,
                until: Until::Exit,
                timeout_ms: 500,
            })
        );
        Ok(())
    }

    #[test]
    fn worktree_and_project_options() -> io::Result<()> {
        assert_eq!(
            action(&["worktree", "create", "App", "login"])?,
            Action::Worktree(Worktree::Create {
                project: "App".into(),
                name: "login".into(),
                branch: "login".into(),
                base: Some("HEAD".into()),
                directory: None,
                hooks: false,
            })
        );
        assert_eq!(
            action(&[
                "worktree",
                "create",
                "App",
                "fix",
                "--branch",
                "fix/x",
                "--existing",
                "--directory",
                "/tmp/fix",
                "--hooks",
            ])?,
            Action::Worktree(Worktree::Create {
                project: "App".into(),
                name: "fix".into(),
                branch: "fix/x".into(),
                base: None,
                directory: Some("/tmp/fix".into()),
                hooks: true,
            })
        );
        assert_eq!(
            action(&["worktree", "checkout-pr", "App", "#12"])?,
            Action::Worktree(Worktree::CheckoutPullRequest {
                project: "App".into(),
                number: 12,
                name: None,
                directory: None,
                hooks: false,
            })
        );
        assert_eq!(
            action(&["worktree", "remove", "login", "--yes", "--force", "--hooks"])?,
            Action::Worktree(Worktree::Remove {
                worktree: "login".into(),
                force: true,
                hooks: true,
            })
        );
        assert_eq!(
            action(&["project", "add", ".", "--create", "--reuse"])?,
            Action::Project(Project::Add {
                directory: ".".into(),
                name: None,
                create: true,
                reuse: true,
            })
        );
        Ok(())
    }
}
