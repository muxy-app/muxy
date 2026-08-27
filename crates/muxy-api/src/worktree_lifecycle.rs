use crate::git::{self, GitError, GitOptions};
use crate::subprocess::Deadline;
use crate::worktree_config::{HookKind, ProjectHookApproval, ResolvedCommand};
use crate::worktree_hooks::{HookContext, HookOptions, SetupPolicy, run_setup, run_teardown};
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

#[derive(Clone, Debug)]
pub struct RemoveWorktreeRequest {
    pub project: Project,
    pub worktree: Worktree,
    pub project_hook_approval: Option<ProjectHookApproval>,
}

#[derive(Clone, Debug)]
pub struct RemoveWorktreeOptions {
    pub git_options: GitOptions,
    pub worktrees_dir: PathBuf,
    pub current_worktrees: Vec<Worktree>,
    pub hook_options: HookOptions,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemoveWorktreeOutcome {
    pub removed: Worktree,
    pub worktrees: Vec<Worktree>,
    pub files_preserved: bool,
    pub warnings: Vec<LifecycleWarning>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemovalInspection {
    pub project_id: String,
    pub worktree: Worktree,
    pub dirty: bool,
    pub teardown_commands: Vec<ResolvedCommand>,
    pub inspection_diagnostic: Option<String>,
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

#[derive(Debug, thiserror::Error)]
pub enum RemoveWorktreeError {
    #[error("Home does not support worktree removal")]
    HomeProject,
    #[error("remote worktree removal is deferred to P12")]
    RemoteProject,
    #[error("project is not a local Git project")]
    NotLocalGitProject,
    #[error("the primary worktree cannot be removed")]
    PrimaryWorktree,
    #[error("worktree no longer matches current project state")]
    StaleWorktree,
    #[error(transparent)]
    Hook(#[from] crate::worktree_hooks::WorktreeHookError),
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

pub fn inspect_worktree_removal(
    project: &Project,
    worktree: &Worktree,
    options: &RemoveWorktreeOptions,
) -> Result<RemovalInspection, RemoveWorktreeError> {
    let worktree = validated_removal_target(project, worktree, &options.current_worktrees)?;
    let deadline = Deadline::new(options.timeout);
    let mut diagnostics = Vec::new();
    let dirty =
        match git::is_worktree_dirty(&options.git_options, Path::new(&worktree.path), &deadline) {
            Ok(dirty) => dirty,
            Err(error) => {
                diagnostics.push(error.to_string());
                false
            }
        };
    let teardown_commands = if worktree.source == Source::Muxy {
        match crate::worktree_config::resolved_commands(
            HookKind::Teardown,
            Path::new(&project.path),
            &options.hook_options.global_config_path,
            true,
        ) {
            Ok(commands) => commands,
            Err(error) => {
                diagnostics.push(error.to_string());
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    Ok(RemovalInspection {
        project_id: project.id.clone(),
        worktree,
        dirty,
        teardown_commands,
        inspection_diagnostic: (!diagnostics.is_empty()).then(|| diagnostics.join("\n")),
    })
}

pub fn remove_worktree(
    request: RemoveWorktreeRequest,
    options: &RemoveWorktreeOptions,
) -> Result<RemoveWorktreeOutcome, RemoveWorktreeError> {
    let worktree = validated_removal_target(
        &request.project,
        &request.worktree,
        &options.current_worktrees,
    )?;
    let mut warnings = Vec::new();
    let repo_path = Path::new(&request.project.path);
    let worktree_path = Path::new(&worktree.path);
    let files_preserved = !repo_path.is_dir();
    if files_preserved {
        warnings.push(LifecycleWarning::PreservedFiles(worktree.path.clone()));
    } else {
        let deadline = Deadline::new(options.timeout);
        if worktree.source == Source::Muxy {
            run_teardown(
                &hook_context(&request.project, &worktree),
                request.project_hook_approval.as_ref(),
                &options.hook_options,
                &deadline,
            )?;
        }
        let removal =
            git::remove_worktree(&options.git_options, repo_path, worktree_path, &deadline)?;
        if removal.reconciled {
            warnings.push(LifecycleWarning::ReconciledGitRemoval(
                "Git reported failure after the checkout was removed".into(),
            ));
        }
        remove_empty_parent(worktree_path);
    }
    let worktrees = options
        .current_worktrees
        .iter()
        .filter(|candidate| !candidate.id.eq_ignore_ascii_case(&worktree.id))
        .cloned()
        .collect::<Vec<_>>();
    let candidate = crate::worktrees::RefreshCandidate::Updated(worktrees.clone());
    if let Some(error) = tracking_error(&options.worktrees_dir, &request.project.id) {
        warnings.push(LifecycleWarning::WorktreeListPersistence(error));
    } else if let Err(error) =
        crate::worktrees::save_candidate(&options.worktrees_dir, &request.project.id, &candidate)
    {
        warnings.push(LifecycleWarning::WorktreeListPersistence(error.to_string()));
    }
    Ok(RemoveWorktreeOutcome {
        removed: worktree,
        worktrees,
        files_preserved,
        warnings,
    })
}

fn validated_removal_target(
    project: &Project,
    requested: &Worktree,
    current: &[Worktree],
) -> Result<Worktree, RemoveWorktreeError> {
    if project.is_home() {
        return Err(RemoveWorktreeError::HomeProject);
    }
    if project.is_remote() {
        return Err(RemoveWorktreeError::RemoteProject);
    }
    if !project.is_git_repo {
        return Err(RemoveWorktreeError::NotLocalGitProject);
    }
    let Some(current) = current
        .iter()
        .find(|candidate| candidate.id.eq_ignore_ascii_case(&requested.id))
    else {
        return Err(RemoveWorktreeError::StaleWorktree);
    };
    if current.path != requested.path
        || current.source != requested.source
        || current.is_primary != requested.is_primary
    {
        return Err(RemoveWorktreeError::StaleWorktree);
    }
    if !project.can_remove_worktree(current)
        || git::canonical_path(Path::new(&current.path))
            == git::canonical_path(Path::new(&project.path))
    {
        return Err(RemoveWorktreeError::PrimaryWorktree);
    }
    Ok(current.clone())
}

fn hook_context(project: &Project, worktree: &Worktree) -> HookContext {
    HookContext {
        project_path: PathBuf::from(&project.path),
        worktree_id: worktree.id.clone(),
        worktree_path: PathBuf::from(&worktree.path),
        worktree_name: worktree.name.clone(),
        worktree_branch: worktree.branch.clone(),
    }
}

fn remove_empty_parent(worktree_path: &Path) {
    let Some(parent) = worktree_path.parent() else {
        return;
    };
    if parent
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_none())
    {
        let _ = std::fs::remove_dir(parent);
    }
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

    #[test]
    fn remove_worktree_contract_requires_inspection_and_disk_first_outcomes() {
        let _: Option<RemoveWorktreeRequest> = None;
        let _: Option<RemovalInspection> = None;
        let _: Option<RemoveWorktreeOutcome> = None;
        let _ = inspect_worktree_removal;
        let _ = remove_worktree;
    }

    fn verification_temp() -> tempfile::TempDir {
        let root = Path::new("target/test-verification");
        std::fs::create_dir_all(root).unwrap();
        tempfile::Builder::new()
            .prefix("p8.")
            .tempdir_in(root)
            .unwrap()
    }

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

    fn removal_options(root: &Path, current_worktrees: Vec<Worktree>) -> RemoveWorktreeOptions {
        let options = options(root);
        RemoveWorktreeOptions {
            git_options: options.git_options,
            worktrees_dir: options.worktrees_dir,
            current_worktrees,
            hook_options: options.hook_options,
            timeout: options.timeout,
        }
    }

    fn created_secondary(
        root: &Path,
        project: &Project,
        parent: &str,
        branch: &str,
    ) -> CreateWorktreeOutcome {
        create_worktree(
            request(project, branch, branch, root.join(parent).join("checkout")),
            &options(root),
        )
        .unwrap()
    }

    #[test]
    fn remove_worktree_inspection_refuses_primary_and_models_dirty_hooks_and_diagnostics() {
        let temp = verification_temp();
        let project = repository(temp.path());
        let created = created_secondary(temp.path(), &project, "inspection", "inspection");
        let worktree = created.worktree.clone();
        let initial_options = removal_options(temp.path(), created.worktrees.clone());
        assert!(
            !inspect_worktree_removal(&project, &worktree, &initial_options)
                .unwrap()
                .dirty
        );
        std::fs::write(Path::new(&worktree.path).join("dirty"), b"dirty").unwrap();
        std::fs::create_dir_all(Path::new(&project.path).join(".muxy")).unwrap();
        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            r#"{"teardown":[{"command":"true","name":"Project cleanup"}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(temp.path().join("xdg/muxy")).unwrap();
        std::fs::write(
            temp.path().join("xdg/muxy/worktree.json"),
            r#"{"teardown":[{"command":"true","name":"Machine cleanup"}]}"#,
        )
        .unwrap();
        let options = removal_options(temp.path(), created.worktrees.clone());

        let inspection = inspect_worktree_removal(&project, &worktree, &options).unwrap();
        assert!(inspection.dirty);
        assert!(inspection.inspection_diagnostic.is_none());
        assert_eq!(inspection.teardown_commands.len(), 2);
        assert_eq!(
            inspection.teardown_commands[0].source,
            crate::worktree_config::CommandSource::Project
        );
        assert_eq!(
            inspection.teardown_commands[1].source,
            crate::worktree_config::CommandSource::Global
        );

        let primary = created
            .worktrees
            .iter()
            .find(|candidate| candidate.is_primary)
            .unwrap();
        assert!(matches!(
            inspect_worktree_removal(&project, primary, &options),
            Err(RemoveWorktreeError::PrimaryWorktree)
        ));
        let mut disguised_primary = worktree.clone();
        disguised_primary.path.clone_from(&project.path);
        let mut disguised_options = options.clone();
        disguised_options
            .current_worktrees
            .iter_mut()
            .find(|candidate| candidate.id == disguised_primary.id)
            .unwrap()
            .path
            .clone_from(&project.path);
        assert!(matches!(
            inspect_worktree_removal(&project, &disguised_primary, &disguised_options),
            Err(RemoveWorktreeError::PrimaryWorktree)
        ));

        let mut stale = worktree.clone();
        stale.path.push_str("-changed");
        assert!(matches!(
            inspect_worktree_removal(&project, &stale, &options),
            Err(RemoveWorktreeError::StaleWorktree)
        ));
        let mut remote = project.clone();
        remote.remote_workspace_id = Some("REMOTE".into());
        assert!(matches!(
            inspect_worktree_removal(&remote, &worktree, &options),
            Err(RemoveWorktreeError::RemoteProject)
        ));

        let mut external = worktree.clone();
        external.source = Source::External;
        let mut external_options = options.clone();
        external_options
            .current_worktrees
            .iter_mut()
            .find(|candidate| candidate.id == external.id)
            .unwrap()
            .source = Source::External;
        let external_inspection =
            inspect_worktree_removal(&project, &external, &external_options).unwrap();
        assert!(external_inspection.teardown_commands.is_empty());

        let mut diagnostic_options = options;
        diagnostic_options.git_options.executable = temp.path().join("missing-git");
        let diagnostic =
            inspect_worktree_removal(&project, &worktree, &diagnostic_options).unwrap();
        assert!(!diagnostic.dirty);
        assert!(diagnostic.inspection_diagnostic.is_some());
    }

    #[test]
    fn remove_worktree_is_disk_first_and_preserves_pre_disk_failures() {
        let temp = verification_temp();
        let project = repository(temp.path());
        let created = created_secondary(temp.path(), &project, "blocked", "blocked");
        let worktree = created.worktree.clone();
        std::fs::create_dir_all(Path::new(&project.path).join(".muxy")).unwrap();
        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            r#"{"teardown":["false"]}"#,
        )
        .unwrap();
        let options = removal_options(temp.path(), created.worktrees.clone());
        let inspection = inspect_worktree_removal(&project, &worktree, &options).unwrap();
        let approval = ProjectHookApproval::from_resolved(&inspection.teardown_commands);
        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            r#"{"teardown":["true"]}"#,
        )
        .unwrap();
        let result = remove_worktree(
            RemoveWorktreeRequest {
                project: project.clone(),
                worktree: worktree.clone(),
                project_hook_approval: Some(approval),
            },
            &options,
        );
        assert!(matches!(result, Err(RemoveWorktreeError::Hook(_))));
        assert!(Path::new(&worktree.path).exists());
        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            r#"{"teardown":["false"]}"#,
        )
        .unwrap();
        let inspection = inspect_worktree_removal(&project, &worktree, &options).unwrap();
        let approval = ProjectHookApproval::from_resolved(&inspection.teardown_commands);
        let result = remove_worktree(
            RemoveWorktreeRequest {
                project: project.clone(),
                worktree: worktree.clone(),
                project_hook_approval: Some(approval),
            },
            &options,
        );
        assert!(matches!(result, Err(RemoveWorktreeError::Hook(_))));
        assert!(Path::new(&worktree.path).exists());
        assert!(
            muxy_core::store::worktrees::load_from(&options.worktrees_dir, &project.id)
                .iter()
                .any(|candidate| candidate.id == worktree.id)
        );

        std::fs::write(
            Path::new(&project.path).join(".muxy/worktree.json"),
            r#"{"teardown":["sleep 1"]}"#,
        )
        .unwrap();
        let inspection = inspect_worktree_removal(&project, &worktree, &options).unwrap();
        let approval = ProjectHookApproval::from_resolved(&inspection.teardown_commands);
        let mut timeout_options = options.clone();
        timeout_options.timeout = Duration::ZERO;
        let result = remove_worktree(
            RemoveWorktreeRequest {
                project: project.clone(),
                worktree: worktree.clone(),
                project_hook_approval: Some(approval),
            },
            &timeout_options,
        );
        assert!(matches!(result, Err(RemoveWorktreeError::Hook(_))));
        assert!(Path::new(&worktree.path).exists());

        let mut git_failure_options = options;
        git_failure_options.git_options.executable = temp.path().join("missing-git");
        let mut external = worktree.clone();
        external.source = Source::External;
        git_failure_options
            .current_worktrees
            .iter_mut()
            .find(|candidate| candidate.id == external.id)
            .unwrap()
            .source = Source::External;
        let result = remove_worktree(
            RemoveWorktreeRequest {
                project,
                worktree: external,
                project_hook_approval: None,
            },
            &git_failure_options,
        );
        assert!(matches!(result, Err(RemoveWorktreeError::Git(_))));
        assert!(Path::new(&worktree.path).exists());
    }

    #[test]
    fn remove_worktree_cleans_disk_parent_and_warns_after_irreversible_failures() {
        let temp = verification_temp();
        let project = repository(temp.path());
        let created = created_secondary(temp.path(), &project, "empty-parent", "removed");
        let worktree = created.worktree.clone();
        let parent = Path::new(&worktree.path).parent().unwrap().to_path_buf();
        let blocked = temp.path().join("blocked-tracking");
        std::fs::write(&blocked, b"blocked").unwrap();
        let mut options = removal_options(temp.path(), created.worktrees.clone());
        options.worktrees_dir = blocked.join("tracking");
        let outcome = remove_worktree(
            RemoveWorktreeRequest {
                project: project.clone(),
                worktree: worktree.clone(),
                project_hook_approval: Some(ProjectHookApproval::from_resolved(&[])),
            },
            &options,
        )
        .unwrap();
        assert!(!Path::new(&worktree.path).exists());
        assert!(!parent.exists());
        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::WorktreeListPersistence(_)))
        );
        assert!(!outcome.files_preserved);
        assert!(
            outcome
                .worktrees
                .iter()
                .all(|candidate| candidate.id != worktree.id)
        );

        let reconciled = created_secondary(temp.path(), &project, "reconciled", "reconciled");
        let reconciled_worktree = reconciled.worktree.clone();
        run(
            Path::new(&project.path),
            &[
                "worktree",
                "remove",
                "--force",
                "--",
                &reconciled_worktree.path,
            ],
        );
        let reconciled_outcome = remove_worktree(
            RemoveWorktreeRequest {
                project: project.clone(),
                worktree: reconciled_worktree,
                project_hook_approval: Some(ProjectHookApproval::from_resolved(&[])),
            },
            &removal_options(temp.path(), reconciled.worktrees),
        )
        .unwrap();
        assert!(
            reconciled_outcome
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::ReconciledGitRemoval(_)))
        );

        let preserved = created_secondary(temp.path(), &project, "preserved", "preserved");
        let preserved_worktree = preserved.worktree.clone();
        let mut missing_project = project;
        missing_project.path = temp
            .path()
            .join("missing-primary")
            .to_string_lossy()
            .into_owned();
        let preserved_outcome = remove_worktree(
            RemoveWorktreeRequest {
                project: missing_project,
                worktree: preserved_worktree.clone(),
                project_hook_approval: None,
            },
            &removal_options(temp.path(), preserved.worktrees),
        )
        .unwrap();
        assert!(preserved_outcome.files_preserved);
        assert!(Path::new(&preserved_worktree.path).exists());
        assert!(
            preserved_outcome
                .warnings
                .iter()
                .any(|warning| matches!(warning, LifecycleWarning::PreservedFiles(_)))
        );
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
