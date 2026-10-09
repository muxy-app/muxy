//! The muxy-cli skill, which teaches AI coding agents to use this command,
//! and installing it where agents look for skills.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SKILL: &str = include_str!("../skills/muxy-cli/SKILL.md");

/// Agent folders in the home folder; each keeps its skills in `skills`.
const AGENTS: [&str; 3] = [".claude", ".codex", ".agents"];

/// Writes the skill into every agent folder in `home` and into each of
/// `directories`, replacing an older copy, and returns the files written.
pub(crate) fn install(home: &Path, directories: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut roots: Vec<PathBuf> = AGENTS
        .iter()
        .map(|agent| home.join(agent))
        .filter(|agent| agent.is_dir())
        .map(|agent| agent.join("skills"))
        .collect();
    for directory in directories {
        roots.push(std::path::absolute(directory)?);
    }
    if roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "found no AI agent folder (~/.claude, ~/.codex or ~/.agents); choose one with --dir",
        ));
    }
    roots
        .into_iter()
        .map(|root| {
            let folder = root.join("muxy-cli");
            fs::create_dir_all(&folder)?;
            let file = folder.join("SKILL.md");
            fs::write(&file, SKILL)?;
            Ok(file)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_into_agent_folders_that_exist_and_chosen_folders() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        assert!(install(home.path(), &[]).is_err());
        fs::create_dir(home.path().join(".claude"))?;
        let chosen = home.path().join("chosen");
        let written = install(home.path(), std::slice::from_ref(&chosen))?;
        assert_eq!(
            written,
            [
                home.path().join(".claude/skills/muxy-cli/SKILL.md"),
                chosen.join("muxy-cli/SKILL.md"),
            ]
        );
        assert!(!home.path().join(".codex").exists());
        assert_eq!(fs::read_to_string(&written[0])?, SKILL);
        Ok(())
    }

    /// Splits a shell command line into words, keeping quoted text together
    /// and standing in a session ID for each `$VARIABLE`.
    fn words(line: &str) -> Vec<std::ffi::OsString> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quoted = false;
        for character in line.chars() {
            match character {
                '"' => quoted = !quoted,
                ' ' if !quoted => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                _ => word.push(character),
            }
        }
        words.push(word);
        words
            .into_iter()
            .filter(|word| !word.is_empty())
            .map(|word| {
                if word.starts_with('$') {
                    "7".into()
                } else {
                    word.into()
                }
            })
            .collect()
    }

    #[test]
    fn every_command_in_the_skill_parses() {
        assert!(SKILL.starts_with("---\nname: muxy-cli\n"));
        let commands: Vec<_> = SKILL
            .lines()
            .filter_map(|line| line.split_once("muxy ").map(|(_, command)| command))
            .filter(|command| !command.starts_with("<command>") && !command.starts_with("--help"))
            .map(|command| {
                let command = command.split("  #").next().unwrap_or(command);
                command.trim_end().trim_end_matches(')')
            })
            .filter(|command| !command.contains('`'))
            .collect();
        assert!(commands.len() > 15, "{commands:?}");
        for command in commands {
            assert!(
                crate::args::parse(&words(command)).is_ok(),
                "muxy {command}"
            );
        }
    }
}
