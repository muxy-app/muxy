//! The folder where agents build extensions: the `muxy-extensions` skill for
//! each agent, and one folder per extension under `extensions/`, the layout of
//! the published extensions repository.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// Startup and starting an agent can both prepare a workspace, and cleaning up
/// would delete the other's half-written files.
static PREPARING: Mutex<()> = Mutex::new(());

pub const SKILL_NAME: &str = "muxy-extensions";
pub const SKILL: &str = include_str!("../../skills/muxy-extensions/SKILL.md");

const AGENT_SKILLS: [&str; 2] = [".claude/skills", ".agents/skills"];
const POINTER: &str = include_str!("../../skills/muxy-extensions/POINTER.md");
const POINTERS: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
const FILES: &[(&str, &str)] = &[
    ("SKILL.md", SKILL),
    (
        "features/ai-agents.md",
        include_str!("../../../../docs/features/ai-agents.md"),
    ),
    (
        "features/muxy-cli.md",
        include_str!("../../../../docs/features/muxy-cli.md"),
    ),
    (
        "references/agents.md",
        include_str!("../../../../docs/extensions/agents.md"),
    ),
    (
        "references/api.md",
        include_str!("../../../../docs/extensions/api.md"),
    ),
    (
        "references/bar-items.md",
        include_str!("../../../../docs/extensions/bar-items.md"),
    ),
    (
        "references/commands.md",
        include_str!("../../../../docs/extensions/commands.md"),
    ),
    (
        "references/contributing.md",
        include_str!("../../../../docs/extensions/contributing.md"),
    ),
    (
        "references/dialogs.md",
        include_str!("../../../../docs/extensions/dialogs.md"),
    ),
    (
        "references/events.md",
        include_str!("../../../../docs/extensions/events.md"),
    ),
    (
        "references/files.md",
        include_str!("../../../../docs/extensions/files.md"),
    ),
    (
        "references/get-started.md",
        include_str!("../../../../docs/extensions/get-started.md"),
    ),
    (
        "references/git.md",
        include_str!("../../../../docs/extensions/git.md"),
    ),
    (
        "references/localizations.md",
        include_str!("../../../../docs/extensions/localizations.md"),
    ),
    (
        "references/manifest.md",
        include_str!("../../../../docs/extensions/manifest.md"),
    ),
    (
        "references/overview.md",
        include_str!("../../../../docs/extensions/overview.md"),
    ),
    (
        "references/panels.md",
        include_str!("../../../../docs/extensions/panels.md"),
    ),
    (
        "references/permissions.md",
        include_str!("../../../../docs/extensions/permissions.md"),
    ),
    (
        "references/tabs.md",
        include_str!("../../../../docs/extensions/tabs.md"),
    ),
    (
        "references/schema/manifest.schema.json",
        include_str!("../../../../docs/extensions/schema/manifest.schema.json"),
    ),
];

/// Where one extension of the workspace lives.
pub fn extension_folder(root: &Path, name: &str) -> PathBuf {
    root.join("extensions").join(name)
}

/// The skill's instructions, which a prompt can point an agent at from any folder.
pub fn skill_file(root: &Path) -> PathBuf {
    root.join(AGENT_SKILLS[1]).join(SKILL_NAME).join("SKILL.md")
}

/// Creates the workspace if needed and brings its skill up to date with this
/// build. Only Muxy's own skill and pointer files are touched.
pub fn prepare(root: &Path) -> io::Result<()> {
    let _preparing = PREPARING.lock().unwrap_or_else(PoisonError::into_inner);
    fs::create_dir_all(root.join("extensions"))?;
    for skills in AGENT_SKILLS {
        let folder = root.join(skills).join(SKILL_NAME);
        refuse_links_on_the_way(root, &format!("{skills}/{SKILL_NAME}/SKILL.md"))?;
        for (relative, contents) in FILES {
            refuse_links_on_the_way(&folder, relative)?;
            write_if_changed(&folder.join(relative), contents)?;
        }
        remove_others(&folder, &folder)?;
    }
    for pointer in POINTERS {
        write_if_changed(&root.join(pointer), POINTER)?;
    }
    Ok(())
}

/// Updates a workspace the user already has after an app update. One that was
/// never created stays uncreated until the first extension needs it.
pub fn refresh(root: &Path) -> io::Result<()> {
    if root.is_dir() { prepare(root) } else { Ok(()) }
}

/// A folder on the way to a skill file that is a link would put Muxy's files, and
/// its cleanup, somewhere else. Above the workspace links are fine: `~/.config` is
/// often one.
fn refuse_links_on_the_way(folder: &Path, relative: &str) -> io::Result<()> {
    let mut path = folder.to_owned();
    for part in Path::new(relative)
        .parent()
        .into_iter()
        .flat_map(Path::components)
    {
        path.push(part);
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
            return Err(io::Error::other(format!(
                "{} is a link; Muxy keeps its skill in real folders",
                path.display()
            )));
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, contents: &str) -> io::Result<()> {
    if fs::read(path).is_ok_and(|current| current == contents.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    muxy_core::fs::write_atomic(path, contents.as_bytes(), 0o644)?;
    Ok(())
}

/// Removes what an earlier version of the skill left behind.
fn remove_others(root: &Path, folder: &Path) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if fs::symlink_metadata(&path)?.is_dir() {
            remove_others(root, &path)?;
            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)?;
            }
            continue;
        }
        let known = path
            .strip_prefix(root)
            .is_ok_and(|relative| FILES.iter().any(|(file, _)| relative == Path::new(file)));
        if !known {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_the_skill_for_each_agent_and_a_pointer_for_the_others() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().join("workspace");
        prepare(&root)?;
        assert!(root.join("extensions").is_dir());
        for skills in AGENT_SKILLS {
            let skill = root.join(skills).join(SKILL_NAME);
            assert_eq!(fs::read_to_string(skill.join("SKILL.md"))?, SKILL);
            assert!(skill.join("references/manifest.md").is_file());
            assert!(
                skill
                    .join("references/schema/manifest.schema.json")
                    .is_file()
            );
        }
        for pointer in POINTERS {
            assert_eq!(fs::read_to_string(root.join(pointer))?, POINTER);
        }
        assert_eq!(
            skill_file(&root),
            root.join(".agents/skills/muxy-extensions/SKILL.md")
        );
        Ok(())
    }

    #[test]
    fn refresh_replaces_muxys_skill_and_keeps_everything_else() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().join("workspace");
        prepare(&root)?;
        let skill = root.join(AGENT_SKILLS[0]).join(SKILL_NAME);
        fs::write(skill.join("SKILL.md"), "from an older Muxy")?;
        fs::write(skill.join("references/removed.md"), "gone from this build")?;
        fs::create_dir_all(skill.join("references/old"))?;
        fs::write(skill.join("references/old/page.md"), "gone too")?;
        let other = root.join(AGENT_SKILLS[0]).join("mine/SKILL.md");
        fs::create_dir_all(other.parent().unwrap_or(&root))?;
        fs::write(&other, "the user's own skill")?;
        let work = extension_folder(&root, "hello").join("package.json");
        fs::create_dir_all(work.parent().unwrap_or(&root))?;
        fs::write(&work, "{}")?;

        refresh(&root)?;

        assert_eq!(fs::read_to_string(skill.join("SKILL.md"))?, SKILL);
        assert!(!skill.join("references/removed.md").exists());
        assert!(!skill.join("references/old").exists());
        assert_eq!(fs::read_to_string(other)?, "the user's own skill");
        assert_eq!(fs::read_to_string(work)?, "{}");
        Ok(())
    }

    #[test]
    fn a_linked_skill_folder_is_refused_and_what_it_points_at_is_kept() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let elsewhere = folder.path().join("elsewhere");
        fs::create_dir_all(&elsewhere)?;
        fs::write(elsewhere.join("mine.txt"), "kept")?;
        let root = folder.path().join("workspace");
        let skills = root.join(AGENT_SKILLS[0]);
        fs::create_dir_all(&skills)?;
        std::os::unix::fs::symlink(&elsewhere, skills.join(SKILL_NAME))?;
        assert!(prepare(&root).is_err());
        assert_eq!(fs::read_to_string(elsewhere.join("mine.txt"))?, "kept");
        assert!(!elsewhere.join("SKILL.md").exists());
        Ok(())
    }

    #[test]
    fn a_linked_folder_above_the_skill_is_refused() -> io::Result<()> {
        for link in [".claude", ".claude/skills", ".agents/skills"] {
            let folder = tempfile::tempdir()?;
            let elsewhere = folder.path().join("elsewhere");
            let theirs = elsewhere.join(SKILL_NAME);
            fs::create_dir_all(&theirs)?;
            fs::write(theirs.join("mine.txt"), "kept")?;
            let root = folder.path().join("workspace");
            let link = root.join(link);
            fs::create_dir_all(link.parent().unwrap_or(&root))?;
            std::os::unix::fs::symlink(&elsewhere, &link)?;
            assert!(prepare(&root).is_err(), "{}", link.display());
            assert_eq!(fs::read_to_string(theirs.join("mine.txt"))?, "kept");
        }
        Ok(())
    }

    #[test]
    fn a_linked_workspace_or_folder_above_it_still_works() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let real = folder.path().join("dotfiles");
        fs::create_dir_all(&real)?;
        let config = folder.path().join("config");
        std::os::unix::fs::symlink(&real, &config)?;
        let root = config.join("workspace");
        prepare(&root)?;
        assert!(
            real.join("workspace")
                .join(AGENT_SKILLS[0])
                .join(SKILL_NAME)
                .is_dir()
        );
        Ok(())
    }

    #[test]
    fn a_linked_folder_inside_the_skill_is_refused() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let elsewhere = folder.path().join("elsewhere");
        fs::create_dir_all(&elsewhere)?;
        let root = folder.path().join("workspace");
        let skill = root.join(AGENT_SKILLS[0]).join(SKILL_NAME);
        fs::create_dir_all(&skill)?;
        std::os::unix::fs::symlink(&elsewhere, skill.join("references"))?;
        assert!(prepare(&root).is_err());
        assert!(
            fs::read_dir(&elsewhere)?.next().is_none(),
            "nothing written"
        );
        Ok(())
    }

    #[test]
    fn refresh_does_not_create_a_workspace_nobody_asked_for() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().join("workspace");
        refresh(&root)?;
        assert!(!root.exists());
        Ok(())
    }

    #[test]
    fn an_unchanged_skill_is_not_written_again() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().join("workspace");
        prepare(&root)?;
        let file = root.join(POINTERS[0]);
        let written = fs::metadata(&file)?.modified()?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        prepare(&root)?;
        assert_eq!(fs::metadata(&file)?.modified()?, written);
        Ok(())
    }
}
