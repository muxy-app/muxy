use super::{Result, error, path, read, run, server_path, validate_branch, validate_member};
use crate::{Registry, catalog::GitReceipt};
use muxy_core::files::FileId;
use muxy_protocol::{
    GitReply, ProjectDescriptor, ProjectId, ProjectIntent, ProjectKind, ProjectMutation,
    WorktreeAction, WorktreeIntent, WorktreeRemoval,
};
use std::ffi::{OsStr, OsString};
use std::path::Path;
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
        let directory = path(&project.directory)?;
        if self
            .catalog
            .child_at(parent.id, &directory)
            .is_some_and(|id| id != project.id)
        {
            return Err(error("Worktree is already registered under this parent"));
        }
        let entry = validate_member(&path(&parent.directory)?, &directory)?;
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
        let target = &path(&project.directory)?;
        let entry = validate_member(&path(&parent.directory)?, target)?;
        if entry.primary || entry.locked {
            return Err(error("Primary or locked worktrees cannot be removed"));
        }
        let metadata = std::fs::symlink_metadata(target).map_err(error)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(error("Worktree directory was replaced"));
        }
        let identity = FileId::of_link(target).map_err(error)?;
        let status = read::status(target)?;
        let summary = read::summary(target)?;
        Ok(WorktreeRemoval {
            directory: project.directory.clone(),
            device: identity.device(),
            inode: identity.index(),
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
        let lock = self.git.lock_for(&path(&repository.directory)?)?;
        let _guard = lock.write().unwrap_or_else(PoisonError::into_inner);
        let (project, removal) = match &intent.action {
            WorktreeAction::Create { project, .. }
            | WorktreeAction::Register { project, .. }
            | WorktreeAction::CheckoutPullRequest { project, .. } => (*project, None),
            WorktreeAction::Remove { .. } => (owner, Some(path(&parent.directory)?)),
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
        if let Some(approved) = intent
            .options
            .as_ref()
            .and_then(|options| options.hooks.as_ref())
        {
            let source = if let Some(parent) = parent.parent_id {
                self.catalog.project(parent)?
            } else {
                parent.clone()
            };
            super::hooks::approved(
                &path(&source.directory)?,
                self.git.hook_config.as_deref(),
                matches!(intent.action, WorktreeAction::Remove { .. }),
                approved,
            )?;
        }
        let receipt = match &intent.action {
            WorktreeAction::CheckoutPullRequest {
                project,
                directory,
                number,
            } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                if path(directory)?.try_exists().map_err(error)? {
                    return Err(error("Worktree directory already exists"));
                }
                let branch = self
                    .git
                    .github
                    .prepare_worktree(&path(&parent.directory)?, *number)?;
                reserve_folder(directory)?;
                Self::new_worktree_receipt(owner, intent, parent, *project, directory, &branch)?
            }
            WorktreeAction::Create {
                project,
                directory,
                branch,
                base,
            } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                validate_branch(&path(&parent.directory)?, branch)?;
                if let Some(base) = base {
                    if base.starts_with('-') {
                        return Err(error("Invalid base branch"));
                    }
                    run(
                        &path(&parent.directory)?,
                        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
                    )?;
                }
                reserve_folder(directory)?;
                Self::new_worktree_receipt(owner, intent, parent, *project, directory, branch)?
            }
            WorktreeAction::Register { project, directory } => {
                self.validate_worktree_parent(parent, *project, directory)?;
                let parent_directory = path(&parent.directory)?;
                let directory_path = path(directory)?;
                let entry = validate_member(&parent_directory, &directory_path)?;
                if entry.primary || same_folder(&parent_directory, &directory_path) {
                    return Err(error("This worktree is already the parent project"));
                }
                let folder = directory_path.file_name().map(OsStr::to_string_lossy);
                Self::new_worktree_receipt(
                    owner,
                    intent,
                    parent,
                    *project,
                    directory,
                    entry
                        .branch
                        .as_deref()
                        .or(folder.as_deref())
                        .unwrap_or("Detached worktree"),
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
                    hooks_started: false,
                    hooks_finished: false,
                    hook_error: None,
                    applied: false,
                    failed: None,
                    reply: None,
                    set_aside: None,
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
        let directory = path(directory)?;
        if !directory.is_absolute() {
            return Err(error("Worktree directory must be absolute"));
        }
        if self.catalog.project(id).is_ok()
            || self.catalog.child_at(parent.id, &directory).is_some()
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
        let canonical = muxy_core::files::canonicalize(&path(directory)?).map_err(error)?;
        let identity = FileId::of_link(&canonical).map_err(error)?;
        let project = ProjectDescriptor {
            id,
            directory: server_path(&canonical)?,
            home: false,
            name: intent
                .options
                .as_ref()
                .and_then(|options| options.name.as_deref())
                .unwrap_or(name)
                .into(),
            icon: None,
            logo: None,
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
            device: identity.device(),
            inode: identity.index(),
            hooks_started: false,
            hooks_finished: false,
            hook_error: None,
            applied: false,
            failed: None,
            reply: None,
            set_aside: None,
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
                let target = path(&current.project.directory)?;
                if same_identity(&target, current.device, current.inode) {
                    let _ = std::fs::remove_dir(target);
                }
            }
            if matches!(current.intent.action, WorktreeAction::Remove { .. })
                && !path(&current.project.directory)?
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

    #[allow(
        clippy::too_many_lines,
        reason = "Apply and durably record the worktree lifecycle in order"
    )]
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
        let repository = &path(&parent.directory)?;
        let target = &path(&receipt.project.directory)?;
        if !receipt.applied {
            let exists = target.try_exists().map_err(error)?;
            if exists {
                let meta = std::fs::symlink_metadata(target).map_err(error)?;
                if !same_identity(target, receipt.device, receipt.inode) || !meta.is_dir() {
                    return Err(error(
                        "Worktree directory identity changed; files were preserved",
                    ));
                }
            } else if !removing {
                return Err(error("Reserved worktree directory is missing"));
            }
            match &receipt.intent.action.clone() {
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
                        #[cfg(unix)]
                        super::processes::stop(target)?;
                        #[cfg(windows)]
                        self.end_sessions_inside(target)?;
                        if self.inspect_removal(&receipt.project)? != *expected {
                            return Err(error(
                                "Worktree changed while stopping sessions; inspect removal again",
                            ));
                        }
                        // Refuse a busy worktree before its teardown hooks run.
                        #[cfg(windows)]
                        super::processes::check_unused(
                            target,
                            &super::processes::aside_path(target, receipt.intent.operation)?,
                        )?;
                        self.run_worktree_hooks(&mut receipt, repository, true)?;
                        let current = self.inspect_removal(&receipt.project)?;
                        if current.device != expected.device
                            || current.inode != expected.inode
                            || current.head != expected.head
                            || current.branch != expected.branch
                        {
                            return Err(error(
                                "Worktree identity changed during teardown; files were preserved",
                            ));
                        }
                        #[cfg(unix)]
                        remove_registered(repository, target)?;
                        #[cfg(windows)]
                        self.delete_worktree_folder(&mut receipt, repository, target)?;
                    } else if let Some(entry) = read::worktrees(repository)?.into_iter().find(|w| {
                        path(&w.directory)
                            .is_ok_and(|directory| muxy_core::paths::path_eq(&directory, target))
                    }) {
                        if entry.primary
                            || entry.locked
                            || entry.branch != expected.branch
                            || entry.head != expected.head
                        {
                            return Err(error("Git worktree registration changed during removal"));
                        }
                        remove_registered(repository, target)?;
                    }
                    #[cfg(windows)]
                    if !exists {
                        finish_deleting_set_aside(&receipt)?;
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
            if let Err(cause) = self.run_worktree_hooks(&mut receipt, repository, false) {
                if cause.code() == muxy_protocol::ErrorCode::PersistenceFailed {
                    return Err(cause);
                }
                receipt.hook_error = Some(cause.to_string());
            }
            self.catalog.finish_git_project(&receipt)?;
            Ok(receipt.hook_error.map_or_else(
                || GitReply::Project(receipt.project.clone()),
                |message| GitReply::WorktreeSetupFailed {
                    project: receipt.project.clone(),
                    message,
                },
            ))
        }
    }

    fn run_worktree_hooks(
        &self,
        receipt: &mut GitReceipt,
        repository: &Path,
        teardown: bool,
    ) -> Result<()> {
        let Some(hooks) = receipt
            .intent
            .options
            .as_ref()
            .and_then(|options| options.hooks.clone())
        else {
            return Ok(());
        };
        if receipt.hooks_finished {
            return Ok(());
        }
        if receipt.hooks_started {
            return Err(error(
                "Worktree hooks were interrupted and were not run again. Review the worktree before retrying.",
            ));
        }
        super::hooks::approved(
            repository,
            self.git.hook_config.as_deref(),
            teardown,
            &hooks,
        )?;
        receipt.hooks_started = true;
        self.catalog.save_git(receipt.clone())?;
        let branch = read::summary(&path(&receipt.project.directory)?)?
            .branch
            .unwrap_or_default();
        super::hooks::run(repository, &receipt.project, &branch, &hooks)?;
        receipt.hooks_finished = true;
        self.catalog.save_git(receipt.clone())
    }

    fn stop_worktree_sessions(&self, project: &ProjectDescriptor) -> Result<()> {
        let target = path(&project.directory)?;
        for session in self.catalog.owned(project.id) {
            self.discard_owned(session)?;
        }
        for session in self.list() {
            if self
                .catalog
                .project(session.project)
                .is_ok_and(|p| path(&p.directory).is_ok_and(|p| same_folder(&p, &target)))
            {
                self.end(session.id)?;
            }
        }
        Ok(())
    }

    /// Ends this server's sessions whose shell reported a folder inside the
    /// worktree. Windows can't read another process's working directory.
    #[cfg(windows)]
    fn end_sessions_inside(&self, target: &Path) -> Result<()> {
        let target = muxy_core::files::canonicalize(target).map_err(error)?;
        for session in self.list() {
            let inside = self.handle(session.id).is_some_and(|handle| {
                path(&handle.metadata().directory)
                    .is_ok_and(|directory| muxy_core::paths::path_starts_with(&directory, &target))
            });
            if inside {
                self.end(session.id)?;
            }
        }
        Ok(())
    }

    /// Sets the folder aside, which fails while a file in it is open, records
    /// where, deletes it, and has Git forget the worktree. Until the files
    /// are gone the folder goes back on failure, as when a running program's
    /// file can't be deleted.
    #[cfg(windows)]
    fn delete_worktree_folder(
        &self,
        receipt: &mut GitReceipt,
        repository: &Path,
        target: &Path,
    ) -> Result<()> {
        let aside = super::processes::aside_path(target, receipt.intent.operation)?;
        // Recorded first, so recovery finds the folder after any crash.
        receipt.set_aside = Some(server_path(&aside)?);
        self.catalog.save_git(receipt.clone())?;
        if let Err(cause) = super::processes::set_aside(target, &aside) {
            receipt.set_aside = None;
            self.catalog.save_git(receipt.clone())?;
            return Err(cause);
        }
        if let Err(cause) = super::processes::delete_set_aside(&aside) {
            super::processes::restore(&aside, target)?;
            receipt.set_aside = None;
            self.catalog.save_git(receipt.clone())?;
            return Err(error(format!(
                "Some worktree files could not be deleted; the rest were preserved: {cause}"
            )));
        }
        remove_registered(repository, target)
    }

    pub(crate) fn resume_git(&self) {
        for receipt in self.catalog.pending_git() {
            if let Err(error) = self.git_worktree(receipt.owner, &receipt.intent) {
                log::warn!("Worktree recovery needs attention: {error}");
            }
        }
    }
}

/// Has Git delete the worktree and its registration. With the folder already
/// gone, Git only forgets it.
fn remove_registered(repository: &Path, target: &Path) -> Result<()> {
    let args = [
        OsString::from("worktree"),
        "remove".into(),
        "--force".into(),
        "--".into(),
        target.as_os_str().to_owned(),
    ];
    run(repository, &args).map(drop)
}

/// Deletes what a removal interrupted after setting the folder aside left.
#[cfg(windows)]
fn finish_deleting_set_aside(receipt: &GitReceipt) -> Result<()> {
    let Some(aside) = &receipt.set_aside else {
        return Ok(());
    };
    match super::processes::delete_set_aside(&path(aside)?) {
        Err(cause) if cause.kind() != std::io::ErrorKind::NotFound => Err(error(cause)),
        _ => Ok(()),
    }
}

/// Whether `path` is still the folder a receipt recorded.
fn same_identity(path: &Path, device: u64, inode: u64) -> bool {
    FileId::of_link(path).is_ok_and(|id| id.device() == device && id.index() == inode)
}

/// Creates the empty folder a new worktree fills, refusing one that already exists.
fn reserve_folder(directory: &muxy_protocol::ServerPath) -> Result<()> {
    let directory = path(directory)?;
    if let Some(parent) = directory.parent() {
        std::fs::create_dir_all(parent).map_err(error)?;
    }
    std::fs::create_dir(&directory).map_err(|cause| {
        if cause.kind() == std::io::ErrorKind::AlreadyExists {
            error("Worktree directory already exists")
        } else {
            error(cause)
        }
    })
}

fn same_folder(one: &Path, other: &Path) -> bool {
    muxy_core::files::canonicalize(one).is_ok_and(|one| {
        muxy_core::files::canonicalize(other)
            .is_ok_and(|other| muxy_core::paths::path_eq(&one, &other))
    })
}

fn create_worktree(
    repository: &Path,
    receipt: &GitReceipt,
    branch: &str,
    base: Option<&str>,
) -> Result<()> {
    let target = &path(&receipt.project.directory)?;
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
        if !same_identity(target, receipt.device, receipt.inode) {
            return Err(error("Worktree directory was replaced during creation"));
        }
    }
    Ok(())
}
