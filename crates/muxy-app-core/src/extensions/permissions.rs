use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const GIT_READS: [&str; 13] = [
    "git.status",
    "git.diff",
    "git.repoInfo",
    "git.log",
    "git.branches",
    "git.remoteBranches",
    "git.currentBranch",
    "git.aheadBehind",
    "git.pr.info",
    "git.pr.number",
    "git.pr.diff",
    "git.pr.list",
    "git.worktrees",
];

const BROWSER_READS: [&str; 16] = [
    "browser.list",
    "browser.read",
    "browser.waitFor",
    "browser.getText",
    "browser.getHTML",
    "browser.getAttribute",
    "browser.waitForNavigation",
    "browser.screenshot",
    "browser.storage.get",
    "browser.cookies.get",
    "browser.wait",
    "browser.is",
    "browser.getValue",
    "browser.getCount",
    "browser.find",
    "browser.snapshot",
];

/// The manifest permission a bridge verb needs, following main's verb table.
pub fn required_permission(verb: &str, args: &Value) -> Option<&'static str> {
    Some(match verb {
        "files.list" | "files.read" | "files.stat" => "files:read",
        "files.write" | "files.mkdir" | "files.rename" | "files.move" | "files.delete" => {
            "files:write"
        }
        "gh.user" => "gh:read",
        v if GIT_READS.contains(&v) => "git:read",
        v if v.starts_with("git.") => "git:write",
        "exec" | "execAsync" | "exec.start" | "exec.cancel" => "commands:exec",
        "runScript" => "commands:run-script",
        "tabs.list" => "tabs:read",
        "tabs.switch" | "tabs.new" | "tabs.next" | "tabs.previous" | "tabs.open"
        | "tabs.setTitle" | "tabs.setIcon" => "tabs:write",
        "panes.list" | "panes.readScreen" => "panes:read",
        "panes.send" | "panes.sendKeys" | "panes.close" | "panes.rename" => "panes:write",
        "projects.list" | "workspaces.list" => "projects:read",
        "projects.delete" => "projects:delete",
        v if v.starts_with("projects.") || v.starts_with("workspaces.") => "projects:write",
        "worktrees.list" => "worktrees:read",
        "worktrees.switch" | "worktrees.refresh" => "worktrees:write",
        "agents.list" => "agents:read",
        "browser.wait" if args["function"].as_str().is_some_and(|f| !f.is_empty()) => {
            "browser:write"
        }
        v if BROWSER_READS.contains(&v) => "browser:read",
        v if v.starts_with("browser.") => "browser:write",
        "panels.open"
        | "panels.toggle"
        | "panels.close"
        | "popover.close"
        | "popover.resize"
        | "topbar.set"
        | "statusbar.set"
        | "modal.openWebview"
        | "modal.submitWebview"
        | "modal.closeWebview" => "panels:write",
        "notifications.notify" | "toast" => "notifications:write",
        "storage.get" | "storage.keys" => "storage:read",
        "storage.set" | "storage.delete" => "storage:write",
        "shortcuts.register" | "shortcuts.unregister" => "shortcuts:register",
        _ => return None,
    })
}

/// Events that also need a read permission before an extension may subscribe.
pub fn event_permission(event: &str) -> Option<&'static str> {
    match event {
        "agent.status" => Some("agents:read"),
        "file.changed" => Some("files:read"),
        "projects.changed" => Some("projects:read"),
        "worktree.headChanged" => Some("worktrees:read"),
        _ => None,
    }
}

/// Actions that ask the user at runtime even when the manifest grants them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    Exec,
    PanesSend,
    PanesSendKeys,
    PanesReadScreen,
    TabsOpenForeign,
    TabsRunCommand,
    GitWrite,
    FilesWrite,
    HttpFetch,
    ProjectsDelete,
}

impl Gate {
    pub fn name(self) -> &'static str {
        match self {
            Self::Exec => "exec",
            Self::PanesSend => "panes.send",
            Self::PanesSendKeys => "panes.sendKeys",
            Self::PanesReadScreen => "panes.readScreen",
            Self::TabsOpenForeign => "tabs.openForeign",
            Self::TabsRunCommand => "tabs.runCommand",
            Self::GitWrite => "git.write",
            Self::FilesWrite => "files.write",
            Self::HttpFetch => "http.fetch",
            Self::ProjectsDelete => "projects.delete",
        }
    }

    /// Plural description used by "Block all … from this extension".
    pub fn kind(self) -> &'static str {
        match self {
            Self::Exec => "shell commands",
            Self::PanesSend => "terminal input",
            Self::PanesSendKeys => "terminal keystrokes",
            Self::PanesReadScreen => "terminal output reads",
            Self::TabsOpenForeign => "foreign tab opens",
            Self::TabsRunCommand => "auto-run terminal commands",
            Self::GitWrite => "git changes",
            Self::FilesWrite => "file changes",
            Self::HttpFetch => "network requests",
            Self::ProjectsDelete => "project deletions",
        }
    }

    pub fn purpose(self) -> &'static str {
        match self {
            Self::Exec => "wants to run a shell command",
            Self::PanesSend => "wants to type into a terminal",
            Self::PanesSendKeys => "wants to press keys in a terminal",
            Self::PanesReadScreen => "wants to read terminal output",
            Self::TabsOpenForeign => "wants to open another extension's tab",
            Self::TabsRunCommand => "wants to open a terminal that runs a command",
            Self::GitWrite => "wants to modify the git repository",
            Self::FilesWrite => "wants to modify workspace files",
            Self::HttpFetch => "wants to make a network request",
            Self::ProjectsDelete => "wants to delete a project",
        }
    }
}

/// One consent question: the gated action, the rule a "remember" choice saves,
/// and what the prompt shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub gate: Gate,
    pub pattern: String,
    pub summary: String,
    pub details: Vec<String>,
}

const ANY: &str = "*";

impl Request {
    fn new(gate: Gate, pattern: String, summary: String, details: Vec<String>) -> Self {
        Self {
            gate,
            pattern,
            summary,
            details,
        }
    }

    /// Consent for a bridge call, when the verb is gated. `location` is the
    /// repository or project directory the call acts on.
    pub fn for_call(owner: &str, verb: &str, args: &Value, location: &str) -> Option<Self> {
        let text = |field: &str| args[field].as_str().unwrap_or("").to_owned();
        match verb {
            "exec" | "exec.start" => Some(Self::exec(args)),
            "panes.send" | "panes.sendKeys" | "panes.readScreen" => {
                let (gate, action) = match verb {
                    "panes.send" => (Gate::PanesSend, "send to"),
                    "panes.sendKeys" => (Gate::PanesSendKeys, "send-keys to"),
                    _ => (Gate::PanesReadScreen, "read screen of"),
                };
                let pane = text("paneID");
                Some(Self::new(
                    gate,
                    ANY.into(),
                    format!("{action} pane {pane}"),
                    vec![format!("pane: {pane}")],
                ))
            }
            "tabs.open" => Self::tab(owner, args),
            "files.write" | "files.mkdir" | "files.rename" | "files.move" | "files.delete" => {
                let operation = &verb["files.".len()..];
                let path = match operation {
                    "move" => text("into"),
                    "delete" => args["paths"][0].as_str().unwrap_or("").to_owned(),
                    _ => text("path"),
                };
                Some(Self::file(operation, &path))
            }
            "git.worktree.switch" => None,
            v if v.starts_with("git.") && required_permission(v, args) == Some("git:write") => {
                let operation = &v["git.".len()..];
                Some(Self::new(
                    Gate::GitWrite,
                    format!("op:{operation}"),
                    format!("git {operation}"),
                    vec![
                        format!("operation: {operation}"),
                        format!("repo: {location}"),
                    ],
                ))
            }
            _ => None,
        }
    }

    fn exec(args: &Value) -> Self {
        if let Some(shell) = args["shell"].as_str() {
            return Self::new(
                Gate::Exec,
                format!("shell:{shell}"),
                "sh -c …".into(),
                vec![format!("shell: {shell}")],
            );
        }
        let words: Vec<&str> = args["argv"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let joined = words.join(" ");
        Self::new(
            Gate::Exec,
            format!("argv:{}", words.first().unwrap_or(&"")),
            if joined.is_empty() {
                "(empty)".into()
            } else {
                joined.clone()
            },
            vec![format!("argv: {joined}")],
        )
    }

    fn tab(owner: &str, args: &Value) -> Option<Self> {
        if args["kind"] == "terminal" {
            let command = args["command"].as_str().map_or("", str::trim);
            return (!command.is_empty()).then(|| {
                Self::new(
                    Gate::TabsRunCommand,
                    format!("command:{command}"),
                    command.into(),
                    vec![format!("command: {command}")],
                )
            });
        }
        let target = args["extension"]["id"].as_str().unwrap_or("");
        let tab = args["extension"]["tabType"].as_str().unwrap_or("");
        (target != owner).then(|| {
            Self::new(
                Gate::TabsOpenForeign,
                ANY.into(),
                format!("open {target} tab {tab}"),
                vec![format!("extension: {target}"), format!("tab type: {tab}")],
            )
        })
    }

    pub fn file(operation: &str, path: &str) -> Self {
        Self::new(
            Gate::FilesWrite,
            format!("op:{operation}"),
            format!("file {operation}"),
            vec![format!("operation: {operation}"), format!("path: {path}")],
        )
    }

    pub fn http(host: &str, method: &str, url: &str) -> Self {
        Self::new(
            Gate::HttpFetch,
            format!("host:{host}"),
            format!("fetch from {host}"),
            vec![
                format!("host: {host}"),
                format!("method: {method}"),
                format!("url: {url}"),
            ],
        )
    }

    pub fn project_delete(name: &str, path: &str) -> Self {
        Self::new(
            Gate::ProjectsDelete,
            format!("name:{name}"),
            format!("delete project {name}"),
            vec![format!("project: {name}"), format!("path: {path}")],
        )
    }

    /// What a remembered choice will apply to, for the prompt's footer.
    pub fn scope(&self) -> String {
        match self.pattern.split_once(':') {
            Some(("argv", program)) => format!("{program} *"),
            Some((_, value)) => value.into(),
            None => "(any)".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Consent {
    Allow,
    Deny,
    Blocked,
    Ask,
}

/// What the user picked in a consent prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Choice {
    AllowAndRemember,
    Allow,
    Cancel,
    DenyAndRemember,
    Block,
}

impl Choice {
    pub fn allows(self) -> bool {
        matches!(self, Self::AllowAndRemember | Self::Allow)
    }
}

/// Remembered consent rules, keyed `owner:gate:pattern`.
#[derive(Clone, Debug)]
pub struct Grants {
    path: PathBuf,
    rules: BTreeMap<String, Consent>,
}

fn key(owner: &str, gate: Gate, pattern: &str) -> String {
    format!("{owner}:{}:{pattern}", gate.name())
}

/// Rewrites keys saved before gated verbs were grouped (`files:files.write`).
fn migrate(key: String) -> String {
    match key.split_once(':') {
        Some((owner, verb)) if !verb.contains(':') => {
            if let Some(operation) = verb.strip_prefix("files.") {
                return format!("{owner}:files.write:op:{operation}");
            }
            if let Some(operation) = verb.strip_prefix("git.") {
                return format!("{owner}:git.write:op:{operation}");
            }
            key
        }
        _ => key,
    }
}

impl Grants {
    pub fn load(profile: &Path) -> Result<Self, String> {
        let path = profile.join("extension-grants.json");
        let rules: BTreeMap<String, Consent> = match super::storage::read(&path) {
            Ok(value) => serde_json::from_value(value).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self {
            path,
            rules: rules
                .into_iter()
                .map(|(key, consent)| (migrate(key), consent))
                .collect(),
        })
    }

    /// A specific rule wins over the gate-wide one, as on main.
    pub fn decision(&self, owner: &str, request: &Request) -> Consent {
        self.rules
            .get(&key(owner, request.gate, &request.pattern))
            .or_else(|| self.rules.get(&key(owner, request.gate, ANY)))
            .copied()
            .unwrap_or(Consent::Ask)
    }

    pub fn remember(
        &mut self,
        owner: &str,
        request: &Request,
        choice: Choice,
    ) -> Result<(), String> {
        let mut rules = self.rules.clone();
        match choice {
            Choice::AllowAndRemember => {
                rules.insert(key(owner, request.gate, &request.pattern), Consent::Allow);
            }
            Choice::DenyAndRemember => {
                rules.insert(key(owner, request.gate, &request.pattern), Consent::Deny);
            }
            Choice::Block => {
                let prefix = key(owner, request.gate, "");
                rules.retain(|key, _| !key.starts_with(&prefix));
                rules.insert(key(owner, request.gate, ANY), Consent::Blocked);
            }
            Choice::Allow | Choice::Cancel => return Ok(()),
        }
        self.save(rules)
    }

    pub fn clear(&mut self, owner: &str) -> Result<(), String> {
        let mut rules = self.rules.clone();
        rules.retain(|key, _| !key.starts_with(&format!("{owner}:")));
        self.save(rules)
    }

    fn save(&mut self, rules: BTreeMap<String, Consent>) -> Result<(), String> {
        super::storage::write(&self.path, &serde_json::json!(rules))?;
        self.rules = rules;
        Ok(())
    }
}
