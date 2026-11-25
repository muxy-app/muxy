use std::io;
use std::path::{Path, PathBuf};

use crate::agent_skills::{AGENT_SKILLS, refuse_links_on_the_way, write_if_changed};

pub const SKILL_NAME: &str = "muxy-settings";
pub const SKILL: &str = include_str!("../../skills/muxy-settings/SKILL.md");

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

pub fn skill_file(profile: &Path) -> PathBuf {
    profile
        .join(AGENT_SKILLS[1])
        .join(SKILL_NAME)
        .join("SKILL.md")
}

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

    #[test]
    fn linked_settings_skill_folders_are_refused_without_changing_the_target() -> io::Result<()> {
        for relative in [
            ".claude",
            ".claude/skills",
            ".claude/skills/muxy-settings",
            ".claude/skills/muxy-settings/references",
            ".agents/skills",
            ".agents/skills/muxy-settings",
            ".agents/skills/muxy-settings/references",
        ] {
            let directory = tempfile::tempdir()?;
            let profile = directory.path().join("profile");
            let target = directory.path().join("target");
            std::fs::create_dir_all(&target)?;
            for name in ["SKILL.md", "settings.md"] {
                std::fs::write(target.join(name), "keep this file")?;
            }
            let link: PathBuf = profile.join(relative).components().collect();
            std::fs::create_dir_all(link.parent().unwrap_or(&profile))?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &link)?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(&target, &link)?;

            let error = prepare(&profile).err().ok_or_else(|| {
                io::Error::other(format!("accepted linked skill folder {relative}"))
            })?;
            assert_eq!(
                error.to_string(),
                format!(
                    "{} is a link; Muxy keeps its skill in real folders",
                    link.display()
                )
            );
            for name in ["SKILL.md", "settings.md"] {
                assert_eq!(
                    std::fs::read_to_string(target.join(name))?,
                    "keep this file"
                );
            }
            assert_eq!(std::fs::read_dir(&target)?.count(), 2);
        }
        Ok(())
    }

    #[test]
    fn preparing_settings_keeps_unchanged_files_and_refreshes_stale_ones() -> io::Result<()> {
        let profile = tempfile::tempdir()?;
        prepare(profile.path())?;
        let skill = skill_file(profile.path());
        let file = std::fs::OpenOptions::new().write(true).open(&skill)?;
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))?;
        drop(file);
        let modified = std::fs::metadata(&skill)?.modified()?;
        let reference = skill
            .parent()
            .unwrap_or(profile.path())
            .join("references/settings.md");
        std::fs::write(&reference, "from an older Muxy")?;

        prepare(profile.path())?;

        assert_eq!(std::fs::metadata(&skill)?.modified()?, modified);
        assert_eq!(std::fs::read_to_string(reference)?, FILES[1].1);
        Ok(())
    }
}
