use muxy_proto::session::EnvironmentEntry;
use std::collections::HashMap;
use std::path::Path;

pub const DEFAULT_SHELL: &str = "/bin/zsh";
const POSIX_SHELL: &str = "/bin/sh";
const XDG_FALLBACK_DATA_DIRECTORIES: &str = "/usr/local/share:/usr/share";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellInvocation {
    pub executable: String,
    pub arguments: Vec<String>,
    pub environment: Vec<EnvironmentEntry>,
}

#[derive(Clone, Debug)]
struct OrderedEnvironment {
    keys: Vec<String>,
    values: HashMap<String, String>,
}

impl OrderedEnvironment {
    fn new(entries: &[EnvironmentEntry]) -> Self {
        let mut environment = Self {
            keys: Vec::new(),
            values: HashMap::new(),
        };
        for entry in entries {
            environment.set(&entry.key, Some(entry.value.clone()));
        }
        environment
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    fn set(&mut self, key: &str, value: Option<String>) {
        match value {
            Some(value) => {
                if !self.values.contains_key(key) {
                    self.keys.push(key.to_owned());
                }
                self.values.insert(key.to_owned(), value);
            }
            None => {
                self.keys.retain(|held| held != key);
                self.values.remove(key);
            }
        }
    }

    fn entries(self) -> Vec<EnvironmentEntry> {
        self.keys
            .into_iter()
            .filter_map(|key| {
                self.values
                    .get(&key)
                    .cloned()
                    .map(|value| EnvironmentEntry { key, value })
            })
            .collect()
    }
}

pub fn invocation(
    command: &str,
    shell: &str,
    resources_directory: &str,
    environment: &[EnvironmentEntry],
) -> ShellInvocation {
    let resolved_shell = if shell.is_empty() {
        DEFAULT_SHELL
    } else {
        shell
    };
    let name = shell_name(resolved_shell);
    let mut environment = OrderedEnvironment::new(environment);
    let mut arguments = vec![format!("-{name}")];

    if !resources_directory.is_empty() {
        environment.set(
            "GHOSTTY_RESOURCES_DIR",
            Some(resources_directory.to_owned()),
        );
        let root = Path::new(resources_directory).join("shell-integration");
        let root = root.to_string_lossy();
        match name {
            "zsh" => apply_zsh(&root, &mut environment),
            "bash" => {
                apply_bash(&root, &mut environment);
                if command.is_empty() {
                    arguments.push("--posix".into());
                }
            }
            "fish" | "elvish" | "nu" => apply_xdg(&root, &mut environment),
            _ => {}
        }
    }

    if command.is_empty() {
        ShellInvocation {
            executable: resolved_shell.to_owned(),
            arguments,
            environment: environment.entries(),
        }
    } else {
        ShellInvocation {
            executable: POSIX_SHELL.into(),
            arguments: vec![POSIX_SHELL.into(), "-c".into(), format!("exec {command}")],
            environment: environment.entries(),
        }
    }
}

pub fn shell_name(shell: &str) -> &str {
    Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(shell)
}

fn apply_zsh(root: &str, environment: &mut OrderedEnvironment) {
    if let Some(existing) = environment.get("ZDOTDIR").map(str::to_owned) {
        environment.set("GHOSTTY_ZSH_ZDOTDIR", Some(existing));
    }
    environment.set("ZDOTDIR", Some(format!("{root}/zsh")));
}

fn apply_bash(root: &str, environment: &mut OrderedEnvironment) {
    if let Some(existing) = environment.get("ENV").map(str::to_owned) {
        environment.set("GHOSTTY_BASH_ENV", Some(existing));
    }
    environment.set("ENV", Some(format!("{root}/bash/ghostty.bash")));
    environment.set("GHOSTTY_BASH_INJECT", Some("1".into()));
    if environment.get("HISTFILE").is_none()
        && let Some(home) = environment.get("HOME").filter(|home| !home.is_empty())
    {
        environment.set("HISTFILE", Some(format!("{home}/.bash_history")));
        environment.set("GHOSTTY_BASH_UNEXPORT_HISTFILE", Some("1".into()));
    }
}

fn apply_xdg(root: &str, environment: &mut OrderedEnvironment) {
    environment.set("GHOSTTY_SHELL_INTEGRATION_XDG_DIR", Some(root.to_owned()));
    let existing = environment.get("XDG_DATA_DIRS").map(str::to_owned);
    match existing.as_deref() {
        None | Some("") => environment.set(
            "XDG_DATA_DIRS",
            Some(format!("{root}:{XDG_FALLBACK_DATA_DIRECTORIES}")),
        ),
        Some(value) if !value.split(':').any(|entry| entry == root) => {
            environment.set("XDG_DATA_DIRS", Some(format!("{root}:{value}")));
        }
        Some(_) => {}
    }
}

#[cfg(test)]
fn value_for_test<'a>(entries: &'a [EnvironmentEntry], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(values: &[(&str, &str)]) -> Vec<EnvironmentEntry> {
        values
            .iter()
            .map(|(key, value)| EnvironmentEntry {
                key: (*key).into(),
                value: (*value).into(),
            })
            .collect()
    }

    fn value<'a>(invocation: &'a ShellInvocation, key: &str) -> Option<&'a str> {
        invocation
            .environment
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_str())
    }

    #[test]
    fn shell_integration_vectors_match_retained_behavior() {
        let resources = "/Applications/Muxy.app/Contents/Resources/ghostty";
        let root = format!("{resources}/shell-integration");
        let zsh = invocation("", "/bin/zsh", resources, &entries(&[("ZDOTDIR", "/old")]));
        assert_eq!(zsh.arguments, ["-zsh"]);
        assert_eq!(value(&zsh, "ZDOTDIR"), Some(format!("{root}/zsh").as_str()));
        assert_eq!(value(&zsh, "GHOSTTY_ZSH_ZDOTDIR"), Some("/old"));

        let bash = invocation(
            "",
            "/bin/bash",
            resources,
            &entries(&[("HOME", "/home/test")]),
        );
        assert_eq!(bash.arguments, ["-bash", "--posix"]);
        assert_eq!(
            value(&bash, "ENV"),
            Some(format!("{root}/bash/ghostty.bash").as_str())
        );
        assert_eq!(value(&bash, "HISTFILE"), Some("/home/test/.bash_history"));

        for shell in ["/bin/fish", "/bin/elvish", "/bin/nu"] {
            let value = invocation(
                shell,
                shell,
                resources,
                &entries(&[("XDG_DATA_DIRS", "/usr/share")]),
            );
            assert_eq!(
                super::value_for_test(&value.environment, "XDG_DATA_DIRS"),
                Some(format!("{root}:/usr/share").as_str())
            );
        }
    }

    #[test]
    fn startup_and_unknown_shell_vectors_match_retained_behavior() {
        let resources = "/bundle/ghostty";
        let command = "echo hi";
        let startup = invocation(command, "/bin/bash", resources, &[]);
        assert_eq!(startup.executable, "/bin/sh");
        assert_eq!(startup.arguments, ["/bin/sh", "-c", "exec echo hi"]);
        assert_eq!(value(&startup, "GHOSTTY_BASH_INJECT"), Some("1"));

        let unknown = invocation("", "/usr/local/bin/xonsh", resources, &[]);
        assert_eq!(unknown.arguments, ["-xonsh"]);
        assert_eq!(value(&unknown, "ENV"), None);
        assert_eq!(value(&unknown, "XDG_DATA_DIRS"), None);

        let fallback = invocation("", "", "", &[]);
        assert_eq!(fallback.executable, DEFAULT_SHELL);
        assert_eq!(fallback.arguments, ["-zsh"]);
    }
}
