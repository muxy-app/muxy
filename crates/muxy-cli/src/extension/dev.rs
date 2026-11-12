//! `muxy extension dev <folder>`: loads and turns on an unpacked extension,
//! then builds and reloads it each time its files change, printing one
//! `ready` or `failed` line per round with the log lines that round added.

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};

use serde_json::{Map, Value, json};

use super::{CHANGE_TIME, invalid};
use crate::browser::call_app;

/// How often the folder is checked for changes.
const POLL: Duration = Duration::from_millis(400);
/// How long the files must stay unchanged before a round starts, so a save
/// that touches several files builds once.
const QUIET: Duration = Duration::from_millis(300);
/// How long the reloaded pages get to load and log what went wrong.
const SETTLE: Duration = Duration::from_millis(1500);
/// How many lines of a failed build are shown.
const BUILD_LINES: usize = 40;
const LOG_LINES: u64 = 500;
/// Folders that never hold the extension's source.
const SKIPPED: [&str; 4] = ["node_modules", "logs", "target", "coverage"];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Dev {
    folder: PathBuf,
    once: bool,
}

pub(crate) fn parse(words: &[&str]) -> io::Result<Dev> {
    let usage = || invalid("usage: muxy extension dev <folder> [--once]");
    let (folder, once) = match words {
        [folder] => (*folder, false),
        [folder, "--once"] | ["--once", folder] => (*folder, true),
        _ => return Err(usage()),
    };
    if folder.is_empty() || folder.starts_with('-') {
        return Err(usage());
    }
    Ok(Dev {
        folder: std::path::absolute(Path::new(folder))?,
        once,
    })
}

pub(crate) fn run(dev: &Dev) -> Result<(), Box<dyn Error>> {
    let folder = fs::canonicalize(&dev.folder)
        .map_err(|error| format!("can't open {}: {error}", dev.folder.display()))?;
    let package = Package::read(&folder)?;
    let mut round = 1;
    let mut stdout = io::stdout();
    if !build_step(&folder, &package, &package.name, round, &mut stdout)? {
        return Err(format!("{} didn't build; fix it and run dev again", package.name).into());
    }
    let name = prepare(&folder, &package.name)?;
    let ready = reload_step(&name, round, &mut stdout)?;
    if dev.once {
        return if ready {
            Ok(())
        } else {
            Err(format!("{name} isn't working yet").into())
        };
    }
    writeln!(
        stdout,
        "watching {} for changes; press Ctrl-C to stop",
        folder.display()
    )?;
    let mut builds = package.build.is_some();
    let mut seen = fingerprint(&folder, builds);
    loop {
        thread::sleep(POLL);
        let now = fingerprint(&folder, builds);
        if now == seen {
            continue;
        }
        let mut settled = now;
        loop {
            thread::sleep(QUIET);
            let again = fingerprint(&folder, builds);
            if again == settled {
                break;
            }
            settled = again;
        }
        seen = settled;
        round += 1;
        let outcome = Package::read(&folder).and_then(|package| {
            builds = package.build.is_some();
            build_and_reload(&folder, &package, &name, round, &mut stdout)
        });
        if let Err(error) = outcome {
            writeln!(stdout, "failed {name} (round {round}): {error}")?;
            stdout.flush()?;
        }
    }
}

/// What `dev` reads from the extension's `package.json`.
struct Package {
    name: String,
    build: Option<String>,
}

impl Package {
    fn read(folder: &Path) -> Result<Self, Box<dyn Error>> {
        let path = folder.join("package.json");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("can't read {}: {error}", path.display()))?;
        let manifest: Value = serde_json::from_str(&text)
            .map_err(|error| format!("{} isn't valid JSON: {error}", path.display()))?;
        let name = manifest["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| format!("{} has no name", path.display()))?
            .to_owned();
        let build = manifest["scripts"]["build"]
            .as_str()
            .map(|_| runner(folder).to_owned());
        Ok(Self { name, build })
    }
}

/// The package manager whose lock file the folder has.
fn runner(folder: &Path) -> &'static str {
    [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("bun.lock", "bun"),
        ("bun.lockb", "bun"),
    ]
    .iter()
    .find(|(lock, _)| folder.join(lock).is_file())
    .map_or("npm", |(_, runner)| runner)
}

fn call(verb: &str, args: Map<String, Value>, extra: Duration) -> Result<Value, Box<dyn Error>> {
    call_app(verb, args, true, extra)
}

fn named(name: &str) -> Map<String, Value> {
    Map::from_iter([("name".to_owned(), json!(name))])
}

/// Loads the folder unless it already is, and turns the extension on.
fn prepare(folder: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    let status = call("extension.status", named(name), Duration::ZERO);
    let status = if let Ok(status) = status {
        status
    } else {
        let path = folder.to_str().ok_or("the folder's path must be UTF-8")?;
        let loaded = call(
            "extension.load",
            Map::from_iter([("path".to_owned(), json!(path))]),
            CHANGE_TIME,
        )?;
        let name = loaded["name"].as_str().unwrap_or(name).to_owned();
        call("extension.status", named(&name), Duration::ZERO)?
    };
    let name = status["name"].as_str().unwrap_or(name).to_owned();
    let loaded_from = status["path"].as_str().map(PathBuf::from);
    if status["unpacked"].as_bool() != Some(true) || loaded_from.as_deref() != Some(folder) {
        return Err(format!(
            "{name} is already installed from {}; unload or rename it before developing {}",
            loaded_from.unwrap_or_default().display(),
            folder.display()
        )
        .into());
    }
    if status["enabled"].as_bool() != Some(true) {
        call("extension.enable", named(&name), CHANGE_TIME)?;
    }
    Ok(name)
}

/// One round: build, reload, and report. Whether the extension came up
/// without load errors.
fn build_and_reload(
    folder: &Path,
    package: &Package,
    name: &str,
    round: u32,
    out: &mut impl Write,
) -> Result<bool, Box<dyn Error>> {
    Ok(build_step(folder, package, name, round, out)? && reload_step(name, round, out)?)
}

/// Runs the build script, if there is one. Whether it passed.
fn build_step(
    folder: &Path,
    package: &Package,
    name: &str,
    round: u32,
    out: &mut impl Write,
) -> Result<bool, Box<dyn Error>> {
    if let Some(runner) = &package.build {
        writeln!(out, "building {name} with {runner} run build")?;
        out.flush()?;
        if let Some(failure) = build(folder, runner) {
            writeln!(out, "failed {name} (round {round}): the build failed")?;
            for line in failure {
                writeln!(out, "  {line}")?;
            }
            return Ok(false);
        }
    }
    Ok(true)
}

/// Reloads the extension and reports how it came up. Whether it loaded
/// without errors.
fn reload_step(name: &str, round: u32, out: &mut impl Write) -> Result<bool, Box<dyn Error>> {
    let before = log_lines(name);
    if let Err(error) = call("extension.reload", named(name), CHANGE_TIME) {
        writeln!(out, "failed {name} (round {round}): {error}")?;
        return Ok(false);
    }
    thread::sleep(SETTLE);
    let status = call("extension.status", named(name), Duration::ZERO)?;
    let errors: Vec<_> = status["load_errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let after = log_lines(name);
    let added = new_lines(&before, &after);
    let version = status["version"].as_str().unwrap_or_default();
    if errors.is_empty() {
        writeln!(out, "ready {name} {version} (round {round})")?;
    } else {
        writeln!(out, "failed {name} {version} (round {round})")?;
        for error in &errors {
            writeln!(out, "  load error: {error}")?;
        }
    }
    for line in added {
        writeln!(out, "  log: {line}")?;
    }
    if let Some(waiting) = super::waiting_prompts(&status) {
        writeln!(out, "  {waiting}")?;
    }
    out.flush()?;
    Ok(errors.is_empty())
}

/// The last lines of a build that failed, or `None` when it passed.
fn build(folder: &Path, runner: &str) -> Option<Vec<String>> {
    let output = match Command::new(runner)
        .args(["run", "build"])
        .current_dir(folder)
        .output()
    {
        Ok(output) if output.status.success() => return None,
        Ok(output) => output,
        Err(error) => return Some(vec![format!("couldn't run {runner}: {error}")]),
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<_> = text.lines().map(str::to_owned).collect();
    Some(lines[lines.len().saturating_sub(BUILD_LINES)..].to_vec())
}

fn log_lines(name: &str) -> Vec<String> {
    let mut args = named(name);
    args.insert("lines".to_owned(), json!(LOG_LINES));
    call("extension.logs", args, Duration::ZERO)
        .ok()
        .and_then(|reply| {
            reply["lines"].as_array().map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// The lines at the end of `after` that `before` didn't end with. The log
/// keeps only its last lines, so with no overlap every line counts as new.
fn new_lines<'a>(before: &[String], after: &'a [String]) -> &'a [String] {
    let overlap = (0..=before.len().min(after.len()))
        .rev()
        .find(|length| before[before.len() - length..] == after[..*length])
        .unwrap_or(0);
    &after[overlap..]
}

/// The source files' paths, sizes and modification times. `dist` is left
/// out when `dev` builds it, so its own build doesn't start another round.
fn fingerprint(folder: &Path, builds: bool) -> Vec<(PathBuf, u64, SystemTime)> {
    let mut files = Vec::new();
    let mut folders = vec![folder.to_owned()];
    while let Some(current) = folders.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let hidden = name.starts_with('.') && (kind.is_dir() || name == ".DS_Store");
            if hidden || SKIPPED.contains(&name.as_ref()) {
                continue;
            }
            if kind.is_dir() {
                if !(builds && current == folder && name == "dist") {
                    folders.push(entry.path());
                }
            } else if let Ok(metadata) = entry.metadata() {
                files.push((
                    entry.path(),
                    metadata.len(),
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ));
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|line| (*line).to_owned()).collect()
    }

    #[test]
    fn a_folder_and_once_parse() -> io::Result<()> {
        let dev = parse(&["extensions/hello"])?;
        assert!(dev.folder.is_absolute());
        assert!(dev.folder.ends_with("extensions/hello"));
        assert!(!dev.once);
        assert!(parse(&["extensions/hello", "--once"])?.once);
        assert!(parse(&["--once", "extensions/hello"])?.once);
        for words in [vec![], vec!["--once"], vec!["a", "b"], vec!["--watch"]] {
            assert!(parse(&words).is_err(), "{words:?}");
        }
        Ok(())
    }

    #[test]
    fn only_lines_the_round_added_are_new() {
        let before = lines(&["a", "b", "c"]);
        assert_eq!(new_lines(&before, &lines(&["a", "b", "c", "d"])), ["d"]);
        assert_eq!(
            new_lines(&before, &lines(&["b", "c", "d", "e"])),
            ["d", "e"]
        );
        assert!(new_lines(&before, &lines(&["a", "b", "c"])).is_empty());
        assert_eq!(new_lines(&[], &lines(&["x"])), ["x"]);
        assert_eq!(new_lines(&before, &lines(&["x", "y"])), ["x", "y"]);
    }

    #[test]
    fn the_lock_file_picks_the_package_manager() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        assert_eq!(runner(folder.path()), "npm");
        fs::write(folder.path().join("pnpm-lock.yaml"), "")?;
        assert_eq!(runner(folder.path()), "pnpm");
        Ok(())
    }

    #[test]
    fn built_output_and_dependencies_do_not_count_as_changes() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        fs::write(folder.path().join("panel.html"), "a")?;
        let before = fingerprint(folder.path(), true);
        for skipped in ["dist", "node_modules", ".git", "logs"] {
            fs::create_dir(folder.path().join(skipped))?;
            fs::write(folder.path().join(skipped).join("file"), "b")?;
        }
        assert_eq!(fingerprint(folder.path(), true), before);
        assert_ne!(fingerprint(folder.path(), false), before);
        fs::write(folder.path().join(".DS_Store"), "d")?;
        assert_eq!(fingerprint(folder.path(), true), before);
        fs::write(folder.path().join(".env"), "KEY=1")?;
        assert_ne!(fingerprint(folder.path(), true), before);
        fs::create_dir(folder.path().join("src"))?;
        fs::write(folder.path().join("src/main.ts"), "c")?;
        assert_ne!(fingerprint(folder.path(), true), before);
        Ok(())
    }
}
