use muxy_core::prefs::{Prefs, home_dir};
use std::path::{Path, PathBuf};

pub const FINDER_BUNDLE_IDENTIFIER: &str = "com.apple.finder";
const SELECTED_KEY: &str = "muxy.ide.selectedBundleIdentifier";
const EDITOR: u8 = 0;
const OTHER_TOOL: u8 = 1;

const CURATED: [(&str, u32, u8); 30] = [
    ("com.microsoft.VSCode", 10, EDITOR),
    ("com.microsoft.VSCodeInsiders", 11, EDITOR),
    ("com.vscodium", 12, EDITOR),
    ("com.todesktop.230313mzl4w4u92", 13, EDITOR),
    ("dev.zed.Zed", 14, EDITOR),
    ("com.exafunction.windsurf", 15, EDITOR),
    ("com.qoder.ide", 16, EDITOR),
    ("com.apple.dt.Xcode", 17, EDITOR),
    ("com.jetbrains.PhpStorm", 18, EDITOR),
    ("com.jetbrains.WebStorm", 19, EDITOR),
    ("com.jetbrains.PyCharm", 20, EDITOR),
    ("com.jetbrains.IntelliJ-IDEA", 21, EDITOR),
    ("com.jetbrains.CLion", 22, EDITOR),
    ("com.jetbrains.GoLand", 23, EDITOR),
    ("com.jetbrains.RubyMine", 24, EDITOR),
    ("com.jetbrains.DataGrip", 25, EDITOR),
    ("com.jetbrains.Rider", 26, EDITOR),
    ("com.jetbrainsFleet", 27, EDITOR),
    ("com.panic.Nova", 28, EDITOR),
    ("com.sublimetext.4", 29, EDITOR),
    ("com.barebones.bbedit", 30, EDITOR),
    ("com.macromates.TextMate", 31, EDITOR),
    ("org.gnu.Emacs", 32, EDITOR),
    ("org.aquamacs.Aquamacs", 33, EDITOR),
    ("com.code.athas", 34, EDITOR),
    ("com.openai.codex", 80, OTHER_TOOL),
    ("ai.opencode.desktop", 81, OTHER_TOOL),
    ("com.google.antigravity-ide", 82, OTHER_TOOL),
    ("com.jetbrains.air", 84, OTHER_TOOL),
    (FINDER_BUNDLE_IDENTIFIER, 79, OTHER_TOOL),
];

const AI_COMPANIONS: [&str; 4] = [
    "com.openai.codex",
    "ai.opencode.desktop",
    "com.google.antigravity-ide",
    "com.jetbrains.air",
];

const CLI_COMMANDS: [(&str, &[&str]); 7] = [
    ("com.microsoft.VSCode", &["code"]),
    ("com.microsoft.VSCodeInsiders", &["code-insiders"]),
    ("com.todesktop.230313mzl4w4u92", &["cursor"]),
    ("com.exafunction.windsurf", &["windsurf"]),
    ("com.vscodium", &["codium", "vscodium"]),
    ("com.qoder.ide", &["qoder"]),
    ("dev.zed.Zed", &["zed"]),
];

#[derive(Debug, Clone)]
pub struct InstalledIde {
    pub bundle_identifier: String,
    pub display_name: String,
    pub path: PathBuf,
    pub rank: u32,
    pub group: u8,
}

pub fn finder() -> InstalledIde {
    InstalledIde {
        bundle_identifier: FINDER_BUNDLE_IDENTIFIER.to_owned(),
        display_name: "Finder".to_owned(),
        path: PathBuf::from("/System/Library/CoreServices/Finder.app"),
        rank: 79,
        group: OTHER_TOOL,
    }
}

pub fn installed() -> Vec<InstalledIde> {
    let mut found: Vec<InstalledIde> = Vec::new();
    for path in bundles() {
        let Some((identifier, name)) = metadata(&path) else {
            continue;
        };
        let Some((rank, group)) = admit(&identifier, &name) else {
            continue;
        };
        if found
            .iter()
            .any(|entry| entry.bundle_identifier == identifier)
        {
            continue;
        }
        found.push(InstalledIde {
            bundle_identifier: identifier,
            display_name: name,
            path,
            rank,
            group,
        });
    }
    found.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then_with(|| left.rank.cmp(&right.rank))
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    found
}

pub fn resolve(bundle_identifier: Option<&str>) -> Option<InstalledIde> {
    if bundle_identifier == Some(FINDER_BUNDLE_IDENTIFIER) {
        return Some(finder());
    }
    let installed = installed();
    if let Some(identifier) = bundle_identifier
        && let Some(entry) = installed
            .iter()
            .find(|entry| entry.bundle_identifier == identifier)
    {
        return Some(entry.clone());
    }
    installed.into_iter().next()
}

pub fn open_project(project_path: &str, ide: &InstalledIde) -> bool {
    let launched = if ide.bundle_identifier == FINDER_BUNDLE_IDENTIFIER {
        spawn("/usr/bin/open", &[project_path])
    } else if let Some(command) = cli_command(&ide.bundle_identifier) {
        spawn(&command, &[project_path])
    } else {
        spawn(
            "/usr/bin/open",
            &["-a", &ide.path.to_string_lossy(), project_path],
        )
    };
    if launched {
        Prefs::store_default(SELECTED_KEY, Some(&ide.bundle_identifier));
    }
    launched
}

pub fn display_name(bundle_identifier: &str) -> Option<String> {
    if bundle_identifier == FINDER_BUNDLE_IDENTIFIER {
        return Some("Finder".to_owned());
    }
    let bundle = locate_bundle(bundle_identifier)?;
    metadata(&bundle).map(|(_, name)| name)
}

fn admit(bundle_identifier: &str, display_name: &str) -> Option<(u32, u8)> {
    if let Some((_, rank, group)) = CURATED
        .iter()
        .find(|(identifier, _, _)| *identifier == bundle_identifier)
    {
        return Some((*rank, *group));
    }
    let lowered_identifier = bundle_identifier.to_lowercase();
    let lowered_name = display_name.to_lowercase();
    if lowered_identifier == "com.jetbrains.toolbox" || lowered_name.contains("toolbox") {
        return None;
    }
    if !lowered_identifier.starts_with("com.jetbrains.") {
        return None;
    }
    let group = if AI_COMPANIONS.contains(&lowered_identifier.as_str()) {
        OTHER_TOOL
    } else {
        EDITOR
    };
    Some((40, group))
}

fn cli_command(bundle_identifier: &str) -> Option<String> {
    let names = CLI_COMMANDS
        .iter()
        .find(|(identifier, _)| *identifier == bundle_identifier)
        .map(|(_, names)| *names)?;
    names.iter().find_map(|name| executable_path(name))
}

fn executable_path(name: &str) -> Option<String> {
    let mut directories: Vec<String> = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        if !directories.iter().any(|entry| entry == extra) {
            directories.push(extra.to_owned());
        }
    }
    directories.into_iter().find_map(|directory| {
        let candidate = Path::new(&directory).join(name);
        is_executable(&candidate).then(|| candidate.to_string_lossy().into_owned())
    })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or_default()
}

fn spawn(command: &str, arguments: &[&str]) -> bool {
    std::process::Command::new(command)
        .args(arguments)
        .spawn()
        .is_ok()
}

fn bundles() -> Vec<PathBuf> {
    let roots = [
        PathBuf::from("/Applications"),
        home_dir().join("Applications"),
        PathBuf::from("/System/Applications"),
    ];
    roots
        .iter()
        .filter_map(|root| std::fs::read_dir(root).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .collect()
}

fn metadata(bundle: &Path) -> Option<(String, String)> {
    let info = plist::Value::from_file(bundle.join("Contents/Info.plist")).ok()?;
    let info = info.as_dictionary()?;
    let identifier = info
        .get("CFBundleIdentifier")
        .and_then(|value| value.as_string())?
        .to_owned();
    let name = info
        .get("CFBundleDisplayName")
        .or_else(|| info.get("CFBundleName"))
        .and_then(|value| value.as_string())
        .map(str::to_owned)
        .or_else(|| {
            bundle
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })?;
    Some((identifier, name))
}

fn locate_bundle(bundle_identifier: &str) -> Option<PathBuf> {
    bundles()
        .into_iter()
        .find(|path| metadata(path).is_some_and(|(identifier, _)| identifier == bundle_identifier))
}

#[cfg(test)]
mod tests {
    use super::{EDITOR, InstalledIde, OTHER_TOOL};
    use std::path::PathBuf;

    fn entry(identifier: &str, name: &str, rank: u32, group: u8, path: &str) -> InstalledIde {
        InstalledIde {
            bundle_identifier: identifier.to_owned(),
            display_name: name.to_owned(),
            path: PathBuf::from(path),
            rank,
            group,
        }
    }

    fn sorted(mut entries: Vec<InstalledIde>) -> Vec<String> {
        entries.sort_by(|left, right| {
            left.group
                .cmp(&right.group)
                .then_with(|| left.rank.cmp(&right.rank))
                .then_with(|| {
                    left.display_name
                        .to_lowercase()
                        .cmp(&right.display_name.to_lowercase())
                })
                .then_with(|| left.path.cmp(&right.path))
        });
        entries
            .into_iter()
            .map(|entry| entry.display_name)
            .collect()
    }

    #[test]
    fn editors_sort_before_other_tools_then_by_rank_name_and_path() {
        let entries = vec![
            entry(
                "com.openai.codex",
                "Codex",
                80,
                OTHER_TOOL,
                "/Applications/Codex.app",
            ),
            entry("b", "beta", 40, EDITOR, "/Applications/b.app"),
            entry("a", "Alpha", 40, EDITOR, "/Applications/a.app"),
            entry(
                "com.microsoft.VSCode",
                "Code",
                10,
                EDITOR,
                "/Applications/Code.app",
            ),
        ];
        assert_eq!(sorted(entries), vec!["Code", "Alpha", "beta", "Codex"]);
    }

    #[test]
    fn the_admission_rule_follows_the_curated_table_and_jetbrains_prefix() {
        assert_eq!(
            super::admit("com.microsoft.VSCode", "Visual Studio Code"),
            Some((10, EDITOR))
        );
        assert_eq!(
            super::admit("com.jetbrains.Aqua", "Aqua"),
            Some((40, EDITOR))
        );
        assert_eq!(
            super::admit("com.jetbrains.air", "Junie"),
            Some((84, OTHER_TOOL))
        );
        assert_eq!(super::admit("com.jetbrains.toolbox", "Toolbox App"), None);
        assert_eq!(
            super::admit("com.jetbrains.Other", "JetBrains Toolbox"),
            None
        );
        assert_eq!(super::admit("com.apple.Safari", "Safari"), None);
    }
}
