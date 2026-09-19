use gpui::Context;
use muxy_app_core::extensions::api;
use muxy_client::Client;
use muxy_protocol::{
    GitAction, GitReply, GitRequest, ProjectId, ServerPath, WorktreeAction, WorktreeIntent,
};
use serde_json::{Value, json};

use super::{AppModel, Call};

pub(super) fn handles(verb: &str) -> bool {
    matches!(
        verb,
        "worktrees.refresh"
            | "worktrees.switch"
            | "git.worktree.switch"
            | "git.worktree.add"
            | "git.worktree.remove"
            | "git.pr.checkoutWorktree"
    )
}

async fn git(client: &Client, project: ProjectId, action: GitAction) -> Result<GitReply, String> {
    client
        .git_async(GitRequest { project, action })
        .await
        .map_err(|error| error.to_string())
}

fn operation(action: WorktreeAction) -> GitAction {
    GitAction::Worktree(WorktreeIntent {
        operation: muxy_protocol::OperationId::new(),
        action,
    })
}

pub(super) async fn call(client: &Client, call: &Call) -> Result<Value, String> {
    let catalog = client
        .catalog_async()
        .await
        .map_err(|error| error.to_string())?;
    let origin = catalog
        .projects
        .iter()
        .find(|p| p.id == call.project)
        .ok_or("project was not found")?;
    let root = origin.parent_id.unwrap_or(origin.id);
    if matches!(
        call.verb.as_str(),
        "worktrees.refresh" | "worktrees.switch" | "git.worktree.switch"
    ) {
        let GitReply::Worktrees(worktrees) = git(client, root, GitAction::Worktrees).await? else {
            return Err("unexpected worktree listing".into());
        };
        for worktree in &worktrees {
            if !worktree.primary
                && !worktree.bare
                && !worktree.prunable
                && worktree.registered.is_none()
            {
                git(
                    client,
                    root,
                    operation(WorktreeAction::Register {
                        project: ProjectId::new(),
                        directory: worktree.directory.clone(),
                    }),
                )
                .await?;
            }
        }
        return Ok(json!({"count":worktrees.len()}));
    }
    let requested = api::text(&call.args, "path")?;
    let directory = resolve_path(requested, api::path_text(&origin.directory)?)?;
    if call.verb == "git.worktree.remove" {
        let target = catalog
            .projects
            .iter()
            .find(|p| p.parent_id == Some(root) && p.directory == directory)
            .ok_or("worktree is not registered; refresh worktrees first")?;
        let GitReply::Removal(expected) = git(client, target.id, GitAction::InspectRemoval).await?
        else {
            return Err("unexpected worktree inspection".into());
        };
        if expected.dirty && !call.args["force"].as_bool().unwrap_or(false) {
            return Err("worktree has uncommitted changes".into());
        }
        git(
            client,
            target.id,
            operation(WorktreeAction::Remove { expected }),
        )
        .await?;
        return Ok(json!({"path":api::path_text(&directory)?,"dirRemoved":true}));
    }
    let action = if call.verb == "git.pr.checkoutWorktree" {
        WorktreeAction::CheckoutPullRequest {
            project: ProjectId::new(),
            directory,
            number: call.args["number"]
                .as_u64()
                .ok_or("invalid pull request number")?,
        }
    } else {
        WorktreeAction::Create {
            project: ProjectId::new(),
            directory,
            branch: api::text(&call.args, "branch")?.into(),
            base: call.args["createBranch"]
                .as_bool()
                .unwrap_or(false)
                .then(|| {
                    call.args["baseBranch"]
                        .as_str()
                        .filter(|base| !base.is_empty())
                        .unwrap_or("HEAD")
                        .to_owned()
                }),
        }
    };
    let GitReply::Project(project) = git(client, root, operation(action)).await? else {
        return Err("unexpected worktree creation result".into());
    };
    if call.verb == "git.pr.checkoutWorktree" {
        let GitReply::Summary(Some(summary)) = git(client, project.id, GitAction::Summary).await?
        else {
            return Err("new worktree branch is unavailable".into());
        };
        Ok(json!({"branch":summary.branch}))
    } else {
        Ok(json!(api::path_text(&project.directory)?))
    }
}

fn resolve_path(path: &str, origin: &str) -> Result<ServerPath, String> {
    let path = if let Some(suffix) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or("home directory is unavailable")?
            .join(suffix)
    } else {
        std::path::Path::new(origin).join(path)
    };
    let path = path.to_str().ok_or("worktree path is not UTF-8")?;
    Ok(ServerPath(path.as_bytes().into()))
}

impl AppModel {
    pub(super) fn switch_extension_worktree(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let identifier = api::text(&call.args, "identifier")?;
        let origin = self
            .state
            .project(call.project)
            .ok_or("project was not found")?;
        let root = origin.parent_id.unwrap_or(origin.id);
        let target = self
            .state
            .projects()
            .iter()
            .filter(|p| p.id == root || p.parent_id == Some(root))
            .find(|p| {
                p.id.to_string() == identifier
                    || p.directory.to_str() == Some(identifier)
                    || p.name == identifier
                    || self
                        .git
                        .projects
                        .get(&p.id)
                        .and_then(|r| r.summary.as_ref())
                        .and_then(|s| s.branch.as_deref())
                        == Some(identifier)
            })
            .map(|p| p.id)
            .ok_or("worktree was not found; refresh worktrees first")?;
        self.state
            .select_project(target)
            .map_err(|error| error.to_string())?;
        // A modal can temporarily switch worktrees while completing an action.
        self.changed(cx);
        self.focus_requested = self.webviews.modal.is_none();
        Ok(Value::Null)
    }
}
