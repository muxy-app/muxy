use crate::git::{self, GitError, GitOptions};
use crate::subprocess::Deadline;
use crate::worktree_hooks::{HookContext, HookOptions, SetupPolicy, run_setup};
use crate::worktree_location::{
    LocationContext, WorktreeLocationError, WorktreeLocationRequest, create_parent, resolve,
    sanitize_component,
};
use muxy_core::store::Project;
use muxy_core::store::worktrees::{Source, Worktree, WorktreeFile, load_file_from, primary};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleWarning {
    Reconciliation(String),
    WorktreeListPersistence(String),
    Setup(String),
    WorkspacePersistence(String),
    ActivePreferencePersistence(String),
    LocationPreferencePersistence(String),
    ReconciledGitRemoval(String),
    PreservedFiles(String),
}

#[derive(Clone, Debug)]
pub struct CreateWorktreeRequest {
    pub project: Project,
    pub name: String,
    pub branch: String,
    pub create_branch: bool,
    pub base_branch: Option<String>,
    pub location: WorktreeLocationRequest,
    pub setup_policy: SetupPolicy,
}

#[derive(Clone, Debug)]
pub struct CreateWorktreeOptions {
    pub git_options: GitOptions,
    pub worktrees_dir: PathBuf,
    pub current_worktrees: Vec<Worktree>,
    pub location_context: LocationContext,
    pub hook_options: HookOptions,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateWorktreeOutcome {
    pub worktree: Worktree,
    pub worktrees: Vec<Worktree>,
    pub warnings: Vec<LifecycleWarning>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateWorktreeError {
    #[error("name and branch are required")]
    InvalidName,
    #[error("Home does not support worktree creation")]
    HomeProject,
    #[error("remote worktree creation is deferred to P12")]
    RemoteProject,
    #[error("project is not an existing local Git repository")]
    NotLocalGitProject,
    #[error("worktree path already exists: {}", .0.display())]
    PathExists(PathBuf),
    #[error(transparent)]
    Location(#[from] WorktreeLocationError),
    #[error(transparent)]
    Git(#[from] GitError),
}

pub fn create_worktree(
    mut request: CreateWorktreeRequest,
    options: &CreateWorktreeOptions,
) -> Result<CreateWorktreeOutcome, CreateWorktreeError> {
    request.name = request.name.trim().to_owned();
    request.branch = request.branch.trim().to_owned();
    request.base_branch = request
        .base_branch
        .map(|branch| branch.trim().to_owned())
        .filter(|branch| !branch.is_empty());
    if request.name.is_empty() || request.branch.is_empty() {
        return Err(CreateWorktreeError::InvalidName);
    }
    if request.project.is_home() {
        return Err(CreateWorktreeError::HomeProject);
    }
    if request.project.is_remote() {
        return Err(CreateWorktreeError::RemoteProject);
    }
    if !request.project.is_git_repo || !Path::new(&request.project.path).is_dir() {
        return Err(CreateWorktreeError::NotLocalGitProject);
    }
    git::validate_branch(&request.branch)?;
    if request.create_branch {
        if let Some(base) = request.base_branch.as_deref() {
            git::validate_branch(base)?;
        }
    } else {
        request.base_branch = None;
    }

    let slug = sanitize_component(&request.name).unwrap_or_else(|_| muxy_core::store::new_uuid());
    let location = resolve(
        &request.project,
        &slug,
        &request.branch,
        request.location,
        &options.location_context,
    )?;
    if location.path.exists() {
        return Err(CreateWorktreeError::PathExists(location.path));
    }
    create_parent(&location)?;

    let deadline = Deadline::new(options.timeout);
    git::add_worktree(
        &options.git_options,
        Path::new(&request.project.path),
        &location.path,
        &request.branch,
        request.create_branch,
        request.base_branch.as_deref(),
        &deadline,
    )?;

    let tracking_error = tracking_error(&options.worktrees_dir, &request.project.id);
    let mut persisted = if options.current_worktrees.is_empty() {
        vec![primary(&request.project.name, &request.project.path)]
    } else {
        options.current_worktrees.clone()
    };
    let created = Worktree {
        id: muxy_core::store::new_uuid(),
        name: request.name,
        path: location.path.to_string_lossy().into_owned(),
        branch: Some(request.branch),
        source: Source::Muxy,
        is_primary: false,
        created_at: muxy_core::store::reference_now(),
        last_active_at: None,
    };
    persisted.push(created.clone());
    let mut warnings = Vec::new();
    let worktrees = match git::list_worktrees(
        &options.git_options,
        Path::new(&request.project.path),
        &deadline,
    ) {
        Ok(records) => crate::worktrees::reconcile_git_records(
            persisted,
            &records,
            &request.project.name,
            &request.project.path,
        ),
        Err(error) => {
            warnings.push(LifecycleWarning::Reconciliation(error.to_string()));
            persisted
        }
    };
    let created = worktrees
        .iter()
        .find(|worktree| worktree.id == created.id)
        .cloned()
        .unwrap_or(created);

    if let Some(error) = tracking_error {
        warnings.push(LifecycleWarning::WorktreeListPersistence(error));
    } else {
        let candidate = crate::worktrees::RefreshCandidate::Updated(worktrees.clone());
        if let Err(error) = crate::worktrees::save_candidate(
            &options.worktrees_dir,
            &request.project.id,
            &candidate,
        ) {
            warnings.push(LifecycleWarning::WorktreeListPersistence(error.to_string()));
        }
    }

    let hook_context = HookContext {
        project_path: PathBuf::from(&request.project.path),
        worktree_id: created.id.clone(),
        worktree_path: PathBuf::from(&created.path),
        worktree_name: created.name.clone(),
        worktree_branch: created.branch.clone(),
    };
    if let Err(error) = run_setup(
        &hook_context,
        request.setup_policy,
        &options.hook_options,
        &deadline,
    ) {
        warnings.push(LifecycleWarning::Setup(error.to_string()));
    }

    Ok(CreateWorktreeOutcome {
        worktree: created,
        worktrees,
        warnings,
    })
}

fn tracking_error(directory: &Path, project_id: &str) -> Option<String> {
    match load_file_from(directory, project_id) {
        Ok(WorktreeFile::Missing | WorktreeFile::Loaded(_)) => None,
        Ok(WorktreeFile::Invalid) => Some("worktree file is malformed".into()),
        Err(error) => Some(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::GitOptions;
    use crate::worktree_config::ProjectHookApproval;
    use crate::worktree_hooks::{HookOptions, SetupPolicy};
    use crate::worktree_location::{LocationContext, WorktreeLocationRequest};
    use muxy_core::store::Project;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    fn run(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn repository(root: &Path) -> Project {
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        run(&repo, &["init", "-q"]);
        run(&repo, &["config", "user.name", "Muxy Test"]);
        run(&repo, &["config", "user.email", "test@muxy.invalid"]);
        std::fs::write(repo.join("README"), b"initial").unwrap();
        run(&repo, &["add", "README"]);
        run(&repo, &["commit", "-qm", "initial"]);
        let mut project = Project::new("Project".into(), repo.to_string_lossy().into_owned(), 0);
        project.id = "PROJECT-ID".into();
        project.is_git_repo = true;
        project
    }

    fn options(root: &Path) -> CreateWorktreeOptions {
        CreateWorktreeOptions {
            git_options: GitOptions {
                executable: PathBuf::from("git"),
                environment: HashMap::new(),
            },
            worktrees_dir: root.join("tracking"),
            current_worktrees: Vec::new(),
            location_context: LocationContext {
                home: root.join("home"),
                profile_worktree_root: root.join("profile/worktree-checkouts"),
                default_path_template: None,
                default_parent_path: None,
            },
            hook_options: HookOptions {
                global_config_path: root.join("xdg/muxy/worktree.json"),
                environment: Vec::new(),
            },
            timeout: Duration::from_secs(10),
        }
    }

    fn request(
        project: &Project,
        name: &str,
        branch: &str,
        path: PathBuf,
    ) -> CreateWorktreeRequest {
        CreateWorktreeRequest {
            project: project.clone(),
            name: name.into(),
            branch: branch.into(),
            create_branch: true,
            base_branch: None,
            location: WorktreeLocationRequest::Explicit(path),
            setup_policy: SetupPolicy::SkipAll,
        }
    }

    #[test]
    fn create_worktree_validates_before_git_and_does_not_remove_after_add_failure() {
        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let target = temp.path().join("invalid");
        let mut invalid = request(&project, "   ", "feature", target.clone());
        assert!(matches!(
            create_worktree(invalid.clone(), &options(temp.path())),
            Err(CreateWorktreeError::InvalidName)
        ));
        invalid.name = "Feature".into();
        invalid.branch = "-invalid".into();
        assert!(matches!(
            create_worktree(invalid, &options(temp.path())),
            Err(CreateWorktreeError::Git(_))
        ));
        assert!(!target.exists());

        let existing = temp.path().join("existing");
        std::fs::create_dir(&existing).unwrap();
        assert!(matches!(
            create_worktree(
                request(&project, "Existing", "existing", existing),
                &options(temp.path())
            ),
            Err(CreateWorktreeError::PathExists(_))
        ));

        let failure_path = temp.path().join("git-failure");
        let mut failure = request(&project, "Failure", "failure", failure_path.clone());
        failure.base_branch = Some("missing-base".into());
        assert!(matches!(
            create_worktree(failure, &options(temp.path())),
            Err(CreateWorktreeError::Git(_))
        ));
        assert!(!failure_path.exists());

        let mut remote = project.clone();
        remote.remote_workspace_id = Some("REMOTE".into());
        assert!(matches!(
            create_worktree(
                request(&remote, "Remote", "remote", temp.path().join("remote")),
                &options(temp.path())
            ),
            Err(CreateWorktreeError::RemoteProject)
        ));
        let home = muxy_core::store::home_project();
        assert!(matches!(
            create_worktree(
                request(&home, "Home", "home", temp.path().join("home-worktree")),
                &options(temp.path())
            ),
            Err(CreateWorktreeError::HomeProject)
        ));
    }

    #[test]
    fn create_worktree_succeeds_with_uppercase_managed_identity_and_skip_all_reads_no_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let target = temp.path().join("feature");
        let sentinel = temp.path().join("setup-ran");
        std::fs::create_dir_all(project.path.clone() + "/.muxy").unwrap();
        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            format!(r#"{{"setup":["touch {}"]}}"#, sentinel.display()),
        )
        .unwrap();
        std::fs::create_dir_all(temp.path().join("xdg/muxy")).unwrap();
        std::fs::write(temp.path().join("xdg/muxy/worktree.json"), "{invalid").unwrap();

        let outcome = create_worktree(
            request(&project, " Feature ", " feature/one ", target.clone()),
            &options(temp.path()),
        )
        .unwrap();

        assert_eq!(outcome.worktree.id, outcome.worktree.id.to_uppercase());
        assert_eq!(outcome.worktree.name, "Feature");
        assert_eq!(outcome.worktree.branch.as_deref(), Some("feature/one"));
        assert_eq!(
            outcome.worktree.source,
            muxy_core::store::worktrees::Source::Muxy
        );
        assert!(target.exists());
        assert!(!sentinel.exists());
        assert!(outcome.warnings.is_empty());
        assert!(
            outcome
                .worktrees
                .iter()
                .any(|worktree| worktree.id == outcome.worktree.id)
        );
    }

    #[test]
    fn create_worktree_keeps_git_success_when_tracking_or_setup_fails() {
        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let blocked = temp.path().join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        let mut blocked_options = options(temp.path());
        blocked_options.worktrees_dir = blocked.join("tracking");
        let tracked = create_worktree(
            request(&project, "Tracked", "tracked", temp.path().join("tracked")),
            &blocked_options,
        )
        .unwrap();
        assert!(
            tracked
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::WorktreeListPersistence(_)))
        );
        assert!(Path::new(&tracked.worktree.path).exists());

        let mut setup = request(&project, "Setup", "setup", temp.path().join("setup"));
        setup.setup_policy = SetupPolicy::NativeApproved(ProjectHookApproval::default());
        let setup_options = options(temp.path());
        std::fs::create_dir_all(temp.path().join("xdg/muxy")).unwrap();
        std::fs::write(&setup_options.hook_options.global_config_path, "{invalid").unwrap();
        let outcome = create_worktree(setup, &setup_options).unwrap();
        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::Setup(_)))
        );
        assert!(Path::new(&outcome.worktree.path).exists());
    }

    #[test]
    fn create_worktree_keeps_unsaved_managed_identity_across_consecutive_creations() {
        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let blocked = temp.path().join("blocked");
        std::fs::write(&blocked, b"file").unwrap();
        let mut first_options = options(temp.path());
        first_options.worktrees_dir = blocked.join("tracking");
        let first = create_worktree(
            request(&project, "First", "first", temp.path().join("first")),
            &first_options,
        )
        .unwrap();

        let mut second_options = first_options;
        second_options.current_worktrees = first.worktrees.clone();
        let second = create_worktree(
            request(&project, "Second", "second", temp.path().join("second")),
            &second_options,
        )
        .unwrap();

        let retained = second
            .worktrees
            .iter()
            .find(|worktree| worktree.id == first.worktree.id)
            .unwrap();
        assert_eq!(retained.source, Source::Muxy);
        assert_eq!(retained.name, "First");
    }

    #[cfg(unix)]
    #[test]
    fn create_worktree_keeps_the_managed_seed_when_post_add_listing_fails() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let wrapper = temp.path().join("git-wrapper");
        std::fs::write(
            &wrapper,
            "#!/bin/sh\ncase \"$*\" in\n  *\"worktree list\"*) exit 9 ;;\n  *) exec git \"$@\" ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut options = options(temp.path());
        options.git_options.executable = wrapper;

        let outcome = create_worktree(
            request(
                &project,
                "Reconcile",
                "reconcile",
                temp.path().join("reconcile"),
            ),
            &options,
        )
        .unwrap();

        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::Reconciliation(_)))
        );
        assert!(
            outcome
                .worktrees
                .iter()
                .any(|worktree| worktree.id == outcome.worktree.id)
        );
        assert!(Path::new(&outcome.worktree.path).exists());
    }

    #[cfg(unix)]
    #[test]
    fn create_worktree_turns_every_setup_failure_into_a_nonrollback_warning() {
        let temp = tempfile::tempdir().unwrap();
        let project = repository(temp.path());
        let project_config = Path::new(&project.path).join(".muxy/worktree.json");
        std::fs::create_dir_all(project_config.parent().unwrap()).unwrap();
        std::fs::write(&project_config, r#"{"setup":["approved"]}"#).unwrap();
        let options = options(temp.path());
        let displayed = crate::worktree_config::resolved_commands(
            crate::worktree_config::HookKind::Setup,
            Path::new(&project.path),
            &options.hook_options.global_config_path,
            true,
        )
        .unwrap();
        let approval = ProjectHookApproval::from_resolved(&displayed);
        std::fs::write(&project_config, r#"{"setup":["changed"]}"#).unwrap();
        let mut changed = request(&project, "Changed", "changed", temp.path().join("changed"));
        changed.setup_policy = SetupPolicy::NativeApproved(approval);
        let changed = create_worktree(changed, &options).unwrap();
        assert!(
            changed
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::Setup(_)))
        );
        assert!(Path::new(&changed.worktree.path).exists());

        std::fs::write(&project_config, r#"{"setup":[]}"#).unwrap();
        std::fs::create_dir_all(options.hook_options.global_config_path.parent().unwrap()).unwrap();
        std::fs::write(
            &options.hook_options.global_config_path,
            r#"{"setup":["exit 7"]}"#,
        )
        .unwrap();
        let mut failed = request(&project, "Failed", "failed", temp.path().join("failed"));
        failed.setup_policy = SetupPolicy::NativeApproved(ProjectHookApproval::default());
        let failed = create_worktree(failed, &options).unwrap();
        assert!(
            failed
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::Setup(_)))
        );
        assert!(Path::new(&failed.worktree.path).exists());

        std::fs::write(
            &options.hook_options.global_config_path,
            r#"{"setup":["sleep 1"]}"#,
        )
        .unwrap();
        let mut timeout_options = options;
        timeout_options.timeout = Duration::from_millis(100);
        let mut timed_out = request(&project, "Timeout", "timeout", temp.path().join("timeout"));
        timed_out.setup_policy = SetupPolicy::NativeApproved(ProjectHookApproval::default());
        let timed_out = create_worktree(timed_out, &timeout_options).unwrap();
        assert!(
            timed_out
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::Setup(_)))
        );
        assert!(Path::new(&timed_out.worktree.path).exists());
    }
}
