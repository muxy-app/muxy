use std::ffi::OsString;
use std::fmt::Write as _;

use super::commands::COMMANDS;

pub(crate) const ALIASES: &[(&str, &str)] = &[
    ("ls", "session list"),
    ("new", "session create"),
    ("send", "session send"),
    ("send-key", "session send-keys"),
    ("capture", "session read-screen"),
    ("wait", "session wait"),
];

pub(crate) fn alias(word: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find_map(|(name, path)| (*name == word).then_some(*path))
}

pub(crate) fn known(path: &str) -> bool {
    COMMANDS
        .iter()
        .any(|(command, _, _)| *command == path || command.starts_with(&format!("{path} ")))
}

pub(crate) fn children(path: &str) -> Vec<String> {
    if path == "completion" {
        return ["bash", "zsh", "fish"].map(str::to_owned).to_vec();
    }
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path} ")
    };
    let mut names: Vec<_> = COMMANDS
        .iter()
        .filter_map(|(command, _, _)| {
            command
                .strip_prefix(&prefix)?
                .split_whitespace()
                .next()
                .map(str::to_owned)
        })
        .collect();
    if path.is_empty() {
        names.push("help".into());
        names.extend(ALIASES.iter().map(|(name, _)| (*name).into()));
    }
    names.sort();
    names.dedup();
    names
}

fn arguments(path: &str) -> &'static str {
    COMMANDS
        .iter()
        .find_map(|(command, args, _)| (*command == path).then_some(*args))
        .unwrap_or_default()
}

fn tokens(path: &str) -> Vec<&'static str> {
    arguments(path)
        .split_whitespace()
        .map(|word| word.trim_matches(['[', ']', '(', ')']))
        .collect()
}

pub(crate) fn takes_value(path: &str, flag: &str) -> bool {
    tokens(path).windows(2).any(|pair| {
        pair[0] == flag
            && pair[1]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
    })
}

pub(crate) fn flags(path: &str) -> Vec<String> {
    if path.is_empty() {
        return ["--host", "--help", "--version", "--build-info"]
            .map(str::to_owned)
            .to_vec();
    }
    if !known(path) {
        return Vec::new();
    }
    let mut flags: Vec<_> = tokens(path)
        .into_iter()
        .filter(|word| word.starts_with("--") && *word != "--")
        .map(str::to_owned)
        .collect();
    flags.push("--help".into());
    if flags.iter().any(|flag| flag == "--quiet") {
        flags.push("-q".into());
    }
    flags.sort();
    flags.dedup();
    flags
}

pub(crate) fn detail(path: &str) -> Option<String> {
    let path = alias(path).unwrap_or(path);
    if let Some((_, args, description)) = COMMANDS.iter().find(|(command, _, _)| *command == path) {
        let mut text = format!("Usage: muxy {path} {args}\n\n{description}\n");
        if args.contains("--json") {
            text.push_str("\nOutput:\n  --json         Machine-readable output with full IDs\n");
            if args.contains("--quiet") {
                text.push_str("  -q, --quiet    IDs only; cannot be combined with --json\n");
            }
            text.push_str("  Pipes retain their original output format.\n");
        }
        if path.starts_with("browser ")
            || path.starts_with("extension ")
            || path.starts_with("config ")
        {
            text.push_str("\nThis command uses the desktop app on this computer, not --host.\n");
        }
        if let Some(example) = example(path) {
            let _ = writeln!(text, "\nExamples:\n{example}");
        }
        let group = path.split_whitespace().next().unwrap_or(path);
        let _ = writeln!(
            text,
            "\nRun muxy help {group} for related commands and shared behavior."
        );
        return Some(text);
    }
    let children = children(path);
    if children.is_empty() {
        return None;
    }
    let mut text = format!("Usage: muxy {path} <command>\n\nCommands:\n");
    for child in children {
        let command = format!("{path} {child}");
        let description = COMMANDS
            .iter()
            .find_map(|(path, _, summary)| (*path == command).then_some(*summary))
            .unwrap_or_default();
        let _ = writeln!(text, "  {child:12} {description}");
    }
    Some(text)
}

fn example(path: &str) -> Option<&'static str> {
    Some(match path {
        "session create" => {
            "  muxy new app -- 'npm run dev'\n  id=$(muxy session create app -- 'exec npm test')"
        }
        "session inspect" => "  muxy session inspect\n  muxy session inspect 42 --json",
        "session rename" => "  muxy session rename 42 dev-server",
        "session send" => {
            "  muxy send 42 'npm test'\n  printf 'npm test\\n' | muxy send 42 --stdin"
        }
        "session send-keys" => "  muxy send-key 42 Ctrl+C Enter",
        "session read-screen" => {
            "  muxy capture 42 --lines 20\n  muxy capture 42 --saved > screen.txt"
        }
        "session wait" => "  muxy wait 42 --text ready --timeout-ms 120000\n  muxy wait 42 --exit",
        "session list" => "  muxy ls --project app\n  muxy ls --quiet\n  muxy ls --all --json",
        "project inspect" => "  muxy project inspect app --json",
        "project list" => "  muxy project list\n  muxy --host devbox project list --json",
        _ => return None,
    })
}

pub(crate) fn requested(arguments: &[OsString]) -> Option<String> {
    let mut path = String::new();
    let mut consumed = 0;
    for word in arguments {
        let Some(word) = word.to_str() else { break };
        let next = if path.is_empty() {
            word.to_owned()
        } else {
            format!("{path} {word}")
        };
        if !known(&next) {
            break;
        }
        path = next;
        consumed += 1;
    }
    if !path.contains(' ') {
        return None;
    }
    let mut words = arguments[consumed..].iter();
    while let Some(word) = words.next() {
        if word == "--" {
            return None;
        }
        if word == "--help" || word == "-h" {
            return detail(&path);
        }
        if word.to_str().is_some_and(|word| takes_value(&path, word)) {
            words.next();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{self, Command};

    #[test]
    fn every_documented_subcommand_has_specific_help_without_a_server() -> std::io::Result<()> {
        for (path, _, _) in COMMANDS {
            if !path.contains(' ') {
                continue;
            }
            let words: Vec<_> = path.split_whitespace().map(OsString::from).collect();
            let mut flag = words.clone();
            flag.push("--help".into());
            let mut named = vec![OsString::from("help")];
            named.extend(words);
            let expected = Command::DetailedUsage(detail(path).unwrap_or_default());
            assert_eq!(args::parse(&flag)?.1, expected, "{path}");
            assert_eq!(args::parse(&named)?.1, expected, "{path}");
        }
        Ok(())
    }

    #[test]
    fn help_does_not_consume_literals_or_option_values() {
        for words in [
            vec!["session", "create", "Home", "--", "echo", "--help"],
            vec!["session", "wait", "--text", "--help"],
            vec!["session", "send", "--", "--help"],
        ] {
            assert!(requested(&words.iter().map(OsString::from).collect::<Vec<_>>()).is_none());
        }
    }
}
