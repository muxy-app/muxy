use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Help,
    Version,
    BuildInfo,
    Interactive,
    Projects,
    AddProject {
        directory: PathBuf,
        name: Option<String>,
    },
    Mobile(Mobile),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Mobile {
    Status,
    Enable { port: Option<u16> },
    Disable,
    Pair,
    Revoke { device: String },
}

pub(crate) fn parse(arguments: &[OsString]) -> io::Result<Command> {
    match arguments {
        [flag] if flag == "--help" || flag == "-h" => return Ok(Command::Help),
        [flag] if flag == "--version" || flag == "-V" => return Ok(Command::Version),
        [flag] if flag == "--build-info" => return Ok(Command::BuildInfo),
        _ => {}
    }
    match arguments {
        [] => Ok(Command::Interactive),
        [command, action] if command == "project" && action == "list" => Ok(Command::Projects),
        [command, action, directory] if command == "project" && action == "add" => {
            Ok(Command::AddProject {
                directory: directory.into(),
                name: None,
            })
        }
        [command, action, directory, flag, name]
            if command == "project" && action == "add" && flag == "--name" =>
        {
            let name = name
                .to_str()
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| invalid("project name must be nonempty UTF-8"))?;
            Ok(Command::AddProject {
                directory: directory.into(),
                name: Some(name.trim().into()),
            })
        }
        [command, rest @ ..] if command == "mobile" => mobile(rest).map(Command::Mobile),
        _ => Err(invalid("unknown command or arguments; run muxy --help")),
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

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &[&str]) -> io::Result<Command> {
        parse(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn mobile_commands_parse_and_reject_malformed_input() -> io::Result<()> {
        assert_eq!(parse_words(&["mobile"])?, Command::Mobile(Mobile::Status));
        assert_eq!(
            parse_words(&["mobile", "enable", "--port", "7420"])?,
            Command::Mobile(Mobile::Enable { port: Some(7420) })
        );
        assert_eq!(
            parse_words(&["mobile", "revoke", "3f2a"])?,
            Command::Mobile(Mobile::Revoke {
                device: "3f2a".into()
            })
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
}
