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
    Version,
    BuildInfo,
    Interactive,
    Manage(Box<crate::manage::args::Invocation>),
    Mobile(Mobile),
    /// Joins stdin and stdout to the server, for clients on other computers.
    Stdio(Start),
    Open(PathBuf),
    /// Installs the agent skill into agent folders and these folders.
    InstallSkills(Vec<PathBuf>),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Mobile {
    Status,
    Enable { port: Option<u16> },
    Disable,
    Pair { addresses: Vec<String> },
    Revoke { device: String },
}

/// Reads the command, and the other computer whose server it uses when it
/// starts with `--host DESTINATION`. Help, version and build info describe
/// this computer's Muxy, and a bridge must never reach on to another
/// computer, so none of them takes `--host`.
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
                    | Command::Stdio(_)
                    | Command::Open(_)
                    | Command::InstallSkills(_),
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
        [word] if names_folder(word) => Ok(Command::Open(word.into())),
        _ => {
            crate::manage::args::parse(arguments).map(|command| Command::Manage(Box::new(command)))
        }
    }
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
}
