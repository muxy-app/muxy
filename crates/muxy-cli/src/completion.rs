use std::ffi::OsString;
use std::io::{self, Write};

use crate::manage::help;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub(crate) fn parse(name: &str) -> io::Result<Self> {
        match name {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "choose a shell: bash, zsh or fish",
            )),
        }
    }

    pub(crate) fn script(&self) -> &'static str {
        match self {
            Self::Bash => {
                "_muxy_complete() {
    local candidate
    COMPREPLY=()
    while IFS= read -r candidate; do
        COMPREPLY+=(\"$candidate\")
    done < <(muxy __complete \"${COMP_WORDS[@]:1:COMP_CWORD}\")
}
complete -o default -F _muxy_complete muxy
"
            }
            Self::Zsh => {
                "#compdef muxy
_muxy() {
    local -a candidates
    candidates=(\"${(@f)$(muxy __complete \"${words[@]:1:$((CURRENT - 1))}\")}\")
    if [[ -n $candidates[1] ]]; then
        compadd -- \"${candidates[@]}\"
    else
        _files
    fi
}
compdef _muxy muxy
"
            }
            Self::Fish => {
                "function __muxy_complete
    muxy __complete (commandline -opc)[2..-1] (commandline -ct)
end
complete -c muxy -a '(__muxy_complete)'
"
            }
        }
    }
}

pub(crate) fn complete(words: &[OsString]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    for candidate in candidates(words) {
        writeln!(stdout, "{candidate}")?;
    }
    Ok(())
}

fn candidates(words: &[OsString]) -> Vec<String> {
    let Some((current, before)) = words.split_last() else {
        return help::children("");
    };
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let mut path = String::new();
    let mut skip = false;
    let mut positional = false;
    let mut helping = false;
    for word in before {
        if skip {
            skip = false;
            continue;
        }
        let Some(word) = word.to_str() else {
            positional = true;
            continue;
        };
        if word == "--" {
            return Vec::new();
        }
        if word == "--host" && path.is_empty() {
            skip = true;
        } else if word == "help" && path.is_empty() {
            helping = true;
        } else if word.starts_with('-') {
            skip = help::takes_value(&path, word);
        } else if !positional {
            if path.is_empty() {
                path = help::alias(word).unwrap_or(word).into();
            } else {
                let next = format!("{path} {word}");
                if help::known(&next) {
                    path = next;
                } else {
                    positional = true;
                }
            }
        }
    }
    if skip {
        return Vec::new();
    }
    let mut candidates = if positional {
        Vec::new()
    } else {
        help::children(&path)
    };
    if !helping {
        candidates.extend(help::flags(&path));
    }
    candidates.retain(|candidate| candidate.starts_with(current));
    candidates.sort();
    candidates.dedup();
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestions(words: &[&str]) -> Vec<String> {
        candidates(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn completes_commands_aliases_nested_commands_and_options() {
        assert!(suggestions(&[""]).contains(&"capture".into()));
        assert!(suggestions(&["session", "in"]).contains(&"inspect".into()));
        assert_eq!(suggestions(&["browser", "cookies", "s"]), vec!["set"]);
        assert_eq!(suggestions(&["send", "42", "--st"]), vec!["--stdin"]);
        assert!(suggestions(&["--host", "devbox", "ls", "--"]).contains(&"--quiet".into()));
        assert_eq!(suggestions(&["completion", "z"]), vec!["zsh"]);
        assert_eq!(suggestions(&["help", "session", "cr"]), vec!["create"]);
    }

    #[test]
    fn never_completes_inside_values_or_literal_commands() {
        for words in [
            vec!["--host", ""],
            vec!["new", "--directory", ""],
            vec!["new", "Home", "--", ""],
            vec!["exec", "Home", "--", "echo", ""],
            vec!["session", "wait", "--text", "--"],
        ] {
            assert!(suggestions(&words).is_empty(), "{words:?}");
        }
    }
}
