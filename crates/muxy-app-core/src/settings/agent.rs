//! The `muxy-settings` skill, which an agent started in the profile folder
//! reads to change the app's settings with `muxy config`.

use std::io;
use std::path::{Path, PathBuf};

use crate::extensions::workspace::{refuse_links_on_the_way, write_if_changed};

pub const SKILL_NAME: &str = "muxy-settings";
pub const SKILL: &str = include_str!("../../skills/muxy-settings/SKILL.md");

const AGENT_SKILLS: [&str; 2] = [".claude/skills", ".agents/skills"];
const FILES: &[(&str, &str)] = &[
    ("SKILL.md", SKILL),
    (
        "references/settings.md",
        include_str!("../../../../docs/user-guide/settings.md"),
    ),
    (
        "references/keyboard-shortcuts.md",
        include_str!("../../../../docs/user-guide/keyboard-shortcuts.md"),
    ),
];

/// The skill's instructions, which a prompt points the agent at.
pub fn skill_file(profile: &Path) -> PathBuf {
    profile
        .join(AGENT_SKILLS[1])
        .join(SKILL_NAME)
        .join("SKILL.md")
}

/// Writes the skill for each agent into the profile folder, leaving
/// everything else there alone.
pub fn prepare(profile: &Path) -> io::Result<()> {
    for skills in AGENT_SKILLS {
        let folder = profile.join(skills).join(SKILL_NAME);
        for (relative, contents) in FILES {
            refuse_links_on_the_way(profile, &format!("{skills}/{SKILL_NAME}/{relative}"))?;
            write_if_changed(&folder.join(relative), contents)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_is_written_for_each_agent_and_nothing_else_changes() -> io::Result<()> {
        let profile = tempfile::tempdir()?;
        std::fs::write(profile.path().join("settings.toml"), "keep = true\n")?;
        prepare(profile.path())?;
        prepare(profile.path())?;
        assert_eq!(std::fs::read_to_string(skill_file(profile.path()))?, SKILL);
        assert_eq!(
            std::fs::read_to_string(
                profile
                    .path()
                    .join(".claude/skills/muxy-settings/references/settings.md")
            )?,
            FILES[1].1
        );
        assert_eq!(
            std::fs::read_to_string(profile.path().join("settings.toml"))?,
            "keep = true\n"
        );
        Ok(())
    }
}
