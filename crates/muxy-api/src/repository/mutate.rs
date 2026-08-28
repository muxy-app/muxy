use super::{ChangedFile, RepositoryError, RepositoryHead, RepositoryService};
use crate::git::command::{RepositoryCommandRequest, repository_command, run_output};
use crate::git::validation::validate_repository_path;
use crate::git::{GitError, RepositoryPathError, SafeDeleteError, SafeUntrackedDelete};
use crate::subprocess::{
    CancellationSignal, CapturedOutput, Deadline, StdinMode, SubprocessError, SubprocessOutput,
    bounded_error_text,
};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

const MUTATION_STDOUT_LIMIT: usize = 1_024 * 1_024;
const MUTATION_STDERR_LIMIT: usize = 1_024 * 1_024;
const READ_STDOUT_LIMIT: usize = 16 * 1_024 * 1_024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MUTATION_PENDING: u8 = 0;
const MUTATION_DISPATCHED: u8 = 1;
const MUTATION_STOP_AFTER_CURRENT: u8 = 2;
const MUTATION_CANCELLED: u8 = 3;

#[derive(Clone, Debug, Default)]
pub struct MutationBoundary {
    state: Arc<AtomicU8>,
}

impl MutationBoundary {
    pub fn begin_irreversible(&self) -> bool {
        self.state
            .compare_exchange(
                MUTATION_PENDING,
                MUTATION_DISPATCHED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub fn cancel_for_identity_change(&self) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                MUTATION_PENDING => {
                    if self
                        .state
                        .compare_exchange(
                            MUTATION_PENDING,
                            MUTATION_CANCELLED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return true;
                    }
                }
                MUTATION_DISPATCHED => {
                    if self
                        .state
                        .compare_exchange(
                            MUTATION_DISPATCHED,
                            MUTATION_STOP_AFTER_CURRENT,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return false;
                    }
                }
                MUTATION_STOP_AFTER_CURRENT => return false,
                MUTATION_CANCELLED => return true,
                _ => unreachable!(),
            }
        }
    }

    pub fn finish_irreversible(&self) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                MUTATION_DISPATCHED => {
                    if self
                        .state
                        .compare_exchange(
                            MUTATION_DISPATCHED,
                            MUTATION_PENDING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return false;
                    }
                }
                MUTATION_STOP_AFTER_CURRENT => {
                    if self
                        .state
                        .compare_exchange(
                            MUTATION_STOP_AFTER_CURRENT,
                            MUTATION_CANCELLED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return true;
                    }
                }
                MUTATION_PENDING | MUTATION_CANCELLED => return false,
                _ => unreachable!(),
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) == MUTATION_CANCELLED
    }

    pub fn stop_after_current(&self) -> bool {
        self.state.load(Ordering::Acquire) == MUTATION_STOP_AFTER_CURRENT
    }
}

#[derive(Clone, Debug)]
pub struct MutationControl {
    timeout: Duration,
    cancellation: Option<CancellationSignal>,
    boundary: MutationBoundary,
}

impl Default for MutationControl {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            cancellation: None,
            boundary: MutationBoundary::default(),
        }
    }
}

impl MutationControl {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            cancellation: None,
            boundary: MutationBoundary::default(),
        }
    }

    pub fn with_cancellation(cancellation: CancellationSignal) -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            cancellation: Some(cancellation),
            boundary: MutationBoundary::default(),
        }
    }

    pub fn with_cancellation_and_boundary(
        cancellation: CancellationSignal,
        boundary: MutationBoundary,
    ) -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            cancellation: Some(cancellation),
            boundary,
        }
    }

    pub(crate) fn from_parts_with_boundary(
        timeout: Duration,
        cancellation: Option<CancellationSignal>,
        boundary: MutationBoundary,
    ) -> Self {
        Self {
            timeout,
            cancellation,
            boundary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationOutcome {
    NoMutation,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationEffect {
    NoMutation,
    Uncertain,
    PartialSuccess { completed: &'static str },
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryMutationError {
    #[error(transparent)]
    Path(#[from] RepositoryPathError),
    #[error(transparent)]
    Delete(#[from] SafeDeleteError),
    #[error("changed file no longer matches repository state")]
    StaleChangedFile,
    #[error("conflicted files cannot be discarded")]
    ConflictDiscard,
    #[error("branch name is invalid")]
    InvalidBranch,
    #[error("repository is detached")]
    DetachedHead,
    #[error("branch does not exist")]
    MissingBranch,
    #[error("branch already exists")]
    BranchExists,
    #[error("current branch cannot be deleted")]
    CurrentBranch,
    #[error("branch deletion intent no longer matches repository state")]
    StaleBranchDeletion,
    #[error("commit message is empty")]
    EmptyMessage,
    #[error("tracked changes must be clean")]
    DirtyRepository,
    #[error("remote context changed")]
    RemoteContextChanged,
    #[error("repository {operation} failed after {effect:?}: {source}")]
    Command {
        operation: &'static str,
        effect: MutationEffect,
        #[source]
        source: Box<RepositoryError>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDeletionIntent {
    branch: Vec<u8>,
    expected_oid: Vec<u8>,
    current_branch: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PushTargetSnapshot {
    branch: Vec<u8>,
    upstream: Option<Vec<u8>>,
    remote: Vec<u8>,
    push_ref: Vec<u8>,
    fetch_url: Vec<u8>,
    push_url: Vec<u8>,
    configuration: Vec<u8>,
}

struct CommandRun {
    operation: &'static str,
    args: Vec<OsString>,
    read_only: bool,
    network: bool,
    stdout_limit: usize,
    effect: MutationEffect,
}

impl RepositoryService {
    pub fn stage(
        &self,
        repository: &Path,
        expected: &ChangedFile,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let current = self.current_changed_file(repository, expected)?;
        if current.is_staged && !current.is_unstaged && !current.is_conflicted {
            return Ok(MutationOutcome::NoMutation);
        }
        let mut args = os_args(&["add", "--"]);
        args.extend(validated_related_paths(&current)?);
        self.mutate(
            repository,
            "stage",
            args,
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn stage_all(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        if !self
            .summary(repository)
            .map_err(read_error("stage all"))?
            .is_dirty()
        {
            return Ok(MutationOutcome::NoMutation);
        }
        self.mutate(
            repository,
            "stage all",
            os_args(&["add", "-A"]),
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn unstage(
        &self,
        repository: &Path,
        expected: &ChangedFile,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let current = self.current_changed_file(repository, expected)?;
        if !current.is_staged && !current.is_conflicted {
            return Ok(MutationOutcome::NoMutation);
        }
        let mut args = match self
            .summary(repository)
            .map_err(read_error("unstage"))?
            .head
        {
            RepositoryHead::Unborn => os_args(&["rm", "--cached", "-f", "--"]),
            RepositoryHead::Commit(_) => os_args(&["reset", "HEAD", "--"]),
        };
        args.extend(validated_related_paths(&current)?);
        self.mutate(
            repository,
            "unstage",
            args,
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn unstage_all(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let summary = self
            .summary(repository)
            .map_err(read_error("unstage all"))?;
        if summary.staged_count == 0 && summary.conflicted_count == 0 {
            return Ok(MutationOutcome::NoMutation);
        }
        let args = match summary.head {
            RepositoryHead::Unborn => os_args(&["rm", "--cached", "-r", "-f", "--", "."]),
            RepositoryHead::Commit(_) => os_args(&["reset", "HEAD"]),
        };
        self.mutate(
            repository,
            "unstage all",
            args,
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn discard(
        &self,
        repository: &Path,
        expected: &ChangedFile,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let current = self.current_changed_file(repository, expected)?;
        if current.is_conflicted {
            return Err(RepositoryMutationError::ConflictDiscard);
        }
        if current.is_untracked {
            if let Some(old_path) = self.unstaged_rename_source(repository, &current, control)? {
                return self.restore_rename(repository, &old_path, &current.path, control);
            }
            SafeUntrackedDelete::delete(repository, &current.path)?;
            return Ok(MutationOutcome::Success);
        }
        if current.y_status == b'C' {
            SafeUntrackedDelete::delete(repository, &current.path)?;
            return Ok(MutationOutcome::Success);
        }
        if current.y_status == b'R'
            && let Some(old_path) = current.old_path.as_deref()
        {
            return self.restore_rename(repository, old_path, &current.path, control);
        }
        if !current.is_unstaged {
            return Ok(MutationOutcome::NoMutation);
        }
        let path = validate_repository_path(&current.path)?;
        self.mutate(
            repository,
            "discard tracked change",
            vec![
                OsString::from("checkout"),
                OsString::from("--"),
                path.as_os_str().to_owned(),
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn switch_branch(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        self.check_branch(repository, branch, control)?;
        if !self
            .local_branches(repository)
            .map_err(read_error("switch branch"))?
            .iter()
            .any(|candidate| candidate == branch)
        {
            return Err(RepositoryMutationError::MissingBranch);
        }
        if self.current_branch(repository, control)?.as_deref() == Some(branch) {
            return Ok(MutationOutcome::NoMutation);
        }
        self.mutate(
            repository,
            "switch branch",
            vec![OsString::from("switch"), os_string(branch)?],
            false,
            control,
            MutationEffect::NoMutation,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn switch_remote_branch(
        &self,
        repository: &Path,
        remote_branch: &[u8],
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        self.check_branch(repository, remote_branch, control)?;
        let Some(separator) = remote_branch.iter().position(|byte| *byte == b'/') else {
            return Err(RepositoryMutationError::InvalidBranch);
        };
        let local_branch = &remote_branch[separator + 1..];
        if local_branch.is_empty() {
            return Err(RepositoryMutationError::InvalidBranch);
        }
        if self
            .local_branches(repository)
            .map_err(read_error("local branches"))?
            .iter()
            .any(|branch| branch == local_branch)
        {
            return self.switch_branch(repository, local_branch, control);
        }
        let mut reference = b"refs/remotes/".to_vec();
        reference.extend_from_slice(remote_branch);
        let expected = self.read(
            repository,
            "remote branch identity",
            vec![OsString::from("rev-parse"), os_string(&reference)?],
            control,
        )?;
        let expected = trim_output(&expected.stdout)
            .map(<[u8]>::to_vec)
            .ok_or(RepositoryMutationError::MissingBranch)?;
        let current = self.read(
            repository,
            "remote branch identity",
            vec![OsString::from("rev-parse"), os_string(&reference)?],
            control,
        )?;
        if trim_output(&current.stdout) != Some(expected.as_slice()) {
            return Err(RepositoryMutationError::MissingBranch);
        }
        self.mutate(
            repository,
            "switch remote branch",
            vec![
                OsString::from("switch"),
                OsString::from("--track"),
                os_string(remote_branch)?,
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn create_branch(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        self.check_branch(repository, branch, control)?;
        if self
            .local_branches(repository)
            .map_err(read_error("create branch"))?
            .iter()
            .any(|candidate| candidate == branch)
        {
            return Err(RepositoryMutationError::BranchExists);
        }
        self.mutate(
            repository,
            "create branch",
            vec![
                OsString::from("switch"),
                OsString::from("-c"),
                os_string(branch)?,
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn prepare_branch_deletion(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &MutationControl,
    ) -> Result<BranchDeletionIntent, RepositoryMutationError> {
        self.check_branch(repository, branch, control)?;
        let current_branch = self
            .current_branch(repository, control)?
            .ok_or(RepositoryMutationError::DetachedHead)?;
        if current_branch == branch {
            return Err(RepositoryMutationError::CurrentBranch);
        }
        if !self
            .local_branches(repository)
            .map_err(read_error("prepare branch deletion"))?
            .iter()
            .any(|candidate| candidate == branch)
        {
            return Err(RepositoryMutationError::MissingBranch);
        }
        Ok(BranchDeletionIntent {
            branch: branch.to_vec(),
            expected_oid: self.branch_oid(repository, branch, control)?,
            current_branch,
        })
    }

    pub fn delete_branch(
        &self,
        repository: &Path,
        intent: &BranchDeletionIntent,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        if self.current_branch(repository, control)?.as_deref()
            != Some(intent.current_branch.as_slice())
            || self.branch_oid(repository, &intent.branch, control)? != intent.expected_oid
        {
            return Err(RepositoryMutationError::StaleBranchDeletion);
        }
        self.mutate(
            repository,
            "delete branch",
            vec![
                OsString::from("branch"),
                OsString::from("-D"),
                OsString::from("--"),
                os_string(&intent.branch)?,
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn commit(
        &self,
        repository: &Path,
        message: &str,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(RepositoryMutationError::EmptyMessage);
        }
        self.mutate(
            repository,
            "commit",
            vec![
                OsString::from("commit"),
                OsString::from("-m"),
                OsString::from(message),
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn push(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        self.push_with_boundary(repository, control, || {})
    }

    fn push_with_boundary(
        &self,
        repository: &Path,
        control: &MutationControl,
        boundary: impl FnOnce(),
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let snapshot = self.push_target_snapshot(repository, control)?;
        boundary();
        self.ensure_push_target(repository, &snapshot, control)?;
        self.push_snapshot(repository, &snapshot, control, MutationEffect::Uncertain)?;
        Ok(MutationOutcome::Success)
    }

    pub fn pull(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let snapshot = self.push_target_snapshot(repository, control)?;
        self.ensure_push_target(repository, &snapshot, control)?;
        self.mutate(
            repository,
            "pull",
            os_args(&["pull"]),
            true,
            control,
            MutationEffect::Uncertain,
        )?;
        Ok(MutationOutcome::Success)
    }

    pub fn update_from_base(
        &self,
        repository: &Path,
        base: &[u8],
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        self.check_branch(repository, base, control)?;
        let status = self.read(
            repository,
            "tracked status",
            os_args(&["status", "--porcelain=1", "--untracked-files=no"]),
            control,
        )?;
        if !status.stdout.is_empty() {
            return Err(RepositoryMutationError::DirtyRepository);
        }
        let snapshot = self.push_target_snapshot(repository, control)?;
        self.ensure_push_target(repository, &snapshot, control)?;
        self.mutate(
            repository,
            "fetch base",
            vec![
                OsString::from("fetch"),
                OsString::from("origin"),
                os_string(base)?,
            ],
            true,
            control,
            MutationEffect::Uncertain,
        )?;
        let mut base_reference = b"origin/".to_vec();
        base_reference.extend_from_slice(base);
        if let Err(error) = self.mutate(
            repository,
            "merge base",
            vec![
                OsString::from("merge"),
                OsString::from("--no-edit"),
                os_string(&base_reference)?,
            ],
            false,
            control,
            MutationEffect::Uncertain,
        ) {
            return match self.mutate(
                repository,
                "abort merge",
                os_args(&["merge", "--abort"]),
                false,
                control,
                MutationEffect::Uncertain,
            ) {
                Ok(_) => Err(error.with_effect(MutationEffect::NoMutation)),
                Err(abort) => Err(abort),
            };
        }
        self.ensure_push_target(repository, &snapshot, control)?;
        self.push_snapshot(
            repository,
            &snapshot,
            control,
            MutationEffect::PartialSuccess {
                completed: "base merged",
            },
        )?;
        Ok(MutationOutcome::Success)
    }

    fn current_changed_file(
        &self,
        repository: &Path,
        expected: &ChangedFile,
    ) -> Result<ChangedFile, RepositoryMutationError> {
        for path in expected.related_paths() {
            validate_repository_path(path)?;
        }
        self.changed_files(repository)
            .map_err(read_error("changed file validation"))?
            .files
            .into_iter()
            .find(|current| current.stable_id() == expected.stable_id() && current == expected)
            .ok_or(RepositoryMutationError::StaleChangedFile)
    }

    fn unstaged_rename_source(
        &self,
        repository: &Path,
        untracked: &ChangedFile,
        control: &MutationControl,
    ) -> Result<Option<Vec<u8>>, RepositoryMutationError> {
        let untracked_path = validate_repository_path(&untracked.path)?;
        let working_oid = self.read(
            repository,
            "untracked identity",
            vec![
                OsString::from("hash-object"),
                OsString::from("--"),
                untracked_path.as_os_str().to_owned(),
            ],
            control,
        )?;
        let Some(working_oid) = trim_output(&working_oid.stdout) else {
            return Ok(None);
        };
        let changes = self
            .changed_files(repository)
            .map_err(read_error("rename source validation"))?;
        let mut matched = None;
        for candidate in changes.files.iter().filter(|candidate| {
            candidate.y_status == b'D' && candidate.is_unstaged && !candidate.is_conflicted
        }) {
            let candidate_path = validate_repository_path(&candidate.path)?;
            let tree = self.read(
                repository,
                "tracked identity",
                vec![
                    OsString::from("ls-tree"),
                    OsString::from("-z"),
                    OsString::from("HEAD"),
                    OsString::from("--"),
                    candidate_path.as_os_str().to_owned(),
                ],
                control,
            )?;
            let Some(tree_oid) = parse_ls_tree_oid(&tree.stdout) else {
                continue;
            };
            if tree_oid == working_oid && matched.replace(candidate.path.clone()).is_some() {
                return Ok(None);
            }
        }
        Ok(matched)
    }

    fn restore_rename(
        &self,
        repository: &Path,
        old_path: &[u8],
        new_path: &[u8],
        control: &MutationControl,
    ) -> Result<MutationOutcome, RepositoryMutationError> {
        let old_path = validate_repository_path(old_path)?;
        self.mutate(
            repository,
            "restore renamed path",
            vec![
                OsString::from("checkout"),
                OsString::from("--"),
                old_path.as_os_str().to_owned(),
            ],
            false,
            control,
            MutationEffect::Uncertain,
        )?;
        SafeUntrackedDelete::delete(repository, new_path)
            .map(|()| MutationOutcome::Success)
            .map_err(|source| RepositoryMutationError::Command {
                operation: "discard rename",
                effect: MutationEffect::PartialSuccess {
                    completed: "old path restored",
                },
                source: Box::new(RepositoryError::Io {
                    operation: "discard rename",
                    source: std::io::Error::other(source.to_string()),
                }),
            })
    }

    fn check_branch(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &MutationControl,
    ) -> Result<(), RepositoryMutationError> {
        if branch.is_empty() || branch.starts_with(b"-") || branch.contains(&0) {
            return Err(RepositoryMutationError::InvalidBranch);
        }
        let output = self.read_status(
            repository,
            "validate branch",
            vec![
                OsString::from("check-ref-format"),
                OsString::from("--branch"),
                os_string(branch)?,
            ],
            control,
        )?;
        if output.status.success() {
            Ok(())
        } else {
            Err(RepositoryMutationError::InvalidBranch)
        }
    }

    fn current_branch(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<Option<Vec<u8>>, RepositoryMutationError> {
        let output = self.read_status(
            repository,
            "current branch",
            os_args(&["symbolic-ref", "--quiet", "--short", "HEAD"]),
            control,
        )?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(trim_output(&output.stdout).map(<[u8]>::to_vec))
    }

    fn branch_oid(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &MutationControl,
    ) -> Result<Vec<u8>, RepositoryMutationError> {
        let mut reference = b"refs/heads/".to_vec();
        reference.extend_from_slice(branch);
        let output = self.read(
            repository,
            "branch identity",
            vec![OsString::from("rev-parse"), os_string(&reference)?],
            control,
        )?;
        trim_output(&output.stdout)
            .map(<[u8]>::to_vec)
            .ok_or(RepositoryMutationError::MissingBranch)
    }

    fn push_target_snapshot(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<PushTargetSnapshot, RepositoryMutationError> {
        let branch = self
            .current_branch(repository, control)?
            .ok_or(RepositoryMutationError::DetachedHead)?;
        let upstream = self.read_optional(
            repository,
            "upstream",
            os_args(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ]),
            control,
        )?;
        let configuration = self.remote_configuration(repository, control)?;
        let branch_push_remote = self.config_value(repository, &branch, "pushRemote", control)?;
        let default_push_remote = self.config_key(repository, b"remote.pushDefault", control)?;
        let branch_remote = self.config_value(repository, &branch, "remote", control)?;
        let upstream_remote = upstream
            .as_deref()
            .and_then(|value| value.split(|byte| *byte == b'/').next())
            .filter(|value| !value.is_empty())
            .map(<[u8]>::to_vec);
        let remote = branch_push_remote
            .or(default_push_remote)
            .or(branch_remote)
            .or(upstream_remote)
            .unwrap_or_else(|| b"origin".to_vec());
        let push_ref = self
            .config_value(repository, &branch, "merge", control)?
            .unwrap_or_else(|| {
                let mut reference = b"refs/heads/".to_vec();
                reference.extend_from_slice(&branch);
                reference
            });
        let fetch_url = self.remote_url(repository, &remote, false, control)?;
        let push_url = self.remote_url(repository, &remote, true, control)?;
        Ok(PushTargetSnapshot {
            branch,
            upstream,
            remote,
            push_ref,
            fetch_url,
            push_url,
            configuration,
        })
    }

    fn remote_configuration(
        &self,
        repository: &Path,
        control: &MutationControl,
    ) -> Result<Vec<u8>, RepositoryMutationError> {
        Ok(self
            .read_optional(
                repository,
                "remote configuration",
                os_args(&[
                    "config",
                    "--null",
                    "--get-regexp",
                    "^(remote\\..*\\.(url|pushurl)|remote\\.pushdefault|branch\\..*\\.(remote|pushremote|merge))$",
                ]),
                control,
            )?
            .unwrap_or_default())
    }

    fn ensure_push_target(
        &self,
        repository: &Path,
        expected: &PushTargetSnapshot,
        control: &MutationControl,
    ) -> Result<(), RepositoryMutationError> {
        if self.remote_configuration(repository, control)? != expected.configuration {
            return Err(RepositoryMutationError::RemoteContextChanged);
        }
        if self.push_target_snapshot(repository, control)? != *expected {
            return Err(RepositoryMutationError::RemoteContextChanged);
        }
        Ok(())
    }

    fn config_value(
        &self,
        repository: &Path,
        branch: &[u8],
        suffix: &str,
        control: &MutationControl,
    ) -> Result<Option<Vec<u8>>, RepositoryMutationError> {
        let mut key = b"branch.".to_vec();
        key.extend_from_slice(branch);
        key.push(b'.');
        key.extend_from_slice(suffix.as_bytes());
        self.config_key(repository, &key, control)
    }

    fn config_key(
        &self,
        repository: &Path,
        key: &[u8],
        control: &MutationControl,
    ) -> Result<Option<Vec<u8>>, RepositoryMutationError> {
        self.read_optional(
            repository,
            "remote configuration value",
            vec![
                OsString::from("config"),
                OsString::from("--get"),
                os_string(key)?,
            ],
            control,
        )
    }

    fn remote_url(
        &self,
        repository: &Path,
        remote: &[u8],
        push: bool,
        control: &MutationControl,
    ) -> Result<Vec<u8>, RepositoryMutationError> {
        let mut args = os_args(&["remote", "get-url"]);
        if push {
            args.push(OsString::from("--push"));
        }
        args.push(os_string(remote)?);
        let output = self.read(repository, "remote URL", args, control)?;
        trim_output(&output.stdout)
            .map(<[u8]>::to_vec)
            .ok_or(RepositoryMutationError::RemoteContextChanged)
    }

    fn push_snapshot(
        &self,
        repository: &Path,
        snapshot: &PushTargetSnapshot,
        control: &MutationControl,
        effect: MutationEffect,
    ) -> Result<(), RepositoryMutationError> {
        let args = if snapshot.upstream.is_some() {
            os_args(&["push"])
        } else {
            vec![
                OsString::from("push"),
                OsString::from("--set-upstream"),
                OsString::from("origin"),
                os_string(&snapshot.branch)?,
            ]
        };
        self.mutate(repository, "push", args, true, control, effect)
            .map(|_| ())
    }

    fn read_optional(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &MutationControl,
    ) -> Result<Option<Vec<u8>>, RepositoryMutationError> {
        let output = self.read_status(repository, operation, args, control)?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(trim_output(&output.stdout).map(<[u8]>::to_vec))
    }

    fn read(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &MutationControl,
    ) -> Result<SubprocessOutput, RepositoryMutationError> {
        let output = self.read_status(repository, operation, args, control)?;
        if !output.status.success() {
            return Err(command_error(
                operation,
                MutationEffect::NoMutation,
                status_error(operation, &output),
            ));
        }
        Ok(output)
    }

    fn read_status(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &MutationControl,
    ) -> Result<SubprocessOutput, RepositoryMutationError> {
        self.run(
            repository,
            CommandRun {
                operation,
                args,
                read_only: true,
                network: false,
                stdout_limit: READ_STDOUT_LIMIT,
                effect: MutationEffect::NoMutation,
            },
            control,
        )
    }

    pub(super) fn mutate(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        network: bool,
        control: &MutationControl,
        effect: MutationEffect,
    ) -> Result<SubprocessOutput, RepositoryMutationError> {
        self.run(
            repository,
            CommandRun {
                operation,
                args,
                read_only: false,
                network,
                stdout_limit: MUTATION_STDOUT_LIMIT,
                effect,
            },
            control,
        )
    }

    fn run(
        &self,
        repository: &Path,
        command: CommandRun,
        control: &MutationControl,
    ) -> Result<SubprocessOutput, RepositoryMutationError> {
        if !command.read_only && !control.boundary.begin_irreversible() {
            return Err(command_error(
                command.operation,
                command.effect,
                RepositoryError::Process {
                    operation: command.operation,
                    source: GitError::Process(SubprocessError::Cancelled {
                        output: CapturedOutput::default(),
                    }),
                },
            ));
        }
        let request = RepositoryCommandRequest {
            args: command.args,
            read_only: command.read_only,
            network: command.network,
            stdin: StdinMode::Closed,
            stdout_limit: command.stdout_limit,
            stderr_limit: MUTATION_STDERR_LIMIT,
            cancellation: control.cancellation.clone(),
        };
        let deadline = Deadline::new(control.timeout);
        let output = run_output(
            &self.options.git,
            repository,
            repository_command(&self.options.environment, request),
            &deadline,
        );
        if !command.read_only
            && control.boundary.finish_irreversible()
            && let Some(cancellation) = &control.cancellation
        {
            cancellation.cancel();
        }
        let output = output.map_err(|source| {
            command_error(
                command.operation,
                command.effect,
                RepositoryError::Process {
                    operation: command.operation,
                    source,
                },
            )
        })?;
        if output.stdout_truncated || output.stderr_truncated {
            return Err(command_error(
                command.operation,
                command.effect,
                RepositoryError::Truncated {
                    operation: command.operation,
                },
            ));
        }
        if !command.read_only && !output.status.success() {
            return Err(command_error(
                command.operation,
                command.effect,
                status_error(command.operation, &output),
            ));
        }
        Ok(output)
    }
}

impl RepositoryMutationError {
    pub fn effect(&self) -> MutationEffect {
        match self {
            Self::Command { effect, .. } => *effect,
            _ => MutationEffect::NoMutation,
        }
    }

    fn with_effect(self, effect: MutationEffect) -> Self {
        match self {
            Self::Command {
                operation, source, ..
            } => Self::Command {
                operation,
                effect,
                source,
            },
            other => other,
        }
    }
}

fn read_error(operation: &'static str) -> impl FnOnce(RepositoryError) -> RepositoryMutationError {
    move |source| command_error(operation, MutationEffect::NoMutation, source)
}

fn command_error(
    operation: &'static str,
    effect: MutationEffect,
    source: RepositoryError,
) -> RepositoryMutationError {
    RepositoryMutationError::Command {
        operation,
        effect,
        source: Box::new(source),
    }
}

fn status_error(operation: &'static str, output: &SubprocessOutput) -> RepositoryError {
    RepositoryError::Status {
        operation,
        status: output.status.code(),
        message: bounded_error_text(&output.stderr),
    }
}

fn validated_related_paths(file: &ChangedFile) -> Result<Vec<OsString>, RepositoryMutationError> {
    file.related_paths()
        .into_iter()
        .map(validate_repository_path)
        .map(|path| {
            path.map(|path| path.as_os_str().to_owned())
                .map_err(Into::into)
        })
        .collect()
}

fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn trim_output(bytes: &[u8]) -> Option<&[u8]> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    (!bytes.is_empty()).then_some(bytes)
}

fn parse_ls_tree_oid(output: &[u8]) -> Option<&[u8]> {
    let record = output.strip_suffix(b"\0")?;
    let metadata = record.split(|byte| *byte == b'\t').next()?;
    let mut fields = metadata.split(|byte| *byte == b' ');
    fields.next()?;
    if fields.next()? != b"blob" {
        return None;
    }
    let oid = fields.next()?;
    (matches!(oid.len(), 40 | 64) && oid.iter().all(u8::is_ascii_hexdigit)).then_some(oid)
}

#[cfg(unix)]
fn os_string(bytes: &[u8]) -> Result<OsString, RepositoryMutationError> {
    use std::os::unix::ffi::OsStringExt;

    if bytes.is_empty() || bytes.contains(&0) {
        return Err(RepositoryMutationError::InvalidBranch);
    }
    Ok(OsString::from_vec(bytes.to_vec()))
}

#[cfg(not(unix))]
fn os_string(bytes: &[u8]) -> Result<OsString, RepositoryMutationError> {
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(RepositoryMutationError::InvalidBranch);
    }
    String::from_utf8(bytes.to_vec())
        .map(OsString::from)
        .map_err(|_| RepositoryMutationError::InvalidBranch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_environment::ExecutionEnvironment;
    use crate::git::GitOptions;
    use crate::repository::{ChangedFile, RepositoryService};
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    fn environment_for(home: &Path) -> ExecutionEnvironment {
        ExecutionEnvironment::fallback([
            (
                OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            ),
            (OsString::from("HOME"), home.as_os_str().to_owned()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                home.join("config").into_os_string(),
            ),
        ])
    }

    fn service_for(home: &Path) -> RepositoryService {
        let environment = environment_for(home);
        let executable = environment.resolve_executable(OsStr::new("git")).unwrap();
        RepositoryService::new(crate::repository::RepositoryOptions {
            git: GitOptions {
                executable,
                environment: HashMap::new(),
            },
            environment,
        })
    }

    fn command(repo: Option<&Path>, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        if let Some(repo) = repo {
            command.arg("-C").arg(repo);
        }
        command
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        command
    }

    fn output(repo: Option<&Path>, args: &[&str]) -> Output {
        command(repo, args).output().unwrap()
    }

    fn git(repo: Option<&Path>, args: &[&str]) {
        let output = output(repo, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init(repo: &Path) {
        git(None, &["init", "-q", "-b", "main", repo.to_str().unwrap()]);
        git(Some(repo), &["config", "user.name", "Muxy Tests"]);
        git(
            Some(repo),
            &["config", "user.email", "muxy@example.invalid"],
        );
    }

    fn write(repo: &Path, path: &str, contents: impl AsRef<[u8]>) {
        let path = repo.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn commit_fixture(repo: &Path, message: &str) {
        git(Some(repo), &["add", "-A"]);
        git(
            Some(repo),
            &[
                "-c",
                "user.name=Muxy Tests",
                "-c",
                "user.email=muxy@example.invalid",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    fn control() -> MutationControl {
        MutationControl::default()
    }

    fn changed(service: &RepositoryService, repo: &Path, path: &[u8]) -> ChangedFile {
        service
            .changed_files(repo)
            .unwrap()
            .files
            .into_iter()
            .find(|file| file.path == path)
            .unwrap()
    }

    fn rev(repo: &Path, reference: &str) -> String {
        String::from_utf8(output(Some(repo), &["rev-parse", reference]).stdout)
            .unwrap()
            .trim()
            .to_owned()
    }

    #[test]
    fn repository_mutate_stages_unstages_all_and_handles_unborn_head() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo);
        write(&repo, "tracked.txt", "base\n");
        commit_fixture(&repo, "base");
        write(&repo, "tracked.txt", "changed\n");
        write(&repo, "new.txt", "new\n");
        let service = service_for(&temp.path().join("home"));

        let new_file = changed(&service, &repo, b"new.txt");
        assert_eq!(
            service.stage(&repo, &new_file, &control()).unwrap(),
            MutationOutcome::Success
        );
        assert!(
            String::from_utf8(output(Some(&repo), &["diff", "--cached", "--name-only"]).stdout)
                .unwrap()
                .contains("new.txt")
        );

        let staged = changed(&service, &repo, b"new.txt");
        service.unstage(&repo, &staged, &control()).unwrap();
        assert!(
            output(Some(&repo), &["diff", "--cached", "--name-only"])
                .stdout
                .is_empty()
        );
        service.stage_all(&repo, &control()).unwrap();
        assert_eq!(
            String::from_utf8(output(Some(&repo), &["diff", "--cached", "--name-only"]).stdout)
                .unwrap()
                .lines()
                .count(),
            2
        );
        service.unstage_all(&repo, &control()).unwrap();
        assert!(
            output(Some(&repo), &["diff", "--cached", "--name-only"])
                .stdout
                .is_empty()
        );

        let unborn = temp.path().join("unborn");
        init(&unborn);
        write(&unborn, "initial.txt", "initial\n");
        let unborn_service = service_for(&temp.path().join("unborn-home"));
        unborn_service.stage_all(&unborn, &control()).unwrap();
        unborn_service.unstage_all(&unborn, &control()).unwrap();
        assert!(
            output(Some(&unborn), &["diff", "--cached", "--name-only"])
                .stdout
                .is_empty()
        );
        assert!(unborn.join("initial.txt").is_file());
    }

    #[test]
    fn repository_mutate_uses_related_paths_stages_conflicts_and_handles_binary_files() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo);
        write(&repo, "original.txt", "same contents\n");
        write(&repo, "conflict.txt", "base\n");
        commit_fixture(&repo, "base");
        git(Some(&repo), &["config", "status.renames", "copies"]);
        git(Some(&repo), &["config", "diff.renames", "copies"]);
        git(Some(&repo), &["mv", "original.txt", "renamed.txt"]);
        std::fs::copy(repo.join("renamed.txt"), repo.join("copied.txt")).unwrap();
        write(&repo, "binary.bin", [0, 1, 2, 3]);
        let service = service_for(&temp.path().join("home"));
        let rename = changed(&service, &repo, b"renamed.txt");
        assert_eq!(rename.old_path.as_deref(), Some(b"original.txt".as_slice()));
        assert_eq!(
            service.stage(&repo, &rename, &control()).unwrap(),
            MutationOutcome::NoMutation
        );
        service
            .stage(&repo, &changed(&service, &repo, b"copied.txt"), &control())
            .unwrap();
        service
            .stage(&repo, &changed(&service, &repo, b"binary.bin"), &control())
            .unwrap();
        let staged = output(Some(&repo), &["diff", "--cached", "--name-status"]);
        let staged = String::from_utf8_lossy(&staged.stdout);
        assert!(staged.contains("original.txt"), "{staged}");
        assert!(staged.contains("renamed.txt"), "{staged}");
        assert!(staged.contains("copied.txt"), "{staged}");
        assert!(staged.contains("binary.bin"), "{staged}");

        let staged_rename = changed(&service, &repo, b"renamed.txt");
        service.unstage(&repo, &staged_rename, &control()).unwrap();
        assert!(
            !String::from_utf8_lossy(
                &output(Some(&repo), &["diff", "--cached", "--name-status"]).stdout
            )
            .contains("renamed.txt")
        );
        service.stage_all(&repo, &control()).unwrap();

        git(Some(&repo), &["reset", "--hard", "-q", "HEAD"]);
        git(Some(&repo), &["clean", "-fdq"]);
        git(Some(&repo), &["switch", "-qc", "other"]);
        write(&repo, "conflict.txt", "other\n");
        commit_fixture(&repo, "other");
        git(Some(&repo), &["switch", "-q", "main"]);
        write(&repo, "conflict.txt", "main\n");
        commit_fixture(&repo, "main");
        assert!(
            !command(Some(&repo), &["merge", "other"])
                .env("GIT_AUTHOR_NAME", "Muxy Tests")
                .env("GIT_AUTHOR_EMAIL", "muxy@example.invalid")
                .env("GIT_COMMITTER_NAME", "Muxy Tests")
                .env("GIT_COMMITTER_EMAIL", "muxy@example.invalid")
                .status()
                .unwrap()
                .success()
        );
        write(&repo, "conflict.txt", "resolved\n");
        let conflict = changed(&service, &repo, b"conflict.txt");
        assert!(conflict.is_conflicted);
        service.stage(&repo, &conflict, &control()).unwrap();
        assert!(
            !service.summary(&repo).unwrap().is_dirty()
                || service.summary(&repo).unwrap().conflicted_count == 0
        );
        assert_eq!(service.summary(&repo).unwrap().conflicted_count, 0);
    }

    #[test]
    fn repository_mutate_discards_tracked_untracked_and_rename_changes_and_rejects_stale_files() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo);
        write(&repo, "tracked.txt", "base\n");
        write(&repo, "original.txt", "rename\n");
        commit_fixture(&repo, "base");
        let service = service_for(&temp.path().join("home"));

        write(&repo, "tracked.txt", "changed\n");
        let tracked = changed(&service, &repo, b"tracked.txt");
        service.discard(&repo, &tracked, &control()).unwrap();
        assert_eq!(std::fs::read(repo.join("tracked.txt")).unwrap(), b"base\n");

        write(&repo, "untracked.txt", "remove\n");
        let untracked = changed(&service, &repo, b"untracked.txt");
        service.discard(&repo, &untracked, &control()).unwrap();
        assert!(!repo.join("untracked.txt").exists());

        std::fs::rename(repo.join("original.txt"), repo.join("renamed.txt")).unwrap();
        let renamed = changed(&service, &repo, b"renamed.txt");
        service.discard(&repo, &renamed, &control()).unwrap();
        assert!(repo.join("original.txt").is_file());
        assert!(!repo.join("renamed.txt").exists());

        write(&repo, "stale.txt", "stale\n");
        let stale = changed(&service, &repo, b"stale.txt");
        std::fs::remove_file(repo.join("stale.txt")).unwrap();
        assert!(matches!(
            service.discard(&repo, &stale, &control()),
            Err(RepositoryMutationError::StaleChangedFile)
        ));
    }

    #[test]
    fn repository_mutate_branches_validate_switch_create_and_confirm_forced_delete() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo);
        write(&repo, "base.txt", "base\n");
        commit_fixture(&repo, "base");
        git(Some(&repo), &["branch", "feature10"]);
        git(Some(&repo), &["branch", "feature2"]);
        let service = service_for(&temp.path().join("home"));

        assert_eq!(
            service.local_branches(&repo).unwrap(),
            [
                b"feature2".to_vec(),
                b"feature10".to_vec(),
                b"main".to_vec()
            ]
        );
        service
            .switch_branch(&repo, b"feature2", &control())
            .unwrap();
        assert_eq!(
            service
                .switch_branch(&repo, b"feature2", &control())
                .unwrap(),
            MutationOutcome::NoMutation
        );
        service
            .create_branch(&repo, b"created", &control())
            .unwrap();
        assert!(matches!(
            service.create_branch(&repo, b"created", &control()),
            Err(RepositoryMutationError::BranchExists)
        ));
        assert!(matches!(
            service.create_branch(&repo, b"bad branch", &control()),
            Err(RepositoryMutationError::InvalidBranch)
        ));
        assert!(matches!(
            service.prepare_branch_deletion(&repo, b"created", &control()),
            Err(RepositoryMutationError::CurrentBranch)
        ));

        write(&repo, "unmerged.txt", "unmerged\n");
        commit_fixture(&repo, "unmerged");
        service.switch_branch(&repo, b"main", &control()).unwrap();
        let stale_intent = service
            .prepare_branch_deletion(&repo, b"feature10", &control())
            .unwrap();
        git(Some(&repo), &["branch", "-f", "feature10", "created"]);
        assert!(matches!(
            service.delete_branch(&repo, &stale_intent, &control()),
            Err(RepositoryMutationError::StaleBranchDeletion)
        ));
        let intent = service
            .prepare_branch_deletion(&repo, b"created", &control())
            .unwrap();
        service.delete_branch(&repo, &intent, &control()).unwrap();
        assert!(
            !service
                .local_branches(&repo)
                .unwrap()
                .contains(&b"created".to_vec())
        );

        service
            .switch_branch(&repo, b"feature2", &control())
            .unwrap();
        write(&repo, "base.txt", "feature version\n");
        commit_fixture(&repo, "feature version");
        service.switch_branch(&repo, b"main", &control()).unwrap();
        write(&repo, "base.txt", "dirty\n");
        assert!(matches!(
            service.switch_branch(&repo, b"feature2", &control()),
            Err(RepositoryMutationError::Command {
                effect: MutationEffect::NoMutation,
                ..
            })
        ));
    }

    fn remote_fixture(root: &Path) -> (PathBuf, PathBuf, RepositoryService) {
        let origin = root.join("origin.git");
        git(
            None,
            &[
                "init",
                "--bare",
                "-q",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        let repo = root.join("repo");
        init(&repo);
        write(&repo, "base.txt", "base\n");
        commit_fixture(&repo, "base");
        git(
            Some(&repo),
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(Some(&repo), &["push", "-qu", "origin", "main"]);
        (origin, repo, service_for(&root.join("home")))
    }

    #[test]
    fn repository_mutate_switches_remote_branches_with_a_tracking_local_branch() {
        let temp = tempfile::tempdir().unwrap();
        let (_, repo, service) = remote_fixture(temp.path());
        git(Some(&repo), &["switch", "-qc", "remote-topic"]);
        write(&repo, "remote.txt", "remote\n");
        commit_fixture(&repo, "remote topic");
        git(Some(&repo), &["push", "-qu", "origin", "remote-topic"]);
        git(Some(&repo), &["switch", "-q", "main"]);
        git(Some(&repo), &["branch", "-D", "remote-topic"]);

        let entries = service.branch_entries(&repo).unwrap();
        assert!(entries.iter().any(|entry| {
            entry.kind == crate::repository::BranchKind::Remote
                && entry.name == b"origin/remote-topic"
                && entry.subject == "remote topic"
                && entry.author == "Muxy Tests"
        }));
        assert_eq!(
            service
                .switch_remote_branch(&repo, b"origin/remote-topic", &control())
                .unwrap(),
            MutationOutcome::Success
        );
        assert_eq!(
            String::from_utf8(output(Some(&repo), &["branch", "--show-current"]).stdout)
                .unwrap()
                .trim(),
            "remote-topic"
        );
        assert_eq!(
            String::from_utf8(
                output(Some(&repo), &["rev-parse", "--abbrev-ref", "@{upstream}"]).stdout
            )
            .unwrap()
            .trim(),
            "origin/remote-topic"
        );
        assert!(matches!(
            service.switch_remote_branch(&repo, b"invalid", &control()),
            Err(RepositoryMutationError::InvalidBranch)
        ));
    }

    #[test]
    fn repository_mutate_commits_pushes_with_and_without_upstream_and_pulls() {
        let temp = tempfile::tempdir().unwrap();
        let (origin, repo, service) = remote_fixture(temp.path());
        write(&repo, "local.txt", "local\n");
        service.stage_all(&repo, &control()).unwrap();
        service.commit(&repo, "local commit", &control()).unwrap();
        service.push(&repo, &control()).unwrap();
        assert_eq!(rev(&repo, "HEAD"), rev(&origin, "refs/heads/main"));

        service.create_branch(&repo, b"topic", &control()).unwrap();
        write(&repo, "topic.txt", "topic\n");
        service.stage_all(&repo, &control()).unwrap();
        service.commit(&repo, "topic", &control()).unwrap();
        service.push(&repo, &control()).unwrap();
        assert_eq!(
            String::from_utf8(
                output(Some(&repo), &["rev-parse", "--abbrev-ref", "@{upstream}"]).stdout
            )
            .unwrap()
            .trim(),
            "origin/topic"
        );

        service.switch_branch(&repo, b"main", &control()).unwrap();
        let other = temp.path().join("other");
        git(
            None,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        write(&other, "remote.txt", "remote\n");
        commit_fixture(&other, "remote");
        git(Some(&other), &["push", "-q"]);
        service.pull(&repo, &control()).unwrap();
        assert!(repo.join("remote.txt").is_file());
        assert!(matches!(
            service.commit(&repo, "   ", &control()),
            Err(RepositoryMutationError::EmptyMessage)
        ));
    }

    #[test]
    fn repository_mutate_updates_from_base_aborts_conflicts_and_reports_push_partial_success() {
        let temp = tempfile::tempdir().unwrap();
        let (origin, repo, service) = remote_fixture(temp.path());
        service
            .create_branch(&repo, b"feature", &control())
            .unwrap();
        write(&repo, "feature.txt", "feature\n");
        service.stage_all(&repo, &control()).unwrap();
        service.commit(&repo, "feature", &control()).unwrap();
        service.push(&repo, &control()).unwrap();
        let other = temp.path().join("other");
        git(
            None,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        );
        write(&other, "base-new.txt", "base\n");
        commit_fixture(&other, "base advance");
        git(Some(&other), &["push", "-q"]);

        service
            .update_from_base(&repo, b"main", &control())
            .unwrap();
        assert!(repo.join("base-new.txt").is_file());
        assert_eq!(rev(&repo, "HEAD"), rev(&origin, "refs/heads/feature"));

        service.switch_branch(&repo, b"main", &control()).unwrap();
        service.pull(&repo, &control()).unwrap();
        write(&repo, "conflict.txt", "main\n");
        commit_fixture(&repo, "main conflict");
        service.push(&repo, &control()).unwrap();
        service
            .create_branch(&repo, b"conflicting", &control())
            .unwrap();
        write(&repo, "conflict.txt", "feature\n");
        commit_fixture(&repo, "feature conflict");
        let conflicting_head = rev(&repo, "HEAD");
        let updater = temp.path().join("updater");
        git(
            None,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                updater.to_str().unwrap(),
            ],
        );
        write(&updater, "conflict.txt", "remote main\n");
        commit_fixture(&updater, "remote conflict");
        git(Some(&updater), &["push", "-q"]);
        assert!(matches!(
            service.update_from_base(&repo, b"main", &control()),
            Err(RepositoryMutationError::Command {
                effect: MutationEffect::NoMutation,
                ..
            })
        ));
        assert_eq!(rev(&repo, "HEAD"), conflicting_head);
        assert!(!repo.join(".git/MERGE_HEAD").exists());

        let partial_root = temp.path().join("partial");
        std::fs::create_dir(&partial_root).unwrap();
        let (partial_origin, partial_repo, partial_service) = remote_fixture(&partial_root);
        partial_service
            .create_branch(&partial_repo, b"feature", &control())
            .unwrap();
        write(&partial_repo, "feature.txt", "feature\n");
        commit_fixture(&partial_repo, "feature");
        partial_service.push(&partial_repo, &control()).unwrap();
        let partial_other = partial_root.join("other");
        git(
            None,
            &[
                "clone",
                "-q",
                partial_origin.to_str().unwrap(),
                partial_other.to_str().unwrap(),
            ],
        );
        write(&partial_other, "base-new.txt", "base\n");
        commit_fixture(&partial_other, "base advance");
        git(Some(&partial_other), &["push", "-q"]);
        let hook = partial_origin.join("hooks/pre-receive");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let remote_feature = rev(&partial_origin, "refs/heads/feature");
        assert!(matches!(
            partial_service.update_from_base(&partial_repo, b"main", &control()),
            Err(RepositoryMutationError::Command {
                effect: MutationEffect::PartialSuccess {
                    completed: "base merged"
                },
                ..
            })
        ));
        assert!(partial_repo.join("base-new.txt").is_file());
        assert_ne!(rev(&partial_repo, "HEAD"), remote_feature);
        assert_eq!(rev(&partial_origin, "refs/heads/feature"), remote_feature);
    }

    #[test]
    fn repository_mutate_revalidates_remote_context_before_network_dispatch() {
        for (key, value) in [
            ("branch.main.remote", "changed"),
            ("branch.main.merge", "refs/heads/changed"),
            ("branch.main.pushRemote", "changed"),
            ("remote.pushDefault", "changed"),
            ("remote.origin.url", "/missing/fetch.git"),
            ("remote.origin.pushurl", "/missing/push.git"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let (origin, repo, service) = remote_fixture(temp.path());
            write(&repo, "local.txt", "local\n");
            commit_fixture(&repo, "local");
            let origin_head = rev(&origin, "refs/heads/main");

            let result = service.push_with_boundary(&repo, &control(), || {
                git(Some(&repo), &["config", key, value]);
            });
            assert!(
                matches!(result, Err(RepositoryMutationError::RemoteContextChanged)),
                "{key}: {result:?}"
            );
            assert_eq!(rev(&origin, "refs/heads/main"), origin_head, "{key}");
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn repository_mutate_preserves_non_utf8_path_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        init(&repo);
        let raw = OsString::from_vec(b"raw-\xff.bin".to_vec());
        std::fs::write(repo.join(&raw), [0, 1, 2]).unwrap();
        let service = service_for(&temp.path().join("home"));
        let file = changed(&service, &repo, b"raw-\xff.bin");

        service.stage(&repo, &file, &control()).unwrap();

        let names = output(Some(&repo), &["diff", "--cached", "--name-only", "-z"]);
        assert!(
            names
                .stdout
                .split(|byte| *byte == 0)
                .any(|path| path == b"raw-\xff.bin")
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_mutate_sanitizes_redirects_and_bounds_timeout_cancellation_and_errors() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let selected = temp.path().join("selected");
        let redirected = temp.path().join("redirected");
        init(&selected);
        init(&redirected);
        write(&selected, "selected.txt", "selected\n");
        git(Some(&selected), &["add", "-A"]);
        let redirected_head = output(Some(&redirected), &["rev-parse", "HEAD"]);
        let mut variables = environment_for(&temp.path().join("home")).variables();
        variables.push((
            OsString::from("GIT_DIR"),
            redirected.join(".git").into_os_string(),
        ));
        variables.push((
            OsString::from("GIT_WORK_TREE"),
            redirected.clone().into_os_string(),
        ));
        let environment = ExecutionEnvironment::fallback(variables);
        let executable = environment.resolve_executable(OsStr::new("git")).unwrap();
        let service = RepositoryService::new(crate::repository::RepositoryOptions {
            git: GitOptions {
                executable,
                environment: HashMap::new(),
            },
            environment,
        });
        service.commit(&selected, "selected", &control()).unwrap();
        assert!(
            output(Some(&selected), &["rev-parse", "HEAD"])
                .status
                .success()
        );
        assert_eq!(
            output(Some(&redirected), &["rev-parse", "HEAD"])
                .status
                .success(),
            redirected_head.status.success()
        );

        let fake = temp.path().join("git-fake");
        std::fs::write(&fake, "#!/bin/sh\nsleep 5\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let environment = environment_for(&temp.path().join("fake-home"));
        let delayed = RepositoryService::new(crate::repository::RepositoryOptions {
            git: GitOptions {
                executable: fake.clone(),
                environment: HashMap::new(),
            },
            environment: environment.clone(),
        });
        let short = MutationControl::with_timeout(std::time::Duration::from_millis(50));
        assert!(matches!(
            delayed.commit(&selected, "timeout", &short),
            Err(RepositoryMutationError::Command {
                effect: MutationEffect::Uncertain,
                ..
            })
        ));
        let cancellation = crate::subprocess::CancellationSignal::new();
        cancellation.cancel();
        let cancelled = MutationControl::with_cancellation(cancellation);
        assert!(matches!(
            delayed.commit(&selected, "cancelled", &cancelled),
            Err(RepositoryMutationError::Command { .. })
        ));

        std::fs::write(&fake, "#!/bin/sh\nyes x | head -c 5000 >&2\nexit 9\n").unwrap();
        let error = delayed
            .commit(&selected, "failure", &control())
            .unwrap_err();
        assert!(error.to_string().len() < 2_000);
    }
}
