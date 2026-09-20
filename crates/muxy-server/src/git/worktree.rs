use super::{Result, error, path, read, run, server_path, validate_branch, validate_member};
use crate::{Registry, catalog::GitReceipt};
use muxy_protocol::{
    GitReply, ProjectDescriptor, ProjectId, ProjectIntent, ProjectKind, ProjectMutation,
    WorktreeAction, WorktreeIntent, WorktreeRemoval,
};
use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;
use std::sync::PoisonError;

impl Registry {
    pub(crate) fn validate_worktree_registration(&self, project: &ProjectDescriptor) -> Result<()> {
        let parent = self.catalog.project(
            project
                .parent_id
                .ok_or_else(|| error("Missing worktree parent"))?,
        )?;
        if parent.home || parent.parent_id.is_some() {
            return Err(error("Invalid worktree parent"));
        }
        if self
            .catalog
            .child_at(parent.id, path(&project.directory))
            .is_some_and(|id| id != project.id)
        {
            return Err(error("Worktree is already registered under this parent"));
        }
        let entry = validate_member(path(&parent.directory), path(&project.directory))?;
        if entry.primary {
            return Err(error("Cannot register the primary worktree as a child"));
        }
        Ok(())
    }

    pub(super) fn inspect_removal(&self, project: &ProjectDescriptor) -> Result<WorktreeRemoval> {
        if project.kind != Some(ProjectKind::Worktree) {
            return Err(error("Only a child worktree can be removed from disk"));
        }
        let parent = self.catalog.project(
            project
                .parent_id
                .ok_or_else(|| error("Missing worktree parent"))?,
        )?;
        let target = path(&project.directory);
        let entry = validate_member(path(&parent.directory), target)?;
        if entry.primary || entry.locked {
            return Err(error("Primary or locked worktrees cannot be removed"));
        }
        let metadata = std::fs::symlink_metadata(target).map_err(error)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(error("Worktree directory was replaced"));
        }
        let status = read::status(target)?;
        let summary = read::summary(target)?;
        Ok(WorktreeRemoval {
            directory: project.directory.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
            dirty: !status.is_empty(),
            status,
            head: summary.head,
            branch: summary.branch,
        })
    }

    pub(super) fn git_worktree(
        &self,
        owner: ProjectId,
        intent: &WorktreeIntent,
    ) -> Result<GitReply> {
        let token = self.git.operations.token(intent.operation);
        let _token = token.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = self.catalog.git_receipt(intent.operation);
        if let Some(receipt) = &previous {
            self.catalog.ensure_durable()?;
            if receipt.owner != owner || receipt.intent != *intent {
                return Err(error(
                    "Operation token reused with different worktree request",
                ));
            }
            if let Some(error) = &receipt.failed {
                return Err(crate::ServerError::new(error.code, &error.message));
            }
            if let Some(reply) = &receipt.reply {
                return Ok(reply.clone());
            }
        }
        let parent = self.catalog.project(owner).or_else(|error| {
            previous
                .as_ref()
                .filter(|receipt| {
                    receipt.applied && matches!(intent.action, WorktreeAction::Remove { .. })
                })
                .map(|receipt| receipt.project.clone())
                .ok_or(error)
        })?;
        let repository = if matches!(intent.action, WorktreeAction::Remove { .. }) {
            self.catalog.project(
                parent
                    .parent_id
                    .ok_or_else(|| error("Missing worktree parent"))?,
            )?
        } else {
            parent.clone()
        };
        let lock = self.git.lock_for(path(&repository.directory))?;
        let _guard = lock.write().unwrap_or_else(PoisonError::into_inner);
        let (project, removal) = match &intent.action {
            WorktreeAction::Create { project, .. }
            | WorktreeAction::Register { project, .. }
            | WorktreeAction::CheckoutPullRequest { project, .. } => (*project, None),
            WorktreeAction::Remove { .. } => (owner, Some(path(&parent.directory).to_owned())),
        };
        let _reservation = {
            let _operation = self.session_operation();
            self.catalog.project(repository.id)?;
            if previous.as_ref().is_none_or(|receipt| !receipt.applied) {
                self.catalog.project(owner)?;
            }
            self.git
                .operations
                .reserve(vec![owner, repository.id, project], removal)?
        };
        if let Some(receipt) = previous {
            return self.finish_worktree_attempt(receipt);
        }
        let receipt = self.prepare_worktree_receipt(owner, intent, &parent)?;
        self.catalog.save_git(receipt.clone())?;
        self.finish_worktree_attempt(receipt)
    }

    fn prepare_worktree_receipt(
        &self,
        owner: ProjectId,
        intent: &WorktreeIntent,
        parent: &ProjectDescriptor,
    ) -> Result<GitReceipt> {
        let receipt = match &intent.action {
            WorktreeAction::CheckoutPullRequest {
                project,
                directory,
                number,
            } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                if path(directory).try_exists().map_err(error)? {
                    return Err(error("Worktree directory already exists"));
                }
                let branch = self
                    .git
                    .github
                    .prepare_worktree(path(&parent.directory), *number)?;
                std::fs::create_dir(path(directory)).map_err(error)?;
                Self::new_worktree_receipt(owner, intent, parent, *project, directory, &branch)?
            }
            WorktreeAction::Create {
                project,
                directory,
                branch,
                base,
            } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                validate_branch(path(&parent.directory), branch)?;
                if let Some(base) = base {
                    if base.starts_with('-') {
                        return Err(error("Invalid base branch"));
                    }
                    run(
                        path(&parent.directory),
                        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
                    )?;
                }
                std::fs::create_dir(path(directory)).map_err(error)?;
                Self::new_worktree_receipt(owner, intent, parent, *project, directory, branch)?
            }
            WorktreeAction::Register { project, directory } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                let entry = validate_member(path(&parent.directory), path(directory))?;
                if entry.primary {
                    return Err(error("The primary worktree is already the parent project"));
                }
                Self::new_worktree_receipt(
                    owner,
                    intent,
                    parent,
                    *project,
                    directory,
                    entry.branch.as_deref().unwrap_or("Detached worktree"),
                )?
            }
            WorktreeAction::Remove { expected } => {
                if self.inspect_removal(parent)? != *expected {
                    return Err(error("Worktree changed; inspect and confirm removal again"));
                }
                GitReceipt {
                    owner,
                    intent: intent.clone(),
                    project: parent.clone(),
                    device: expected.device,
                    inode: expected.inode,
                    applied: false,
                    failed: None,
                    reply: None,
                }
            }
        };
        Ok(receipt)
    }

    fn validate_worktree_parent(
        &self,
        parent: &ProjectDescriptor,
        id: ProjectId,
        directory: &muxy_protocol::ServerPath,
    ) -> Result<()> {
        if parent.home || parent.parent_id.is_some() || parent.kind.is_some() {
            return Err(error("Worktrees require an ordinary top-level parent"));
        }
        if !path(directory).is_absolute() {
            return Err(error("Worktree directory must be absolute"));
        }
        if self.catalog.project(id).is_ok()
            || self.catalog.child_at(parent.id, path(directory)).is_some()
        {
            return Err(error("Worktree is already registered"));
        }
        Ok(())
    }

    fn new_worktree_receipt(
        owner: ProjectId,
        intent: &WorktreeIntent,
        parent: &ProjectDescriptor,
        id: ProjectId,
        directory: &muxy_protocol::ServerPath,
        name: &str,
    ) -> Result<GitReceipt> {
        let canonical = path(directory).canonicalize().map_err(error)?;
        let meta = std::fs::symlink_metadata(&canonical).map_err(error)?;
        let project = ProjectDescriptor {
            id,
            directory: server_path(&canonical),
            home: false,
            name: name.into(),
            icon: None,
            color: parent.color.clone(),
            kind: Some(ProjectKind::Worktree),
            parent_id: Some(parent.id),
        };
        project
            .validate()
            .map_err(|_| error("Invalid worktree project"))?;
        Ok(GitReceipt {
            owner,
            intent: intent.clone(),
            project,
            device: meta.dev(),
            inode: meta.ino(),
            applied: false,
            failed: None,
            reply: None,
        })
    }

    fn finish_worktree_attempt(&self, receipt: GitReceipt) -> Result<GitReply> {
        let result = self.apply_worktree(receipt.clone());
        if let Err(e) = &result {
            if e.code() == muxy_protocol::ErrorCode::PersistenceFailed {
                return result;
            }
            let mut current = self
                .catalog
                .git_receipt(receipt.intent.operation)
                .unwrap_or(receipt);
            if !current.applied
                && matches!(
                    current.intent.action,
                    WorktreeAction::Create { .. } | WorktreeAction::CheckoutPullRequest { .. }
                )
            {
                let target = path(&current.project.directory);
                if std::fs::symlink_metadata(target)
                    .is_ok_and(|m| m.dev() == current.device && m.ino() == current.inode)
                {
                    let _ = std::fs::remove_dir(target);
                }
            }
            if matches!(current.intent.action, WorktreeAction::Remove { .. })
                && !path(&current.project.directory)
                    .try_exists()
                    .map_err(error)?
            {
                return result;
            }
            current.failed = Some(e.to_reply());
            self.catalog.save_git(current)?;
        }
        result
    }

    fn apply_worktree(&self, mut receipt: GitReceipt) -> Result<GitReply> {
        let removing = matches!(receipt.intent.action, WorktreeAction::Remove { .. });
        let parent_id = if removing {
            receipt
                .project
                .parent_id
                .ok_or_else(|| error("Missing worktree parent"))?
        } else {
            receipt.owner
        };
        let parent = self.catalog.project(parent_id)?;
        let repository = path(&parent.directory);
        let target = path(&receipt.project.directory);
        if !receipt.applied {
            let exists = target.try_exists().map_err(error)?;
            if exists {
                let meta = std::fs::symlink_metadata(target).map_err(error)?;
                if meta.dev() != receipt.device || meta.ino() != receipt.inode || !meta.is_dir() {
                    return Err(error(
                        "Worktree directory identity changed; files were preserved",
                    ));
                }
            } else if !removing {
                return Err(error("Reserved worktree directory is missing"));
            }
            match &receipt.intent.action {
                WorktreeAction::CheckoutPullRequest { .. } => {
                    create_worktree(repository, &receipt, &receipt.project.name, None)?;
                }
                WorktreeAction::Create { branch, base, .. } => {
                    create_worktree(repository, &receipt, branch, base.as_deref())?;
                }
                WorktreeAction::Register { .. } => {
                    validate_member(repository, target)?;
                }
                WorktreeAction::Remove { expected } => {
                    if exists {
                        if self.inspect_removal(&receipt.project)? != *expected {
                            return Err(error(
                                "Worktree changed; files were preserved. Inspect removal again",
                            ));
                        }
                        self.stop_worktree_sessions(&receipt.project)?;
                        super::processes::stop(target)?;
                        if self.inspect_removal(&receipt.project)? != *expected {
                            return Err(error(
                                "Worktree changed while stopping sessions; inspect removal again",
                            ));
                        }
                        let args = [
                            OsString::from("worktree"),
                            "remove".into(),
                            "--force".into(),
                            "--".into(),
                            target.as_os_str().to_owned(),
                        ];
                        run(repository, &args)?;
                    } else if let Some(entry) = read::worktrees(repository)?
                        .into_iter()
                        .find(|w| w.directory == receipt.project.directory)
                    {
                        if entry.primary
                            || entry.locked
                            || entry.branch != expected.branch
                            || entry.head != expected.head
                        {
                            return Err(error("Git worktree registration changed during removal"));
                        }
                        let args = [
                            OsString::from("worktree"),
                            "remove".into(),
                            "--force".into(),
                            "--".into(),
                            target.as_os_str().to_owned(),
                        ];
                        run(repository, &args)?;
                    }
                }
            }
            receipt.applied = true;
            self.catalog.save_git(receipt.clone())?;
        }
        if removing {
            let intent = ProjectIntent {
                operation: receipt.intent.operation,
                mutation: ProjectMutation::Delete(receipt.project.id),
            };
            if !self.catalog.begin_mutation(&intent)? {
                self.resume_cleanup()?;
            }
            receipt.reply = Some(GitReply::Done);
            self.catalog.save_git(receipt)?;
            Ok(GitReply::Done)
        } else {
            self.catalog.finish_git_project(&receipt)?;
            Ok(GitReply::Project(receipt.project))
        }
    }

    fn stop_worktree_sessions(&self, project: &ProjectDescriptor) -> Result<()> {
        let target = path(&project.directory);
        for session in self.catalog.owned(project.id) {
            self.discard_owned(session)?;
        }
        for session in self.list() {
            if self
                .catalog
                .project(session.project)
                .is_ok_and(|p| path(&p.directory).canonicalize().ok() == target.canonicalize().ok())
            {
                self.end(session.id)?;
            }
        }
        Ok(())
    }

    pub(crate) fn resume_git(&self) {
        for receipt in self.catalog.pending_git() {
            if let Err(error) = self.git_worktree(receipt.owner, &receipt.intent) {
                log::warn!("Worktree recovery needs attention: {error}");
            }
        }
    }
}

fn create_worktree(
    repository: &std::path::Path,
    receipt: &GitReceipt,
    branch: &str,
    base: Option<&str>,
) -> Result<()> {
    let target = path(&receipt.project.directory);
    if let Ok(entry) = validate_member(repository, target) {
        if entry.primary || entry.branch.as_deref() != Some(branch) {
            return Err(error("Worktree changed during creation"));
        }
    } else {
        let mut args: Vec<OsString> = vec!["worktree".into(), "add".into()];
        if base.is_some() {
            args.extend(["-b".into(), branch.into()]);
        }
        args.push("--".into());
        args.push(target.as_os_str().to_owned());
        args.push(base.unwrap_or(branch).into());
        if let Err(e) = run(repository, &args)
            && !validate_member(repository, target)
                .is_ok_and(|entry| !entry.primary && entry.branch.as_deref() == Some(branch))
        {
            return Err(e);
        }
        let meta = std::fs::symlink_metadata(target).map_err(error)?;
        if meta.dev() != receipt.device || meta.ino() != receipt.inode {
            return Err(error("Worktree directory was replaced during creation"));
        }
    }
    Ok(())
}
