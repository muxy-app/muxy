use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn required_permission(verb: &str) -> Option<&'static str> {
    match verb {
        "files.list" | "files.read" | "files.stat" => Some("files:read"),
        "files.write" | "files.mkdir" | "files.rename" | "files.move" | "files.delete" => {
            Some("files:write")
        }
        "git.status" | "git.diff" | "git.repoInfo" | "git.log" | "git.branches"
        | "git.remoteBranches" | "git.currentBranch" | "git.aheadBehind" | "git.pr.info"
        | "git.pr.number" | "git.pr.diff" | "git.pr.list" | "git.worktrees" => Some("git:read"),
        v if v.starts_with("git.") => Some("git:write"),
        "exec" | "execAsync" | "exec.start" | "exec.cancel" => Some("commands:exec"),
        "modal.openWebview" | "modal.submitWebview" | "modal.closeWebview" => Some("panels:write"),
        "runScript" => Some("commands:run-script"),
        "tabs.list" => Some("tabs:read"),
        v if v.starts_with("tabs.") => Some("tabs:write"),
        "worktrees.list" => Some("worktrees:read"),
        "worktrees.refresh" | "worktrees.switch" => Some("worktrees:write"),
        v if v.starts_with("panels.") => Some("panels:write"),
        "notifications.notify" | "toast" => Some("notifications:write"),
        "storage.get" | "storage.keys" => Some("storage:read"),
        "storage.set" | "storage.delete" => Some("storage:write"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Consent {
    Allow,
    Deny,
    Ask,
}

#[derive(Clone, Debug)]
pub struct Grants {
    path: PathBuf,
    rules: BTreeMap<String, Consent>,
}

impl Grants {
    pub fn load(profile: &Path) -> Result<Self, String> {
        let path = profile.join("extension-grants.json");
        let rules = match super::storage::read(&path) {
            Ok(value) => serde_json::from_value(value).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self { path, rules })
    }

    pub fn key(owner: &str, verb: &str, args: &Value) -> Option<String> {
        match required_permission(verb) {
            Some("files:write" | "git:write") => Some(format!("{owner}:{verb}")),
            Some("commands:exec") if verb != "exec.cancel" => {
                let pattern = if let Some(shell) = args["shell"].as_str() {
                    format!("shell:{shell}")
                } else {
                    format!("argv:{}", args["argv"][0].as_str().unwrap_or(""))
                };
                Some(format!("{owner}:exec:{pattern}"))
            }
            _ => None,
        }
    }

    pub fn decision(&self, key: &str) -> Consent {
        self.rules.get(key).copied().unwrap_or(Consent::Ask)
    }

    pub fn remember(&mut self, key: &str, decision: Consent) -> Result<(), String> {
        let mut rules = self.rules.clone();
        rules.insert(key.into(), decision);
        super::storage::write(&self.path, &serde_json::json!(rules))?;
        self.rules = rules;
        Ok(())
    }

    pub fn clear(&mut self, owner: &str) -> Result<(), String> {
        let mut rules = self.rules.clone();
        rules.retain(|key, _| !key.starts_with(&format!("{owner}:")));
        super::storage::write(&self.path, &serde_json::json!(rules))?;
        self.rules = rules;
        Ok(())
    }
}
