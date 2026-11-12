use std::ffi::{OsStr, OsString};
use std::io;
use std::path::PathBuf;

use muxy_client::{SshTarget, Start};
use muxy_protocol::MAX_PAIRING_HOSTS;

use crate::manage::help;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Help,
    /// One command's usage, which needs no server.
    Usage(&'static str),
    DetailedUsage(String),
    Completion(crate::completion::Shell),
    Complete(Vec<OsString>),
    Version,
    BuildInfo,
    Interactive,
    Manage(Box<crate::manage::args::Invocation>),
    Mobile(Mobile),
    /// Joins stdin and stdout to the server, for clients on other computers.
    Stdio(Start),
    Open(PathBuf),
    /// Drives a browser tab in the desktop app on this computer.
    Browser(Box<crate::browser::Invocation>),
    /// Manages extensions in the desktop app on this computer.
    Extension(Box<crate::extension::Invocation>),
    /// Builds and reloads an extension whenever its files change.
    ExtensionDev(crate::extension::dev::Dev),
    /// Reads or changes the desktop app's settings on this computer.
    Config(crate::config::Invocation),
    /// Installs the agent skill into agent folders and these folders.
    InstallSkills(Vec<PathBuf>),
    /// A program status report for this computer's terminal.
    Status(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Mobile {
    Status,
    Enable {
        port: Option<u16>,
    },
    Disable,
    Pair {
        addresses: Vec<String>,
    },
    Revoke {
        device: String,
    },
    /// Turns push notifications to paired phones on or off.
    Push {
        on: bool,
    },
    /// Forgets one phone's push registration and key.
    ResetPush {
        device: String,
    },
}

/// Reads the command, and the other computer whose server it uses when it
/// starts with `--host DESTINATION`. Help, version and build info describe
/// this computer's Muxy, a bridge must never reach on to another computer,
/// and browser tabs, extensions and app settings live in this computer's
/// desktop app, so none of them takes `--host`.
pub(crate) fn parse(arguments: &[OsString]) -> io::Result<(Option<SshTarget>, Command)> {
    match arguments {
        [flag, destination, rest @ ..] if flag == "--host" => {
            if rest.first().is_some_and(|word| word == "--host") {
                return Err(invalid("duplicate option --host"));
            }
            let host = destination
                .to_str()
                .ok_or_else(|| invalid("--host must be UTF-8"))?
                .parse()?;
            match (command(rest)?, rest) {
                (
                    Command::Help
                    | Command::Version
                    | Command::BuildInfo
                    | Command::Completion(_)
                    | Command::Complete(_)
                    | Command::Stdio(_)
                    | Command::Open(_)
                    | Command::Browser(_)
                    | Command::Extension(_)
                    | Command::ExtensionDev(_)
                    | Command::Config(_)
                    | Command::InstallSkills(_)
                    | Command::Status(_),
                    [word, ..],
                ) => Err(invalid(&format!(
                    "--host can't be combined with {}",
                    word.to_string_lossy()
                ))),
                (command, _) => Ok((Some(host), command)),
            }
        }
        [flag] if flag == "--host" => Err(invalid("usage: muxy --host DESTINATION [COMMAND]")),
        _ => Ok((None, command(arguments)?)),
    }
}

fn command(arguments: &[OsString]) -> io::Result<Command> {
    if let Some(alias) = arguments
        .first()
        .and_then(|word| word.to_str())
        .and_then(help::alias)
    {
        let mut expanded: Vec<_> = alias.split_whitespace().map(OsString::from).collect();
        expanded.extend_from_slice(&arguments[1..]);
        return command(&expanded);
    }
    if let Some(usage) = help::requested(arguments) {
        return Ok(Command::DetailedUsage(usage));
    }
    match arguments {
        [flag] if flag == "--help" || flag == "-h" => return Ok(Command::Help),
        [flag] if flag == "--version" || flag == "-V" => return Ok(Command::Version),
        [flag] if flag == "--build-info" => return Ok(Command::BuildInfo),
        _ => {}
    }
    match arguments {
        [] => Ok(Command::Interactive),
        [word] if word == "version" => Ok(Command::Version),
        [word, rest @ ..] if word == "__complete" => Ok(Command::Complete(rest.to_vec())),
        [word, shell] if word == "completion" && shell != "--help" && shell != "-h" => {
            crate::completion::Shell::parse(shell.to_str().unwrap_or_default())
                .map(Command::Completion)
        }
        [word]
            if matches!(
                word.to_str(),
                Some(
                    "server"
                        | "project"
                        | "session"
                        | "worktree"
                        | "settings"
                        | "activity"
                        | "completion"
                )
            ) =>
        {
            Ok(Command::Usage(
                help::topic(word.to_str().unwrap_or_default()).unwrap_or(help::ROOT),
            ))
        }
        [word, flag]
            if (flag == "--help" || flag == "-h")
                && help::topic(word.to_str().unwrap_or_default()).is_some() =>
        {
            Ok(Command::Usage(
                help::topic(word.to_str().unwrap_or_default()).unwrap_or(help::ROOT),
            ))
        }
        [command, .., flag] if command == "mobile" && (flag == "--help" || flag == "-h") => {
            Ok(Command::Usage(help::MOBILE))
        }
        [command, rest @ ..] if command == "mobile" => mobile(rest).map(Command::Mobile),
        [command, rest @ ..] if command == "stdio" => stdio(rest).map(Command::Stdio),
        [command, .., flag]
            if command == "install-skills" && (flag == "--help" || flag == "-h") =>
        {
            Ok(Command::Usage(help::INSTALL_SKILLS))
        }
        [command, rest @ ..] if command == "install-skills" => {
            install_skills(rest).map(Command::InstallSkills)
        }
        [command, rest @ ..] if command == "browser" => crate::browser::parse(rest),
        [command, rest @ ..] if command == "extension" => crate::extension::parse(rest),
        [command, rest @ ..] if command == "config" => crate::config::parse(rest),
        [command, rest @ ..] if command == "status" && asks_help(rest) => {
            Ok(Command::Usage(help::STATUS))
        }
        [command, rest @ ..] if command == "status" => {
            crate::status::parse(rest).map(Command::Status)
        }
        [command] if command == "help" => Ok(Command::Help),
        [command, topics @ ..] if command == "help" => {
            let topics = topics
                .iter()
                .map(|word| {
                    word.to_str()
                        .ok_or_else(|| invalid("help topics must be UTF-8"))
                })
                .collect::<io::Result<Vec<_>>>()?;
            let path = topics.join(" ");
            if help::alias(&path).is_none()
                && let Some(usage) = help::topic(&path)
            {
                return Ok(Command::Usage(usage));
            }
            help::detail(&path)
                .map(Command::DetailedUsage)
                .ok_or_else(|| invalid("unknown command; run muxy --help"))
        }
        [word] if names_folder(word) => Ok(Command::Open(word.into())),
        _ => {
            crate::manage::args::parse(arguments).map(|command| Command::Manage(Box::new(command)))
        }
    }
}

/// Whether `--help` or `-h` stands where `muxy status` reads its state or an
/// option, not where it reads an option's value.
fn asks_help(arguments: &[OsString]) -> bool {
    arguments
        .iter()
        .enumerate()
        .any(|(index, word)| (index == 0 || index % 2 == 1) && (word == "--help" || word == "-h"))
}

fn names_folder(word: &OsStr) -> bool {
    !word.is_empty()
        && !word.as_encoded_bytes().starts_with(b"-")
        && word.to_str().is_none_or(|word| help::topic(word).is_none())
}

fn mobile(arguments: &[OsString]) -> io::Result<Mobile> {
    let text: Vec<_> = arguments.iter().map(|argument| argument.to_str()).collect();
    match text.as_slice() {
        [] => Ok(Mobile::Status),
        [Some("enable")] => Ok(Mobile::Enable { port: None }),
        [Some("enable"), Some("--port"), Some(port)] => Ok(Mobile::Enable {
            port: Some(
                port.parse()
                    .map_err(|_| invalid("port must be a number from 1024 to 65535"))?,
            ),
        }),
        [Some("disable")] => Ok(Mobile::Disable),
        [Some("pair"), options @ ..] => pair(options),
        [Some("revoke"), Some(device)] if !device.is_empty() => Ok(Mobile::Revoke {
            device: (*device).to_owned(),
        }),
        [Some("push"), Some("on")] => Ok(Mobile::Push { on: true }),
        [Some("push"), Some("off")] => Ok(Mobile::Push { on: false }),
        [Some("push"), Some("reset"), Some(device)] if !device.is_empty() => {
            Ok(Mobile::ResetPush {
                device: (*device).to_owned(),
            })
        }
        [Some("push"), ..] => Err(invalid("usage: muxy mobile push on|off|reset <device>")),
        _ => Err(invalid("unknown mobile command; run muxy mobile --help")),
    }
}

fn pair(options: &[Option<&str>]) -> io::Result<Mobile> {
    let mut addresses = Vec::new();
    for option in options.chunks(2) {
        let [Some("--address"), Some(address)] = option else {
            return Err(invalid("usage: muxy mobile pair [--address HOST]..."));
        };
        let address = (*address).to_owned();
        if muxy_protocol::validate_pairing_hosts(std::slice::from_ref(&address)).is_err() {
            return Err(invalid(&format!(
                "--address {address}: use a DNS name or an IPv4 address, without a port"
            )));
        }
        addresses.push(address);
    }
    if addresses.len() > MAX_PAIRING_HOSTS {
        return Err(invalid(&format!(
            "a pairing code holds at most {MAX_PAIRING_HOSTS} addresses"
        )));
    }
    Ok(Mobile::Pair { addresses })
}

fn install_skills(arguments: &[OsString]) -> io::Result<Vec<PathBuf>> {
    arguments
        .chunks(2)
        .map(|option| match option {
            [flag, directory] if flag == "--dir" && !directory.is_empty() => {
                Ok(PathBuf::from(directory))
            }
            _ => Err(invalid("usage: muxy install-skills [--dir DIR]...")),
        })
        .collect()
}

fn stdio(arguments: &[OsString]) -> io::Result<Start> {
    match arguments {
        [] => Ok(Start::IfNeeded),
        [flag] if flag == "--no-start" => Ok(Start::Never),
        _ => Err(invalid("usage: muxy stdio [--no-start]")),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> io::Result<(Option<SshTarget>, Command)> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn aliases_are_exactly_the_existing_session_commands() -> io::Result<()> {
        for (short, long, rest) in [
            ("ls", "list", vec!["--quiet"]),
            ("new", "create", vec!["Home", "--", "echo", "--help"]),
            ("send", "send", vec!["42", "--stdin"]),
            ("send-key", "send-keys", vec!["42", "Ctrl+C", "Enter"]),
            ("capture", "read-screen", vec!["42", "--saved"]),
            ("wait", "wait", vec!["42", "--text", "ready"]),
        ] {
            let mut alias = vec![short];
            alias.extend(&rest);
            let mut full = vec!["session", long];
            full.extend(&rest);
            assert_eq!(words(&alias)?.1, words(&full)?.1);
            assert!(!names_folder(OsStr::new(short)));
            assert!(matches!(
                words(&["help", short])?.1,
                Command::DetailedUsage(_)
            ));
        }
        assert!(matches!(words(&["./ls"])?.1, Command::Open(_)));
        assert!(matches!(
            words(&["--host", "devbox", "ls"])?,
            (Some(_), Command::Manage(_))
        ));
        Ok(())
    }

    #[test]
    fn group_help_and_completions_do_not_need_a_server() -> io::Result<()> {
        for group in [
            "project",
            "session",
            "server",
            "worktree",
            "settings",
            "activity",
            "completion",
        ] {
            assert!(matches!(words(&[group])?.1, Command::Usage(_)));
        }
        for shell in ["bash", "zsh", "fish"] {
            assert!(matches!(
                words(&["completion", shell])?.1,
                Command::Completion(_)
            ));
        }
        assert!(words(&["completion", "unknown"]).is_err());
        assert!(words(&["--host", "example", "completion", "bash"]).is_err());
        assert!(words(&["help", "session", "unknown"]).is_err());
        assert_eq!(words(&["version"])?.1, Command::Version);
        Ok(())
    }

    #[test]
    fn install_skills_takes_folders_but_no_host() -> io::Result<()> {
        assert_eq!(
            words(&["install-skills"])?.1,
            Command::InstallSkills(Vec::new())
        );
        assert_eq!(
            words(&["install-skills", "--dir", "a", "--dir", "b"])?.1,
            Command::InstallSkills(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            words(&["install-skills", "--help"])?.1,
            Command::Usage(help::INSTALL_SKILLS)
        );
        for args in [
            vec!["install-skills", "--dir"],
            vec!["install-skills", "a"],
            vec!["--host", "example", "install-skills"],
        ] {
            assert!(words(&args).is_err(), "{args:?}");
        }
        Ok(())
    }

    #[test]
    fn status_reports_stay_on_this_computer_and_have_help() -> io::Result<()> {
        assert_eq!(
            words(&["status", "working"])?.1,
            Command::Status("\x1b]7501;state=working\x1b\\".into())
        );
        assert_eq!(
            words(&["status", "--help"])?.1,
            Command::Usage(help::STATUS)
        );
        assert_eq!(
            words(&["status", "working", "--help"])?.1,
            Command::Usage(help::STATUS)
        );
        assert!(matches!(
            words(&["status", "working", "--msg", "--help"])?.1,
            Command::Status(_)
        ));
        assert_eq!(words(&["help", "status"])?.1, Command::Usage(help::STATUS));
        assert!(!names_folder(OsStr::new("status")));
        assert!(words(&["--host", "example", "status", "working"]).is_err());
        Ok(())
    }

    #[test]
    fn push_notifications_turn_on_and_off_from_mobile() -> io::Result<()> {
        assert_eq!(
            words(&["mobile", "push", "on"])?.1,
            Command::Mobile(Mobile::Push { on: true })
        );
        assert_eq!(
            words(&["mobile", "push", "off"])?.1,
            Command::Mobile(Mobile::Push { on: false })
        );
        assert!(matches!(
            words(&["--host", "box", "mobile", "push", "off"])?,
            (Some(_), Command::Mobile(Mobile::Push { on: false }))
        ));
        assert_eq!(
            words(&["mobile", "push", "reset", "3f2a"])?.1,
            Command::Mobile(Mobile::ResetPush {
                device: "3f2a".into()
            })
        );
        for args in [
            vec!["mobile", "push"],
            vec!["mobile", "push", "maybe"],
            vec!["mobile", "push", "on", "now"],
            vec!["mobile", "push", "reset"],
            vec!["mobile", "push", "reset", ""],
        ] {
            assert!(words(&args).is_err(), "{args:?}");
        }
        Ok(())
    }

    #[test]
    fn browser_commands_stay_on_this_computer_and_have_help() -> io::Result<()> {
        assert!(matches!(
            words(&["browser", "list"])?.1,
            Command::Browser(_)
        ));
        assert_eq!(words(&["browser"])?.1, Command::Usage(help::BROWSER));
        assert_eq!(
            words(&["help", "browser"])?.1,
            Command::Usage(help::BROWSER)
        );
        assert_eq!(words(&["help"])?.1, Command::Help);
        assert!(!names_folder(OsStr::new("browser")));
        for args in [
            vec!["--host", "example", "browser", "list"],
            vec!["help", "nothing"],
        ] {
            assert!(words(&args).is_err(), "{args:?}");
        }
        Ok(())
    }
}
