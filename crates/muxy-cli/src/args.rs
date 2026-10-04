use std::ffi::OsString;
use std::io;

use muxy_client::{SshTarget, Start};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Help,
    Version,
    BuildInfo,
    Interactive,
    Manage(Box<crate::manage::args::Invocation>),
    Mobile(Mobile),
    /// Joins stdin and stdout to the server, for clients on other computers.
    Stdio(Start),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Mobile {
    Status,
    Enable { port: Option<u16> },
    Disable,
    Pair,
    Revoke { device: String },
}

/// Reads the command, and the other computer whose server it uses when it
/// starts with `--host DESTINATION`.
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
                // These describe this computer's muxy, and a bridge must never
                // reach on to another computer.
                (
                    Command::Help | Command::Version | Command::BuildInfo | Command::Stdio(_),
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
    match arguments {
        [flag] if flag == "--help" || flag == "-h" => return Ok(Command::Help),
        [flag] if flag == "--version" || flag == "-V" => return Ok(Command::Version),
        [flag] if flag == "--build-info" => return Ok(Command::BuildInfo),
        _ => {}
    }
    match arguments {
        [] => Ok(Command::Interactive),
        [command, rest @ ..] if command == "mobile" => mobile(rest).map(Command::Mobile),
        [command, rest @ ..] if command == "stdio" => stdio(rest).map(Command::Stdio),
        _ => {
            crate::manage::args::parse(arguments).map(|command| Command::Manage(Box::new(command)))
        }
    }
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
        [Some("pair")] => Ok(Mobile::Pair),
        [Some("revoke"), Some(device)] if !device.is_empty() => Ok(Mobile::Revoke {
            device: (*device).to_owned(),
        }),
        _ => Err(invalid("unknown mobile command; run muxy --help")),
    }
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
    use crate::manage::args::Action;

    fn parse_words(words: &[&str]) -> io::Result<(Option<SshTarget>, Command)> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn mobile_commands_parse_and_reject_malformed_input() -> io::Result<()> {
        assert_eq!(
            parse_words(&["mobile"])?,
            (None, Command::Mobile(Mobile::Status))
        );
        assert_eq!(
            parse_words(&["mobile", "enable", "--port", "7420"])?,
            (None, Command::Mobile(Mobile::Enable { port: Some(7420) }))
        );
        assert_eq!(
            parse_words(&["mobile", "revoke", "3f2a"])?,
            (
                None,
                Command::Mobile(Mobile::Revoke {
                    device: "3f2a".into()
                })
            )
        );
        for words in [
            &["mobile", "enable", "--port", "70000"][..],
            &["mobile", "enable", "--port"],
            &["mobile", "revoke"],
            &["mobile", "pair", "now"],
            &["mobile", "unknown"],
        ] {
            assert!(parse_words(words).is_err(), "{words:?}");
        }
        Ok(())
    }

    #[test]
    fn stdio_starts_the_server_unless_told_not_to() -> io::Result<()> {
        assert_eq!(
            parse_words(&["stdio"])?,
            (None, Command::Stdio(Start::IfNeeded))
        );
        assert_eq!(
            parse_words(&["stdio", "--no-start"])?,
            (None, Command::Stdio(Start::Never))
        );
        for words in [
            &["stdio", "--help"][..],
            &["stdio", "--no-start", "--no-start"],
            &["stdio", "now"],
        ] {
            assert!(parse_words(words).is_err(), "{words:?}");
        }
        Ok(())
    }

    #[test]
    fn host_comes_first_and_applies_to_the_tui_and_server_commands() -> io::Result<()> {
        let host = SshTarget::new("dev@box")?;
        assert_eq!(
            parse_words(&["--host", "dev@box"])?,
            (Some(host.clone()), Command::Interactive)
        );
        assert_eq!(
            parse_words(&["--host", "dev@box", "mobile", "pair"])?,
            (Some(host), Command::Mobile(Mobile::Pair))
        );
        let (host, command) =
            parse_words(&["--host", "ssh://dev@box:2222", "session", "list", "--json"])?;
        assert_eq!(host, Some(SshTarget::new("ssh://dev@box:2222")?));
        assert!(matches!(command, Command::Manage(invocation) if invocation.json));
        let (_, help) = parse_words(&["--host", "box", "session", "--help"])?;
        assert!(
            matches!(help, Command::Manage(invocation) if matches!(invocation.action, Action::Help(_)))
        );
        Ok(())
    }

    #[test]
    fn host_must_be_valid_and_never_reaches_this_muxy_or_its_bridge() {
        for words in [
            &["--host"][..],
            &["--host", ""],
            &["--host", "-oProxyCommand=sh"],
            &["--host", "dev box"],
            &["--host", "box", "--help"],
            &["--host", "box", "-h"],
            &["--host", "box", "--version"],
            &["--host", "box", "--build-info"],
            &["--host", "box", "stdio"],
            &["--host", "box", "stdio", "--no-start"],
            &["--host", "box", "--host", "other"],
            &["session", "list", "--host", "box"],
        ] {
            assert!(parse_words(words).is_err(), "{words:?}");
        }
    }
}
