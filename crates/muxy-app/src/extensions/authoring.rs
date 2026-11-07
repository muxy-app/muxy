//! What an agent needs to work on an extension: the command that starts it,
//! whether it can reach `muxy`, and the copy a marketplace extension clones to.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use muxy_app_core::extensions::workspace;
use muxy_ui::tr;
use serde_json::Value;

use crate::ai::{InitialPrompt, Provider};

pub(crate) const STORE_REPOSITORY: &str = "muxy-app/extensions";

/// The developer tools that cloning and publishing run, replaceable in tests.
pub(crate) trait Tools {
    fn installed(&self, program: &str) -> bool;
    fn run(
        &self,
        program: &str,
        arguments: &[&str],
        directory: Option<&Path>,
    ) -> Result<String, String>;
    /// Gives GitHub a moment before the next attempt.
    fn wait(&self) {}
}

pub(crate) struct LocalTools;

impl Tools for LocalTools {
    fn installed(&self, program: &str) -> bool {
        crate::ai::find_program(program).is_some()
    }

    fn run(
        &self,
        program: &str,
        arguments: &[&str],
        directory: Option<&Path>,
    ) -> Result<String, String> {
        crate::ai::run_tool(program, arguments, directory)
    }

    fn wait(&self) {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

/// What the user wants an agent to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentTask {
    Create,
    Edit {
        name: String,
        folder: PathBuf,
    },
    Clone {
        name: String,
        /// The installed copy, for when the source can't be downloaded.
        installed: PathBuf,
    },
    Publish {
        name: String,
        folder: PathBuf,
    },
    /// Change the app's settings, working in its profile folder.
    Settings {
        profile: PathBuf,
    },
}

/// Where the agent starts, and what it is told first.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AgentStart {
    pub(crate) directory: PathBuf,
    pub(crate) prompt: String,
    pub(crate) title: String,
}

impl AgentTask {
    /// A folder inside the workspace starts at its root, where the agent finds
    /// the skill by itself. One elsewhere starts where the work is, and the
    /// prompt names the skill so that nothing has to be added to that folder.
    pub(crate) fn start(&self, root: &Path) -> AgentStart {
        let skill = workspace::skill_file(root);
        let skill = skill.display();
        let working_directory = |folder: &Path| {
            if folder.starts_with(root) {
                root.to_owned()
            } else {
                folder.to_owned()
            }
        };
        match self {
            Self::Create => AgentStart {
                directory: root.to_owned(),
                prompt: format!(
                    "Read {skill} and follow it to help me build a Muxy extension. \
                     Start by asking me what it should do."
                ),
                title: tr!("New Extension").to_string(),
            },
            Self::Edit { name, folder } => AgentStart {
                directory: working_directory(folder),
                prompt: format!(
                    "Read {skill} and follow it to help me change the Muxy extension {name} in {}. \
                     Start by asking me what to change.",
                    folder.display()
                ),
                title: tr!("Edit %@", name).to_string(),
            },
            Self::Clone { name, .. } => {
                let copy = clone_name(name);
                let folder = workspace::extension_folder(root, &copy);
                AgentStart {
                    directory: root.to_owned(),
                    prompt: format!(
                        "Read {skill} and follow it. {} is my own copy of the marketplace \
                         extension {name}, renamed {copy}, and it isn't loaded in Muxy yet. \
                         Start by asking me what to change, then load and enable it when it works.",
                        folder.display()
                    ),
                    title: tr!("Edit %@", &copy).to_string(),
                }
            }
            Self::Publish { name, folder } => AgentStart {
                directory: working_directory(folder),
                prompt: format!(
                    "Read {skill} and follow it to help me publish the Muxy extension {name} in {}. \
                     Before running anything that forks or pushes, explain what publishing will \
                     do and wait for my yes.",
                    folder.display()
                ),
                title: tr!("Publish %@", name).to_string(),
            },
            Self::Settings { profile } => AgentStart {
                directory: profile.clone(),
                prompt: format!(
                    "Read {} and follow it to help me change Muxy's settings. \
                     Start by asking me what to change.",
                    muxy_app_core::settings::agent::skill_file(profile).display()
                ),
                title: tr!("Muxy Settings").to_string(),
            },
        }
    }

    /// Readies the folder the agent starts in: its skill, and for a clone the
    /// copy it works on.
    pub(crate) fn prepare(&self, root: &Path) -> Result<(), String> {
        if let Self::Settings { profile } = self {
            return muxy_app_core::settings::agent::prepare(profile)
                .map_err(|error| error.to_string());
        }
        workspace::prepare(root).map_err(|error| error.to_string())?;
        if let Self::Clone { name, installed } = self {
            clone_extension(&LocalTools, root, name, installed)?;
        }
        Ok(())
    }
}

pub(crate) fn clone_name(name: &str) -> String {
    format!("{name}-custom")
}

/// A word the shell reads literally, in every shell Muxy starts. Backslashes go
/// outside the quotes too, because fish reads one inside them as an escape.
/// Control characters can't be quoted, since the line editor acts on them first:
/// see [`typed_safely`].
pub(crate) fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_./=:@+".contains(&byte));
    if plain {
        return text.to_owned();
    }
    let mut quoted = String::from("'");
    for character in text.chars() {
        match character {
            '\'' => quoted.push_str(r"'\''"),
            '\\' => quoted.push_str(r"'\\'"),
            character => quoted.push(character),
        }
    }
    quoted.push('\'');
    quoted
}

/// A line is typed into the shell, so a control character in it, such as from a
/// folder's name, would act on the line instead of being part of it. A
/// replacement character means a path wasn't text, so it would name another file.
pub(crate) fn typed_safely(line: &str) -> Result<(), String> {
    if line.contains(|character: char| character.is_control() || character == '\u{fffd}') {
        return Err(tr!(
            "This folder's path has characters that can't be typed into a terminal safely. Rename it and try again."
        )
        .to_string());
    }
    Ok(())
}

/// The line typed into a new terminal to start `provider`. `muxy_directory`
/// comes first on its `PATH` when the shell would otherwise find no `muxy`, or
/// the one of another Muxy build.
pub(crate) fn launch_line(
    provider: Provider,
    executable: &Path,
    prompt: &str,
    muxy_directory: Option<&Path>,
) -> String {
    let (arguments, initial) = provider.interactive();
    let mut words = vec![shell_quote(&executable.to_string_lossy())];
    words.extend(arguments.iter().map(|word| shell_quote(word)));
    let hint = match initial {
        InitialPrompt::Argument => {
            words.push(shell_quote(prompt));
            None
        }
        InitialPrompt::Flag(flag) => {
            words.push(flag.to_owned());
            words.push(shell_quote(prompt));
            None
        }
        InitialPrompt::Typed => Some(format!(
            "printf '%s\\n' {}; ",
            shell_quote(&tr!("Paste this into the agent: %@", prompt))
        )),
    };
    let path = muxy_directory.map_or_else(String::new, |directory| {
        format!(
            "PATH={}:\"$PATH\" ",
            shell_quote(&directory.to_string_lossy())
        )
    });
    format!("{}{path}{}", hint.unwrap_or_default(), words.join(" "))
}

/// Whether an agent will be able to run `muxy` once the terminal opens.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CliPlan {
    /// It can, with this folder first on `PATH` if that is needed.
    Ready(Option<PathBuf>),
    /// Nothing is installed, so the user is asked first.
    AskToInstall,
    /// This copy of the app can't install it, for this reason.
    Unavailable(String),
}

/// `tool` is this build's `muxy`, `installed` any `muxy` on this computer,
/// `in_shell` the one the user's shell runs, and `unbundled` the one beside a
/// development build, which is never installed.
pub(crate) fn plan_cli(
    tool: Result<PathBuf, String>,
    installed: Option<&Path>,
    in_shell: Option<&Path>,
    unbundled: Option<&Path>,
) -> CliPlan {
    let tool = match tool {
        Ok(tool) => tool,
        Err(reason) => {
            return match unbundled.and_then(Path::parent) {
                Some(folder) => CliPlan::Ready(Some(folder.to_owned())),
                None => CliPlan::Unavailable(reason),
            };
        }
    };
    if installed.is_none() {
        return CliPlan::AskToInstall;
    }
    let same = in_shell.is_some_and(|found| canonical(found) == canonical(&tool));
    if same {
        CliPlan::Ready(None)
    } else {
        CliPlan::Ready(tool.parent().map(Path::to_owned))
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

/// How a marketplace extension became the user's own copy.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ClonedFrom {
    /// The copy from an earlier clone, with the user's changes.
    EarlierClone,
    Source,
    /// Only the installed build output could be copied.
    InstalledBuild,
}

/// Gives the user their own copy of a marketplace extension in the workspace.
/// Cloning again returns to the same copy rather than replacing their work.
pub(crate) fn clone_extension(
    tools: &dyn Tools,
    root: &Path,
    name: &str,
    installed: &Path,
) -> Result<ClonedFrom, String> {
    let copy = clone_name(name);
    let target = workspace::extension_folder(root, &copy);
    if target.join("package.json").is_file() {
        return Ok(ClonedFrom::EarlierClone);
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(tr!(
            "%@ already exists and isn't an extension. Move it and try again.",
            target.display().to_string()
        )
        .to_string());
    }
    let scratch = tempfile::tempdir_in(root).map_err(|error| error.to_string())?;
    let staged = scratch.path().join("copy");
    let from = if clone_source(tools, root, scratch.path(), name, &staged).is_ok() {
        ClonedFrom::Source
    } else {
        copy_tree(installed, &staged).map_err(|error| error.to_string())?;
        ClonedFrom::InstalledBuild
    };
    rename_package(&staged.join("package.json"), &copy)?;
    fs::rename(&staged, &target).map_err(|error| error.to_string())?;
    Ok(from)
}

fn clone_source(
    tools: &dyn Tools,
    root: &Path,
    scratch: &Path,
    name: &str,
    staged: &Path,
) -> Result<(), String> {
    let repository = scratch.join("repository");
    let url = format!("https://github.com/{STORE_REPOSITORY}");
    tools.run(
        "git",
        &[
            "clone",
            "--filter=blob:none",
            "--sparse",
            "--depth=1",
            &url,
            &repository.to_string_lossy(),
        ],
        None,
    )?;
    let source = format!("extensions/{name}");
    tools.run(
        "git",
        &["sparse-checkout", "set", &source, "shared"],
        Some(&repository),
    )?;
    let source = repository.join(source);
    if !source.join("package.json").is_file() {
        return Err(tr!("The store has no source for %@", name).to_string());
    }
    copy_tree(&source, staged).map_err(|error| error.to_string())?;
    let shared = repository.join("shared");
    let kept = root.join("shared");
    if shared.is_dir() && !kept.exists() {
        let staged = scratch.join("shared");
        copy_tree(&shared, &staged).map_err(|error| error.to_string())?;
        fs::rename(&staged, &kept).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Copies a folder's files, leaving out `.git` and links, which could reach
/// outside it.
pub(crate) fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() || entry.file_name() == ".git" {
            continue;
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Changes the package's name, keeping the file's layout where possible so the
/// copy still diffs cleanly against the original.
fn rename_package(file: &Path, name: &str) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|error| error.to_string())?;
    let mut value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err(tr!("package.json must hold an object.").to_string());
    }
    let renamed = if let Some(renamed) = renamed_in_place(&text, &value, name) {
        renamed
    } else {
        value["name"] = Value::from(name);
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())? + "\n"
    };
    fs::write(file, renamed).map_err(|error| error.to_string())
}

/// The text with the top-level name replaced. Other objects can have a `name`
/// too, so each is tried until only the package's own name has changed.
fn renamed_in_place(text: &str, value: &Value, name: &str) -> Option<String> {
    let current = serde_json::to_string(value.get("name")?).ok()?;
    let replacement = serde_json::to_string(name).ok()?;
    let mut wanted = value.clone();
    wanted["name"] = Value::from(name);
    text.match_indices("\"name\"").find_map(|(start, _)| {
        let colon = start + text[start..].find(':')?;
        let value_start = colon + text[colon..].find(&current)?;
        let renamed = format!(
            "{}{replacement}{}",
            &text[..value_start],
            &text[value_start + current.len()..]
        );
        serde_json::from_str::<Value>(&renamed)
            .is_ok_and(|parsed| parsed == wanted)
            .then_some(renamed)
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct NoTools;

    impl Tools for NoTools {
        fn installed(&self, _: &str) -> bool {
            false
        }

        fn run(&self, program: &str, _: &[&str], _: Option<&Path>) -> Result<String, String> {
            Err(format!("{program} is not installed"))
        }
    }

    /// A Git that "clones" by writing the store's layout where it was told to.
    struct FakeGit {
        calls: RefCell<Vec<String>>,
    }

    impl Tools for FakeGit {
        fn installed(&self, _: &str) -> bool {
            true
        }

        fn run(
            &self,
            program: &str,
            arguments: &[&str],
            directory: Option<&Path>,
        ) -> Result<String, String> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", arguments.join(" ")));
            if arguments.first() == Some(&"clone") {
                let repository = Path::new(arguments[arguments.len() - 1]);
                let source = repository.join("extensions/hello");
                fs::create_dir_all(&source).map_err(|error| error.to_string())?;
                fs::write(
                    source.join("package.json"),
                    "{\n  \"author\": { \"name\": \"hello\" },\n  \"name\": \"hello\",\n  \"version\": \"1.0.0\",\n  \"muxy\": {}\n}\n",
                )
                .map_err(|error| error.to_string())?;
                let shared = repository.join("shared/ui");
                fs::create_dir_all(&shared).map_err(|error| error.to_string())?;
                fs::write(shared.join("ui.css"), "a{}").map_err(|error| error.to_string())?;
            }
            assert!(directory.is_none() || arguments.first() != Some(&"clone"));
            Ok(String::new())
        }
    }

    fn provider(id: &str) -> Provider {
        crate::ai::provider(id).unwrap_or_else(|| panic!("no provider {id}"))
    }

    #[test]
    fn every_provider_starts_interactively_with_or_without_its_prompt() {
        let executable = Path::new("/bin/agent");
        assert_eq!(
            launch_line(provider("claude"), executable, "hi there", None),
            "/bin/agent 'hi there'"
        );
        assert_eq!(
            launch_line(provider("opencode"), executable, "hi", None),
            "/bin/agent --prompt hi"
        );
        assert_eq!(
            launch_line(provider("copilot"), executable, "hi", None),
            "/bin/agent -i hi"
        );
        assert_eq!(
            launch_line(provider("kiro"), executable, "hi", None),
            "printf '%s\\n' 'Paste this into the agent: hi'; /bin/agent chat"
        );
        for provider in crate::ai::PROVIDERS {
            let line = launch_line(*provider, executable, "hi", None);
            for headless in ["--print", "--tools=", "exec", "--no-interactive", " run"] {
                assert!(!line.contains(headless), "{}: {line}", provider.id);
            }
        }
    }

    #[test]
    fn a_needed_muxy_folder_goes_first_on_path() {
        assert_eq!(
            launch_line(
                provider("codex"),
                Path::new("/bin/codex"),
                "hi",
                Some(Path::new("/Apps/Muxy Beta.app/Contents/MacOS"))
            ),
            "PATH='/Apps/Muxy Beta.app/Contents/MacOS':\"$PATH\" /bin/codex hi"
        );
    }

    #[test]
    fn prompts_and_paths_stay_one_shell_word() {
        assert_eq!(shell_quote("it's $HOME; rm"), r"'it'\''s $HOME; rm'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("/usr/bin/gh"), "/usr/bin/gh");
    }

    #[test]
    fn a_backslash_stays_outside_the_quotes_and_they_reopen_after_it() {
        assert_eq!(shell_quote(r"a\b"), r"'a'\\'b'");
        assert_eq!(shell_quote(r"a\"), r"'a'\\''");
    }

    /// A quoted word has to come back from a real shell as the same text, with
    /// nothing in it run.
    #[test]
    fn quoted_text_reaches_real_shells_unchanged_and_runs_nothing() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let marker = folder.path().join("ran");
        let marker = marker.display();
        let texts = [
            format!(r"a\b;touch {marker};#"),
            format!(r"x\';touch {marker};#"),
            format!("it's; touch {marker} $(touch {marker}) `touch {marker}`"),
            format!(r"\\'\';touch {marker}"),
            r"trailing\".to_owned(),
            "plain".to_owned(),
            String::new(),
        ];
        for shell in ["/bin/sh", "/bin/bash", "/bin/zsh"] {
            if !Path::new(shell).exists() {
                continue;
            }
            for text in &texts {
                let output = std::process::Command::new(shell)
                    .arg("-c")
                    .arg(format!("printf %s {}", shell_quote(text)))
                    .output()?;
                assert_eq!(String::from_utf8_lossy(&output.stdout), *text, "{shell}");
                assert!(
                    !folder.path().join("ran").exists(),
                    "{shell} ran part of {text:?}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn control_characters_are_never_typed() {
        assert!(typed_safely("/bin/agent 'plain words'").is_ok());
        for line in [
            "x\u{3}touch p",
            "x\rtouch p",
            "x\ntouch p",
            "x\u{1b}[A",
            "/bin/ag\u{fffd}nt",
        ] {
            assert!(typed_safely(line).is_err(), "{line:?}");
        }
    }

    #[test]
    fn plans_how_an_agent_reaches_muxy() {
        let tool = PathBuf::from("/Apps/Muxy.app/Contents/MacOS/muxy");
        let folder = Some(PathBuf::from("/Apps/Muxy.app/Contents/MacOS"));
        assert_eq!(
            plan_cli(Ok(tool.clone()), None, None, None),
            CliPlan::AskToInstall
        );
        assert_eq!(
            plan_cli(Ok(tool.clone()), Some(&tool), Some(&tool), None),
            CliPlan::Ready(None)
        );
        let other = Path::new("/Apps/Muxy Beta.app/Contents/MacOS/muxy");
        assert_eq!(
            plan_cli(Ok(tool.clone()), Some(other), Some(other), None),
            CliPlan::Ready(folder.clone()),
            "another build's muxy would talk to that build's app"
        );
        assert_eq!(
            plan_cli(
                Ok(tool),
                Some(Path::new("/home/.local/bin/muxy")),
                None,
                None
            ),
            CliPlan::Ready(folder),
            "installed, but not on the shell's PATH"
        );
        assert_eq!(
            plan_cli(Err("move the app".into()), None, None, None),
            CliPlan::Unavailable("move the app".into())
        );
        assert_eq!(
            plan_cli(
                Err("not bundled".into()),
                None,
                None,
                Some(Path::new("/target/debug/muxy"))
            ),
            CliPlan::Ready(Some(PathBuf::from("/target/debug")))
        );
    }

    #[test]
    fn work_outside_the_workspace_stays_outside_it() {
        let root = Path::new("/config/muxy/extension-workspace");
        let inside = AgentTask::Edit {
            name: "hello".into(),
            folder: root.join("extensions/hello"),
        };
        assert_eq!(inside.start(root).directory, root);
        let outside = AgentTask::Edit {
            name: "hello".into(),
            folder: PathBuf::from("/code/hello"),
        };
        let start = outside.start(root);
        assert_eq!(start.directory, Path::new("/code/hello"));
        assert!(
            start.prompt.contains(
                "/config/muxy/extension-workspace/.agents/skills/muxy-extensions/SKILL.md"
            ),
            "{}",
            start.prompt
        );
        assert!(!start.prompt.contains('\n'));
    }

    #[test]
    fn publishing_through_an_agent_asks_before_it_changes_anything() {
        let task = AgentTask::Publish {
            name: "hello".into(),
            folder: PathBuf::from("/code/hello"),
        };
        assert!(
            task.start(Path::new("/w"))
                .prompt
                .contains("wait for my yes")
        );
    }

    #[test]
    fn cloning_copies_the_source_and_shared_ui_under_a_new_name() -> Result<(), String> {
        let folder = tempfile::tempdir().map_err(|error| error.to_string())?;
        let root = folder.path();
        workspace::prepare(root).map_err(|error| error.to_string())?;
        let tools = FakeGit {
            calls: RefCell::default(),
        };
        let from = clone_extension(&tools, root, "hello", Path::new("/unused"))?;
        assert_eq!(from, ClonedFrom::Source);
        let copy = workspace::extension_folder(root, "hello-custom");
        let text = fs::read_to_string(copy.join("package.json")).map_err(|e| e.to_string())?;
        assert!(text.contains("\"name\": \"hello-custom\""), "{text}");
        assert!(
            text.starts_with("{\n  \"author\": { \"name\": \"hello\" },\n"),
            "the author's name and the file's layout stay: {text}"
        );
        assert!(root.join("shared/ui/ui.css").is_file());
        let left: Vec<_> = fs::read_dir(root)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        Ok(())
    }

    #[test]
    fn cloning_falls_back_to_the_installed_build_and_never_overwrites() -> Result<(), String> {
        let folder = tempfile::tempdir().map_err(|error| error.to_string())?;
        let root = folder.path();
        workspace::prepare(root).map_err(|error| error.to_string())?;
        let installed = root.join("installed");
        fs::create_dir_all(&installed).map_err(|error| error.to_string())?;
        fs::write(
            installed.join("package.json"),
            r#"{"name":"hello","version":"1.0.0","muxy":{}}"#,
        )
        .map_err(|error| error.to_string())?;
        assert_eq!(
            clone_extension(&NoTools, root, "hello", &installed)?,
            ClonedFrom::InstalledBuild
        );
        let copy = workspace::extension_folder(root, "hello-custom").join("package.json");
        let text = fs::read_to_string(&copy).map_err(|error| error.to_string())?;
        assert!(text.contains("hello-custom"), "{text}");
        fs::write(&copy, "the user's changes").map_err(|error| error.to_string())?;
        assert_eq!(
            clone_extension(&NoTools, root, "hello", &installed)?,
            ClonedFrom::EarlierClone
        );
        assert_eq!(
            fs::read_to_string(&copy).map_err(|error| error.to_string())?,
            "the user's changes"
        );
        Ok(())
    }

    #[test]
    fn a_clone_target_that_isnt_an_extension_is_left_alone() -> Result<(), String> {
        let folder = tempfile::tempdir().map_err(|error| error.to_string())?;
        let root = folder.path();
        workspace::prepare(root).map_err(|error| error.to_string())?;
        let target = workspace::extension_folder(root, "hello-custom");
        fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        fs::write(target.join("notes.txt"), "mine").map_err(|error| error.to_string())?;
        let error = clone_extension(&NoTools, root, "hello", Path::new("/unused")).unwrap_err();
        assert!(error.contains("isn't an extension"), "{error}");
        assert!(target.join("notes.txt").is_file());
        Ok(())
    }

    #[test]
    fn renaming_refuses_a_manifest_that_isnt_an_object() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let file = folder.path().join("package.json");
        for manifest in ["[]", "\"text\"", "1", "true"] {
            fs::write(&file, manifest)?;
            assert!(rename_package(&file, "hello").is_err(), "{manifest}");
        }
        Ok(())
    }

    #[test]
    fn copying_skips_links_and_git() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let from = folder.path().join("from");
        fs::create_dir_all(from.join(".git"))?;
        fs::create_dir_all(from.join("src"))?;
        fs::write(from.join("src/a.js"), "a")?;
        fs::write(from.join(".git/config"), "c")?;
        std::os::unix::fs::symlink("/etc/hosts", from.join("hosts"))?;
        let to = folder.path().join("to");
        copy_tree(&from, &to)?;
        assert!(to.join("src/a.js").is_file());
        assert!(!to.join(".git").exists());
        assert!(fs::symlink_metadata(to.join("hosts")).is_err());
        Ok(())
    }
}
