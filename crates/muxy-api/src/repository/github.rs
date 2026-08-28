use super::{
    MutationBoundary, MutationControl, MutationEffect, MutationOutcome, RepositoryMutationError,
    RepositoryService,
};
use crate::git::command::{RepositoryCommandRequest, repository_command, run_output};
use crate::git::validate_branch;
use crate::subprocess::{
    CancellationSignal, Deadline, EnvironmentMode, StdinMode, SubprocessError, SubprocessOutput,
    SubprocessRequest, bounded_error_text,
};
use std::ffi::OsString;
use std::fmt;
use std::path::Path;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const GH_STDOUT_LIMIT: usize = 2 * 1_024 * 1_024;
const GH_STDERR_LIMIT: usize = 1_024 * 1_024;
const GIT_STDOUT_LIMIT: usize = 16 * 1_024 * 1_024;
const PR_JSON_FIELDS: &str = "url,number,state,isDraft,baseRefName,mergeable,mergeStateStatus,statusCheckRollup,isCrossRepository,headRefOid,headRefName";

#[derive(Clone, Debug)]
pub struct GitHubControl {
    timeout: Duration,
    cancellation: Option<CancellationSignal>,
    boundary: MutationBoundary,
}

impl Default for GitHubControl {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            cancellation: None,
            boundary: MutationBoundary::default(),
        }
    }
}

impl GitHubControl {
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

    fn mutation_control(&self) -> MutationControl {
        MutationControl::from_parts_with_boundary(
            self.timeout,
            self.cancellation.clone(),
            self.boundary.clone(),
        )
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(CancellationSignal::is_cancelled)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedExternalUrl(String);

impl ValidatedExternalUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parse(value: String) -> Result<Self, PullRequestParseError> {
        let Some(remainder) = value.strip_prefix("https://") else {
            return Err(PullRequestParseError::Url);
        };
        let Some((host, path)) = remainder.split_once('/') else {
            return Err(PullRequestParseError::Url);
        };
        if host.is_empty()
            || path.is_empty()
            || host.contains('@')
            || value
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return Err(PullRequestParseError::Url);
        }
        Ok(Self(value))
    }
}

impl TryFrom<String> for ValidatedExternalUrl {
    type Error = PullRequestParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl fmt::Display for ValidatedExternalUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRepositoryIdentity {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl GitHubRepositoryIdentity {
    pub fn repository_argument(&self) -> String {
        format!("{}/{}/{}", self.host, self.owner, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
    Unknown(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestMergeable {
    Mergeable,
    Conflicting,
    Unknown(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestMergeState {
    Clean,
    HasHooks,
    Unstable,
    Behind,
    Blocked,
    Dirty,
    Draft,
    Unknown(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestChecksStatus {
    None,
    Pending,
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullRequestChecks {
    pub status: PullRequestChecksStatus,
    pub passing: usize,
    pub failing: usize,
    pub pending: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestInfo {
    pub url: ValidatedExternalUrl,
    pub number: u64,
    pub state: PullRequestState,
    pub is_draft: bool,
    pub base_branch: String,
    pub mergeable: PullRequestMergeable,
    pub merge_state: PullRequestMergeState,
    pub checks: PullRequestChecks,
    pub is_cross_repository: bool,
    pub head_oid: String,
    pub head_branch: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PullRequestParseError {
    #[error("malformed pull request JSON")]
    Json,
    #[error("unsupported pull request JSON shape")]
    Shape,
    #[error("pull request URL is invalid")]
    Url,
    #[error("GitHub repository identity is invalid")]
    RepositoryIdentity,
    #[error("pull request head no longer matches")]
    HeadIdentity,
    #[error("pull request JSON exceeds the allowed size")]
    Oversized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePullRequestRequest {
    pub branch: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PullRequestMergeMethod {
    Squash,
    Merge,
    Rebase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreatePullRequestOutcome {
    Created(Box<ResolvedPullRequest>),
    CreatedUnreadable {
        url: Option<ValidatedExternalUrl>,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestMergeOutcome {
    Success,
    SuccessWithWarning(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRepositoryContext {
    pub identity: GitHubRepositoryIdentity,
    remote: RemoteConfigurationSnapshot,
    branch: Vec<u8>,
    head_oid: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPullRequest {
    pub info: PullRequestInfo,
    repository: GitHubRepositoryContext,
    branch: Vec<u8>,
    head_oid: Vec<u8>,
    number: u64,
    url: ValidatedExternalUrl,
    base_branch: String,
}

impl ResolvedPullRequest {
    pub fn repository_identity(&self) -> &GitHubRepositoryIdentity {
        &self.repository.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PullRequestLookup {
    Found(Box<ResolvedPullRequest>),
    NoPullRequest(Box<GitHubRepositoryContext>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RemoteConfigurationSnapshot {
    configuration: Vec<u8>,
    upstream: Option<Vec<u8>>,
    push_ref: Option<Vec<u8>>,
    push_remote: Vec<u8>,
    push_url: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHubMutationEffect {
    NoMutation,
    Uncertain,
    PartialSuccess,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("GitHub CLI (gh) is unavailable")]
    MissingExecutable,
    #[error("GitHub {operation} was cancelled after {effect:?}")]
    Cancelled {
        operation: &'static str,
        effect: GitHubMutationEffect,
    },
    #[error("GitHub {operation} process failed after {effect:?}: {source}")]
    Process {
        operation: &'static str,
        effect: GitHubMutationEffect,
        #[source]
        source: Box<SubprocessError>,
    },
    #[error("GitHub {operation} output was truncated after {effect:?}")]
    Truncated {
        operation: &'static str,
        effect: GitHubMutationEffect,
    },
    #[error("GitHub {operation} failed after {effect:?}: {message}")]
    Command {
        operation: &'static str,
        effect: GitHubMutationEffect,
        message: String,
    },
    #[error("GitHub {operation} returned invalid data: {source}")]
    Parse {
        operation: &'static str,
        #[source]
        source: PullRequestParseError,
    },
    #[error("GitHub repository read failed: {message}")]
    RepositoryRead { message: String },
    #[error("remote context changed")]
    RemoteContextChanged,
    #[error("pull request identity changed")]
    StalePullRequest,
    #[error("pull request already exists")]
    PullRequestExists,
    #[error("pull request is not open")]
    PullRequestNotOpen,
    #[error("draft pull requests cannot be merged")]
    DraftPullRequest,
    #[error("pull request cannot be merged")]
    PullRequestNotMergeable,
    #[error("pull request branch is behind its base")]
    PullRequestBehind,
    #[error("pull request merge is blocked")]
    PullRequestBlocked,
    #[error("tracked changes must be clean")]
    DirtyRepository,
    #[error("cross-repository pull requests cannot be updated locally")]
    CrossRepositoryUpdate,
    #[error("branch name is invalid")]
    InvalidBranch,
    #[error("repository action failed after {effect:?}: {source}")]
    RepositoryMutation {
        effect: GitHubMutationEffect,
        #[source]
        source: Box<RepositoryMutationError>,
    },
}

impl GitHubError {
    pub fn mutation_effect(&self) -> Option<GitHubMutationEffect> {
        match self {
            Self::Cancelled { effect, .. }
            | Self::Process { effect, .. }
            | Self::Truncated { effect, .. }
            | Self::Command { effect, .. }
            | Self::RepositoryMutation { effect, .. } => Some(*effect),
            _ => None,
        }
    }
}

enum ViewResult {
    Found(Box<PullRequestInfo>),
    NoPullRequest,
}

impl RepositoryService {
    pub fn pull_request(
        &self,
        repository: &Path,
        branch: &[u8],
        head_oid: &[u8],
        control: &GitHubControl,
    ) -> Result<PullRequestLookup, GitHubError> {
        validate_pull_request_identity(branch, head_oid)?;
        let deadline = Deadline::new(control.timeout);
        self.ensure_local_identity(repository, branch, head_oid, control, &deadline)?;
        let context =
            self.bootstrap_github_repository(repository, branch, head_oid, control, &deadline)?;
        self.lookup_with_context(repository, context, control, &deadline)
    }

    pub fn create_pull_request(
        &self,
        repository: &Path,
        context: &GitHubRepositoryContext,
        request: &CreatePullRequestRequest,
        control: &GitHubControl,
    ) -> Result<CreatePullRequestOutcome, GitHubError> {
        if request.branch.as_bytes() != context.branch {
            return Err(GitHubError::StalePullRequest);
        }
        validate_branch(&request.branch).map_err(|_| GitHubError::InvalidBranch)?;
        validate_branch(&request.base).map_err(|_| GitHubError::InvalidBranch)?;
        let deadline = Deadline::new(control.timeout);
        self.ensure_github_context(repository, context, control, &deadline)?;
        match self.lookup_with_context(repository, context.clone(), control, &deadline)? {
            PullRequestLookup::Found(_) => return Err(GitHubError::PullRequestExists),
            PullRequestLookup::NoPullRequest(_) => {}
        }
        self.ensure_mutation_boundary(repository, context, control, &deadline)?;
        let repository_argument = context.identity.repository_argument();
        let mut args = string_args(&[
            "pr",
            "create",
            "--repo",
            &repository_argument,
            "--head",
            &request.branch,
            "--base",
            &request.base,
            "--title",
            &request.title,
            "--body",
            &request.body,
        ]);
        if request.draft {
            args.push(OsString::from("--draft"));
        }
        let output = self.gh_success(
            repository,
            "create pull request",
            args,
            GitHubMutationEffect::Uncertain,
            control,
            &deadline,
        )?;
        let created_url = output
            .stdout
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|token| !token.is_empty())
            .filter_map(|token| std::str::from_utf8(token).ok())
            .find(|token| token.starts_with("https://"))
            .and_then(|value| ValidatedExternalUrl::parse(value.to_owned()).ok());
        let Some(view_argument) = created_url.as_ref().map(|url| url.as_str().to_owned()) else {
            return Ok(CreatePullRequestOutcome::CreatedUnreadable {
                url: None,
                message: "Pull request was created but could not be read back".to_owned(),
            });
        };
        match self.gh_pr_view(repository, context, &view_argument, control, &deadline) {
            Ok(ViewResult::Found(info)) => Ok(CreatePullRequestOutcome::Created(Box::new(
                resolved_pull_request(context.clone(), *info),
            ))),
            Ok(ViewResult::NoPullRequest) | Err(_) => {
                Ok(CreatePullRequestOutcome::CreatedUnreadable {
                    url: created_url,
                    message: "Pull request was created but could not be read back".to_owned(),
                })
            }
        }
    }

    pub fn merge_pull_request(
        &self,
        repository: &Path,
        resolved: &ResolvedPullRequest,
        method: PullRequestMergeMethod,
        control: &GitHubControl,
    ) -> Result<PullRequestMergeOutcome, GitHubError> {
        let deadline = Deadline::new(control.timeout);
        let current = self.revalidate_pull_request(repository, resolved, control, &deadline)?;
        self.ensure_merge_available(repository, &current, control, &deadline)?;
        self.ensure_mutation_boundary(repository, &resolved.repository, control, &deadline)?;
        let number = current.number.to_string();
        let repository_argument = resolved.repository.identity.repository_argument();
        let flag = match method {
            PullRequestMergeMethod::Squash => "--squash",
            PullRequestMergeMethod::Merge => "--merge",
            PullRequestMergeMethod::Rebase => "--rebase",
        };
        self.gh_success(
            repository,
            "merge pull request",
            string_args(&["pr", "merge", &number, "--repo", &repository_argument, flag]),
            GitHubMutationEffect::Uncertain,
            control,
            &deadline,
        )?;
        if control.is_cancelled() {
            return Ok(PullRequestMergeOutcome::SuccessWithWarning(
                "Pull request merged; local update was cancelled".to_owned(),
            ));
        }
        let mutation_control = control.mutation_control();
        let follow_up = self
            .switch_branch(
                repository,
                current.base_branch.as_bytes(),
                &mutation_control,
            )
            .and_then(|_| self.pull(repository, &mutation_control));
        match follow_up {
            Ok(_) => Ok(PullRequestMergeOutcome::Success),
            Err(error) => Ok(PullRequestMergeOutcome::SuccessWithWarning(format!(
                "Pull request merged; local update failed: {}",
                bounded_message(&error.to_string())
            ))),
        }
    }

    pub fn close_pull_request(
        &self,
        repository: &Path,
        resolved: &ResolvedPullRequest,
        control: &GitHubControl,
    ) -> Result<(), GitHubError> {
        let deadline = Deadline::new(control.timeout);
        let current = self.revalidate_pull_request(repository, resolved, control, &deadline)?;
        if current.state != PullRequestState::Open {
            return Err(GitHubError::PullRequestNotOpen);
        }
        self.ensure_mutation_boundary(repository, &resolved.repository, control, &deadline)?;
        let number = current.number.to_string();
        let repository_argument = resolved.repository.identity.repository_argument();
        self.gh_success(
            repository,
            "close pull request",
            string_args(&["pr", "close", &number, "--repo", &repository_argument]),
            GitHubMutationEffect::Uncertain,
            control,
            &deadline,
        )?;
        Ok(())
    }

    pub fn update_pull_request(
        &self,
        repository: &Path,
        resolved: &ResolvedPullRequest,
        control: &GitHubControl,
    ) -> Result<MutationOutcome, GitHubError> {
        let deadline = Deadline::new(control.timeout);
        let current = self.revalidate_pull_request(repository, resolved, control, &deadline)?;
        if current.state != PullRequestState::Open {
            return Err(GitHubError::PullRequestNotOpen);
        }
        if current.is_cross_repository {
            return Err(GitHubError::CrossRepositoryUpdate);
        }
        if current.merge_state != PullRequestMergeState::Behind {
            return Err(GitHubError::PullRequestNotMergeable);
        }
        if !self.tracked_clean(repository, control, &deadline)? {
            return Err(GitHubError::DirtyRepository);
        }
        self.update_from_base(
            repository,
            current.base_branch.as_bytes(),
            &control.mutation_control(),
        )
        .map_err(map_repository_mutation)
    }

    fn lookup_with_context(
        &self,
        repository: &Path,
        context: GitHubRepositoryContext,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<PullRequestLookup, GitHubError> {
        let mut failure = None;
        if let Some(number) =
            self.configured_pull_request_number(repository, &context.branch, control, deadline)?
        {
            match self.gh_pr_view(repository, &context, &number.to_string(), control, deadline) {
                Ok(ViewResult::Found(info)) => {
                    return Ok(PullRequestLookup::Found(Box::new(resolved_pull_request(
                        context, *info,
                    ))));
                }
                Ok(ViewResult::NoPullRequest) => {}
                Err(error) => failure = Some(error),
            }
        }
        let branch = std::str::from_utf8(&context.branch)
            .map_err(|_| GitHubError::InvalidBranch)?
            .to_owned();
        match self.gh_pr_view(repository, &context, branch.as_str(), control, deadline) {
            Ok(ViewResult::Found(info)) => {
                return Ok(PullRequestLookup::Found(Box::new(resolved_pull_request(
                    context, *info,
                ))));
            }
            Ok(ViewResult::NoPullRequest) => {}
            Err(error) => failure = Some(error),
        }
        match self.gh_pr_list(repository, &context, control, deadline) {
            Ok(Some(info)) => Ok(PullRequestLookup::Found(Box::new(resolved_pull_request(
                context, info,
            )))),
            Ok(None) => match failure {
                Some(error) => Err(error),
                None => Ok(PullRequestLookup::NoPullRequest(Box::new(context))),
            },
            Err(error) => Err(error),
        }
    }

    fn bootstrap_github_repository(
        &self,
        repository: &Path,
        branch: &[u8],
        head_oid: &[u8],
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<GitHubRepositoryContext, GitHubError> {
        let remote = self.capture_remote_snapshot(repository, branch, control, deadline)?;
        let output = self.gh_success(
            repository,
            "repository identity",
            string_args(&["repo", "view", "--json", "nameWithOwner,url"]),
            GitHubMutationEffect::NoMutation,
            control,
            deadline,
        )?;
        let identity = super::parse::parse_github_repository(&output.stdout).map_err(|source| {
            GitHubError::Parse {
                operation: "repository identity",
                source,
            }
        })?;
        if self.capture_remote_snapshot(repository, branch, control, deadline)? != remote {
            return Err(GitHubError::RemoteContextChanged);
        }
        Ok(GitHubRepositoryContext {
            identity,
            remote,
            branch: branch.to_vec(),
            head_oid: head_oid.to_vec(),
        })
    }

    fn ensure_github_context(
        &self,
        repository: &Path,
        context: &GitHubRepositoryContext,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<(), GitHubError> {
        self.ensure_local_identity(
            repository,
            &context.branch,
            &context.head_oid,
            control,
            deadline,
        )?;
        if self.capture_remote_snapshot(repository, &context.branch, control, deadline)?
            != context.remote
        {
            return Err(GitHubError::RemoteContextChanged);
        }
        let current = self.bootstrap_github_repository(
            repository,
            &context.branch,
            &context.head_oid,
            control,
            deadline,
        )?;
        if current != *context {
            return Err(GitHubError::RemoteContextChanged);
        }
        Ok(())
    }

    fn ensure_mutation_boundary(
        &self,
        repository: &Path,
        context: &GitHubRepositoryContext,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<(), GitHubError> {
        self.ensure_local_identity(
            repository,
            &context.branch,
            &context.head_oid,
            control,
            deadline,
        )?;
        if self.capture_remote_snapshot(repository, &context.branch, control, deadline)?
            != context.remote
        {
            return Err(GitHubError::RemoteContextChanged);
        }
        Ok(())
    }

    fn revalidate_pull_request(
        &self,
        repository: &Path,
        resolved: &ResolvedPullRequest,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<PullRequestInfo, GitHubError> {
        self.ensure_github_context(repository, &resolved.repository, control, deadline)?;
        let argument = resolved.number.to_string();
        let ViewResult::Found(current) = self.gh_pr_view(
            repository,
            &resolved.repository,
            &argument,
            control,
            deadline,
        )?
        else {
            return Err(GitHubError::StalePullRequest);
        };
        if current.number != resolved.number
            || current.url != resolved.url
            || current.base_branch != resolved.base_branch
            || current.head_branch.as_bytes() != resolved.branch
            || current.head_oid.as_bytes() != resolved.head_oid
        {
            return Err(GitHubError::StalePullRequest);
        }
        Ok(*current)
    }

    fn ensure_merge_available(
        &self,
        repository: &Path,
        info: &PullRequestInfo,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<(), GitHubError> {
        if info.state != PullRequestState::Open {
            return Err(GitHubError::PullRequestNotOpen);
        }
        if info.is_draft {
            return Err(GitHubError::DraftPullRequest);
        }
        if info.mergeable == PullRequestMergeable::Conflicting {
            return Err(GitHubError::PullRequestNotMergeable);
        }
        match info.merge_state {
            PullRequestMergeState::Behind => return Err(GitHubError::PullRequestBehind),
            PullRequestMergeState::Blocked
            | PullRequestMergeState::Dirty
            | PullRequestMergeState::Draft => return Err(GitHubError::PullRequestBlocked),
            PullRequestMergeState::Clean
            | PullRequestMergeState::HasHooks
            | PullRequestMergeState::Unstable
            | PullRequestMergeState::Unknown(_) => {}
        }
        if !self.tracked_clean(repository, control, deadline)? {
            return Err(GitHubError::DirtyRepository);
        }
        Ok(())
    }

    fn tracked_clean(
        &self,
        repository: &Path,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<bool, GitHubError> {
        self.git_required(
            repository,
            "tracked status",
            string_args(&["status", "--porcelain=1", "--untracked-files=no"]),
            control,
            deadline,
        )
        .map(|output| output.stdout.is_empty())
    }

    fn ensure_local_identity(
        &self,
        repository: &Path,
        branch: &[u8],
        head_oid: &[u8],
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<(), GitHubError> {
        let current_branch = self.git_required(
            repository,
            "current branch",
            string_args(&["symbolic-ref", "--quiet", "--short", "HEAD"]),
            control,
            deadline,
        )?;
        let current_head = self.git_required(
            repository,
            "current head",
            string_args(&["rev-parse", "HEAD"]),
            control,
            deadline,
        )?;
        if trim_output(&current_branch.stdout) != Some(branch)
            || !trim_output(&current_head.stdout).is_some_and(|current| {
                current.len() == head_oid.len() && current.eq_ignore_ascii_case(head_oid)
            })
        {
            return Err(GitHubError::StalePullRequest);
        }
        Ok(())
    }

    fn configured_pull_request_number(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<Option<u64>, GitHubError> {
        let branch = std::str::from_utf8(branch).map_err(|_| GitHubError::InvalidBranch)?;
        let key = format!("branch.{branch}.muxy-pr-number");
        let Some(output) = self.git_optional(
            repository,
            "configured pull request",
            string_args(&["config", "--get", &key]),
            control,
            deadline,
        )?
        else {
            return Ok(None);
        };
        Ok(trim_output(&output).and_then(|value| {
            std::str::from_utf8(value)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|number| *number > 0)
        }))
    }

    fn capture_remote_snapshot(
        &self,
        repository: &Path,
        branch: &[u8],
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<RemoteConfigurationSnapshot, GitHubError> {
        let configuration = self
            .git_optional(
                repository,
                "remote configuration",
                string_args(&[
                    "config",
                    "--null",
                    "--get-regexp",
                    "^(remote\\..*\\.(url|pushurl)|remote\\.pushdefault|branch\\..*\\.(remote|pushremote|merge))$",
                ]),
                control,
                deadline,
            )?
            .unwrap_or_default();
        let upstream = self.git_optional(
            repository,
            "upstream reference",
            string_args(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ]),
            control,
            deadline,
        )?;
        let push_ref = self.git_optional(
            repository,
            "push reference",
            string_args(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{push}",
            ]),
            control,
            deadline,
        )?;
        let branch = std::str::from_utf8(branch).map_err(|_| GitHubError::InvalidBranch)?;
        let branch_push_remote = self.git_config_value(
            repository,
            &format!("branch.{branch}.pushRemote"),
            control,
            deadline,
        )?;
        let default_push_remote =
            self.git_config_value(repository, "remote.pushDefault", control, deadline)?;
        let branch_remote = self.git_config_value(
            repository,
            &format!("branch.{branch}.remote"),
            control,
            deadline,
        )?;
        let upstream_remote = upstream
            .as_deref()
            .and_then(trim_output)
            .and_then(|reference| reference.split(|byte| *byte == b'/').next())
            .filter(|remote| !remote.is_empty())
            .map(<[u8]>::to_vec);
        let push_remote = branch_push_remote
            .or(default_push_remote)
            .or(branch_remote)
            .or(upstream_remote)
            .unwrap_or_else(|| b"origin".to_vec());
        let remote = std::str::from_utf8(&push_remote).map_err(|_| GitHubError::InvalidBranch)?;
        let push_url = self.git_optional(
            repository,
            "push URL",
            string_args(&["remote", "get-url", "--push", remote]),
            control,
            deadline,
        )?;
        Ok(RemoteConfigurationSnapshot {
            configuration,
            upstream,
            push_ref,
            push_remote,
            push_url,
        })
    }

    fn git_config_value(
        &self,
        repository: &Path,
        key: &str,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<Option<Vec<u8>>, GitHubError> {
        self.git_optional(
            repository,
            "remote configuration value",
            string_args(&["config", "--get", key]),
            control,
            deadline,
        )
        .map(|value| value.and_then(|value| trim_output(&value).map(<[u8]>::to_vec)))
    }

    fn gh_pr_view(
        &self,
        repository: &Path,
        context: &GitHubRepositoryContext,
        argument: &str,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<ViewResult, GitHubError> {
        let repository_argument = context.identity.repository_argument();
        let mut args = string_args(&["pr", "view"]);
        args.push(OsString::from(argument));
        args.extend(string_args(&[
            "--repo",
            &repository_argument,
            "--json",
            PR_JSON_FIELDS,
        ]));
        let output = self.run_gh(
            repository,
            "read pull request",
            args,
            GitHubMutationEffect::NoMutation,
            control,
            deadline,
        )?;
        if !output.status.success() {
            if no_pull_request_error(&output.stderr) {
                return Ok(ViewResult::NoPullRequest);
            }
            return Err(command_failure(
                "read pull request",
                GitHubMutationEffect::NoMutation,
                &output,
            ));
        }
        let info =
            super::parse::parse_pull_request(&output.stdout, &context.branch, &context.head_oid)
                .map_err(|source| match source {
                    PullRequestParseError::HeadIdentity => GitHubError::StalePullRequest,
                    source => GitHubError::Parse {
                        operation: "read pull request",
                        source,
                    },
                })?;
        Ok(ViewResult::Found(Box::new(info)))
    }

    fn gh_pr_list(
        &self,
        repository: &Path,
        context: &GitHubRepositoryContext,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<Option<PullRequestInfo>, GitHubError> {
        let repository_argument = context.identity.repository_argument();
        let branch =
            std::str::from_utf8(&context.branch).map_err(|_| GitHubError::InvalidBranch)?;
        let output = self.run_gh(
            repository,
            "list pull requests",
            string_args(&[
                "pr",
                "list",
                "--repo",
                &repository_argument,
                "--state",
                "all",
                "--head",
                branch,
                "--limit",
                "100",
                "--json",
                PR_JSON_FIELDS,
            ]),
            GitHubMutationEffect::NoMutation,
            control,
            deadline,
        )?;
        if !output.status.success() {
            if no_pull_request_error(&output.stderr) {
                return Ok(None);
            }
            return Err(command_failure(
                "list pull requests",
                GitHubMutationEffect::NoMutation,
                &output,
            ));
        }
        super::parse::parse_pull_request_list(&output.stdout, &context.branch, &context.head_oid)
            .map_err(|source| GitHubError::Parse {
                operation: "list pull requests",
                source,
            })
    }

    fn gh_success(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        effect: GitHubMutationEffect,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, GitHubError> {
        let output = self.run_gh(repository, operation, args, effect, control, deadline)?;
        if !output.status.success() {
            return Err(command_failure(operation, effect, &output));
        }
        Ok(output)
    }

    fn run_gh(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        effect: GitHubMutationEffect,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, GitHubError> {
        if control.is_cancelled() {
            return Err(GitHubError::Cancelled { operation, effect });
        }
        let executable = self
            .options
            .environment
            .resolve_executable("gh".as_ref())
            .ok_or(GitHubError::MissingExecutable)?;
        let irreversible = effect != GitHubMutationEffect::NoMutation;
        if irreversible && !control.boundary.begin_irreversible() {
            return Err(GitHubError::Cancelled { operation, effect });
        }
        let output = crate::subprocess::run(
            SubprocessRequest {
                executable,
                args,
                current_dir: Some(repository.to_path_buf()),
                stdin: StdinMode::Closed,
                environment: EnvironmentMode::Replace(self.options.environment.github_variables()),
                stdout_limit: GH_STDOUT_LIMIT,
                stderr_limit: GH_STDERR_LIMIT,
                cancellation: control.cancellation.clone(),
            },
            Some(deadline),
        );
        if irreversible
            && control.boundary.finish_irreversible()
            && let Some(cancellation) = &control.cancellation
        {
            cancellation.cancel();
        }
        let output = output.map_err(|source| GitHubError::Process {
            operation,
            effect,
            source: Box::new(source),
        })?;
        if output.stdout_truncated || output.stderr_truncated {
            return Err(GitHubError::Truncated { operation, effect });
        }
        Ok(output)
    }

    fn git_required(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, GitHubError> {
        let output = self.run_git_read(repository, operation, args, control, deadline)?;
        if !output.status.success() {
            return Err(GitHubError::RepositoryRead {
                message: bounded_error_text(&output.stderr),
            });
        }
        Ok(output)
    }

    fn git_optional(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<Option<Vec<u8>>, GitHubError> {
        let output = self.run_git_read(repository, operation, args, control, deadline)?;
        Ok(output.status.success().then_some(output.stdout))
    }

    fn run_git_read(
        &self,
        repository: &Path,
        operation: &'static str,
        args: Vec<OsString>,
        control: &GitHubControl,
        deadline: &Deadline,
    ) -> Result<SubprocessOutput, GitHubError> {
        if control.is_cancelled() {
            return Err(GitHubError::Cancelled {
                operation,
                effect: GitHubMutationEffect::NoMutation,
            });
        }
        let command = repository_command(
            &self.options.environment,
            RepositoryCommandRequest {
                args,
                read_only: true,
                network: false,
                stdin: StdinMode::Closed,
                stdout_limit: GIT_STDOUT_LIMIT,
                stderr_limit: GH_STDERR_LIMIT,
                cancellation: control.cancellation.clone(),
            },
        );
        let output =
            run_output(&self.options.git, repository, command, deadline).map_err(|source| {
                GitHubError::RepositoryRead {
                    message: bounded_message(&source.to_string()),
                }
            })?;
        if output.stdout_truncated || output.stderr_truncated {
            return Err(GitHubError::Truncated {
                operation,
                effect: GitHubMutationEffect::NoMutation,
            });
        }
        Ok(output)
    }
}

fn validate_pull_request_identity(branch: &[u8], head_oid: &[u8]) -> Result<(), GitHubError> {
    let branch = std::str::from_utf8(branch).map_err(|_| GitHubError::InvalidBranch)?;
    validate_branch(branch).map_err(|_| GitHubError::InvalidBranch)?;
    if !matches!(head_oid.len(), 40 | 64) || !head_oid.iter().all(u8::is_ascii_hexdigit) {
        return Err(GitHubError::StalePullRequest);
    }
    Ok(())
}

fn resolved_pull_request(
    repository: GitHubRepositoryContext,
    info: PullRequestInfo,
) -> ResolvedPullRequest {
    ResolvedPullRequest {
        branch: repository.branch.clone(),
        head_oid: repository.head_oid.clone(),
        number: info.number,
        url: info.url.clone(),
        base_branch: info.base_branch.clone(),
        repository,
        info,
    }
}

fn no_pull_request_error(stderr: &[u8]) -> bool {
    let lowered = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "no pull requests found",
        "no pull request found",
        "could not resolve",
        "no commits between",
    ]
    .iter()
    .any(|message| lowered.contains(message))
}

fn command_failure(
    operation: &'static str,
    effect: GitHubMutationEffect,
    output: &SubprocessOutput,
) -> GitHubError {
    GitHubError::Command {
        operation,
        effect,
        message: bounded_error_text(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        }),
    }
}

fn map_repository_mutation(source: RepositoryMutationError) -> GitHubError {
    let effect = match source.effect() {
        MutationEffect::NoMutation => GitHubMutationEffect::NoMutation,
        MutationEffect::Uncertain => GitHubMutationEffect::Uncertain,
        MutationEffect::PartialSuccess { .. } => GitHubMutationEffect::PartialSuccess,
    };
    GitHubError::RepositoryMutation {
        effect,
        source: Box::new(source),
    }
}

fn string_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn trim_output(bytes: &[u8]) -> Option<&[u8]> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    (!bytes.is_empty()).then_some(bytes)
}

fn bounded_message(message: &str) -> String {
    bounded_error_text(message.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_environment::ExecutionEnvironment;
    use crate::git::GitOptions;
    use crate::repository::parse;
    use crate::repository::{MutationOutcome, RepositoryOptions, RepositoryService};
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    #[cfg(unix)]
    struct GitHubFixture {
        _temp: tempfile::TempDir,
        repository: PathBuf,
        service: RepositoryService,
        log: PathBuf,
        created: PathBuf,
        head: Vec<u8>,
    }

    #[cfg(unix)]
    impl GitHubFixture {
        fn new(scenario: &str, with_gh: bool) -> Self {
            use std::os::unix::fs::PermissionsExt;

            let temp = tempfile::tempdir().unwrap();
            let repository = temp.path().join("repository");
            let origin = temp.path().join("origin.git");
            git(None, &["init", "-q", "--bare", origin.to_str().unwrap()]);
            git(
                None,
                &["init", "-q", "-b", "main", repository.to_str().unwrap()],
            );
            git(Some(&repository), &["config", "user.name", "Muxy Tests"]);
            git(
                Some(&repository),
                &["config", "user.email", "muxy@example.test"],
            );
            std::fs::write(repository.join("tracked"), "main\n").unwrap();
            git(Some(&repository), &["add", "tracked"]);
            git(Some(&repository), &["commit", "-q", "-m", "initial"]);
            git(
                Some(&repository),
                &["remote", "add", "origin", origin.to_str().unwrap()],
            );
            git(Some(&repository), &["push", "-q", "-u", "origin", "main"]);
            git(Some(&repository), &["switch", "-q", "-c", "topic"]);
            std::fs::write(repository.join("topic"), "topic\n").unwrap();
            git(Some(&repository), &["add", "topic"]);
            git(Some(&repository), &["commit", "-q", "-m", "topic"]);
            git(Some(&repository), &["push", "-q", "-u", "origin", "topic"]);
            let head = output(Some(&repository), &["rev-parse", "HEAD"])
                .stdout
                .strip_suffix(b"\n")
                .unwrap()
                .to_vec();
            let bin = temp.path().join("bin");
            std::fs::create_dir(&bin).unwrap();
            if with_gh {
                let gh = bin.join("gh");
                std::fs::write(&gh, fake_gh()).unwrap();
                std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            let log = temp.path().join("gh.log");
            let created = temp.path().join("created");
            let path = if with_gh {
                std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                )))
                .unwrap()
            } else {
                bin.into_os_string()
            };
            let merge_state = if matches!(scenario, "update" | "update-cross") {
                "BEHIND"
            } else {
                "CLEAN"
            };
            let cross_repository = scenario == "update-cross";
            let pr_head = if scenario == "stale" {
                "f".repeat(40)
            } else {
                String::from_utf8_lossy(&head).into_owned()
            };
            let pr_json = format!(
                "{{\"url\":\"https://github.com/muxy/repo/pull/42\",\"number\":42,\"state\":\"OPEN\",\"isDraft\":false,\"baseRefName\":\"main\",\"mergeable\":\"MERGEABLE\",\"mergeStateStatus\":\"{merge_state}\",\"statusCheckRollup\":[],\"isCrossRepository\":{cross_repository},\"headRefOid\":\"{pr_head}\",\"headRefName\":\"topic\"}}",
            );
            let variables = [
                (OsString::from("PATH"), path),
                (
                    OsString::from("HOME"),
                    temp.path().join("home").into_os_string(),
                ),
                (OsString::from("GH_REPO"), OsString::from("wrong/target")),
                (OsString::from("GH_LOG"), log.as_os_str().to_owned()),
                (OsString::from("GH_CREATED"), created.as_os_str().to_owned()),
                (OsString::from("GH_SCENARIO"), OsString::from(scenario)),
                (OsString::from("GH_PR_JSON"), OsString::from(pr_json)),
                (
                    OsString::from("GH_ERROR"),
                    OsString::from("x".repeat(2_000)),
                ),
            ];
            let environment = if with_gh {
                ExecutionEnvironment::fallback(variables)
            } else {
                ExecutionEnvironment::exact(variables)
            };
            let executable = ExecutionEnvironment::from_current_process()
                .resolve_executable(OsStr::new("git"))
                .unwrap();
            let service = RepositoryService::new(RepositoryOptions {
                git: GitOptions {
                    executable,
                    environment: HashMap::new(),
                },
                environment,
            });
            Self {
                _temp: temp,
                repository,
                service,
                log,
                created,
                head,
            }
        }

        fn lookup(&self) -> Result<PullRequestLookup, GitHubError> {
            self.service.pull_request(
                &self.repository,
                b"topic",
                &self.head,
                &GitHubControl::default(),
            )
        }

        fn calls(&self) -> Vec<Vec<String>> {
            let bytes = std::fs::read(&self.log).unwrap_or_default();
            let mut calls = Vec::new();
            let mut current = Vec::new();
            for token in bytes
                .split(|byte| *byte == 0)
                .filter(|token| !token.is_empty())
            {
                let token = String::from_utf8(token.to_vec()).unwrap();
                if token == "END" {
                    calls.push(std::mem::take(&mut current));
                } else {
                    current.push(token);
                }
            }
            calls
        }
    }

    #[cfg(unix)]
    fn fake_gh() -> &'static str {
        r#"#!/bin/sh
set -eu
printf 'ENV=%s\0' "${GH_REPO-unset}" >> "$GH_LOG"
for argument in "$@"; do
    printf '%s\0' "$argument" >> "$GH_LOG"
done
printf 'END\0' >> "$GH_LOG"
if [ "$1 $2" = "repo view" ]; then
    printf '%s\n' '{"nameWithOwner":"muxy/repo","url":"https://github.com/muxy/repo"}'
    exit 0
fi
if [ "$1 $2" = "pr create" ]; then
    : > "$GH_CREATED"
    printf '%s\n' 'https://github.com/muxy/repo/pull/42'
    exit 0
fi
if [ "$1 $2" = "pr merge" ] || [ "$1 $2" = "pr close" ]; then
    if [ "$GH_SCENARIO" = "slow-close" ] && [ "$2" = "close" ]; then
        sleep 10
    fi
    exit 0
fi
if [ "$1 $2" = "pr list" ]; then
    if [ "$GH_SCENARIO" = "list" ]; then
        printf '[%s]\n' "$GH_PR_JSON"
    else
        printf '%s\n' '[]'
    fi
    exit 0
fi
if [ "$1 $2" = "pr view" ]; then
    if [ "$GH_SCENARIO" = "verbose-error" ]; then
        printf '%s' "$GH_ERROR" >&2
        exit 1
    fi
    if [ "$GH_SCENARIO" = "ambiguous" ]; then
        printf '%s\n' 'authentication failed' >&2
        exit 1
    fi
    if [ "$GH_SCENARIO" = "configured" ] && [ "${3-}" = "17" ]; then
        printf '%s\n' "$GH_PR_JSON"
        exit 0
    fi
    if [ "$GH_SCENARIO" = "current" ] && [ "${3-}" = "--repo" ]; then
        printf '%s\n' "$GH_PR_JSON"
        exit 0
    fi
    if [ "$GH_SCENARIO" = "branch" ] && [ "${3-}" = "topic" ]; then
        printf '%s\n' "$GH_PR_JSON"
        exit 0
    fi
    if [ "$GH_SCENARIO" = "action" ] || [ "$GH_SCENARIO" = "update" ] || [ "$GH_SCENARIO" = "update-cross" ] || [ "$GH_SCENARIO" = "slow-close" ] || [ "$GH_SCENARIO" = "stale" ]; then
        printf '%s\n' "$GH_PR_JSON"
        exit 0
    fi
    if [ "$GH_SCENARIO" = "create" ] && [ -f "$GH_CREATED" ]; then
        printf '%s\n' "$GH_PR_JSON"
        exit 0
    fi
    if [ "$GH_SCENARIO" = "create-unreadable" ] && [ -f "$GH_CREATED" ]; then
        printf '%s\n' 'readback failed' >&2
        exit 1
    fi
    printf '%s\n' 'no pull requests found for branch' >&2
    exit 1
fi
printf '%s\n' 'unexpected invocation' >&2
exit 2
"#
    }

    #[cfg(unix)]
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

    #[cfg(unix)]
    fn output(repo: Option<&Path>, args: &[&str]) -> Output {
        command(repo, args).output().unwrap()
    }

    #[cfg(unix)]
    fn git(repo: Option<&Path>, args: &[&str]) {
        let output = output(repo, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn pull_request_parser_requires_identity_and_preserves_unknown_enums() {
        let json = br#"{
            "url":"https://github.com/muxy/repo/pull/42",
            "number":42,
            "state":"FUTURE",
            "isDraft":false,
            "baseRefName":"main",
            "mergeable":"MAYBE",
            "mergeStateStatus":"QUEUED_FOR_MAGIC",
            "statusCheckRollup":null,
            "isCrossRepository":false,
            "headRefOid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "headRefName":"feature"
        }"#;
        let info = parse::parse_pull_request(
            json,
            b"feature",
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap();

        assert_eq!(info.number, 42);
        assert_eq!(info.state, PullRequestState::Unknown("FUTURE".to_owned()));
        assert_eq!(
            info.mergeable,
            PullRequestMergeable::Unknown("MAYBE".to_owned())
        );
        assert_eq!(
            info.merge_state,
            PullRequestMergeState::Unknown("QUEUED_FOR_MAGIC".to_owned())
        );
        assert_eq!(info.checks.status, PullRequestChecksStatus::None);
    }

    #[test]
    fn pull_request_parser_aggregates_all_check_outcomes() {
        let json = br#"{
            "url":"https://github.com/muxy/repo/pull/7",
            "number":7,
            "state":"OPEN",
            "isDraft":false,
            "baseRefName":"main",
            "mergeable":"MERGEABLE",
            "mergeStateStatus":"CLEAN",
            "statusCheckRollup":[
                {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},
                {"__typename":"CheckRun","status":"COMPLETED","conclusion":"FAILURE"},
                {"__typename":"CheckRun","status":"IN_PROGRESS","conclusion":null},
                {"__typename":"StatusContext","state":"NEUTRAL"}
            ],
            "isCrossRepository":false,
            "headRefOid":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "headRefName":"topic"
        }"#;
        let info =
            parse::parse_pull_request(json, b"topic", b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                .unwrap();

        assert_eq!(info.checks.status, PullRequestChecksStatus::Failure);
        assert_eq!(info.checks.passing, 2);
        assert_eq!(info.checks.failing, 1);
        assert_eq!(info.checks.pending, 1);
        assert_eq!(info.checks.total, 4);

        let mut value: serde_json::Value = serde_json::from_slice(json).unwrap();
        value["statusCheckRollup"] = serde_json::json!([
            {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},
            {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SKIPPED"}
        ]);
        let success = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            parse::parse_pull_request(
                &success,
                b"topic",
                b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            )
            .unwrap()
            .checks
            .status,
            PullRequestChecksStatus::Success
        );

        value["statusCheckRollup"] = serde_json::json!([
            {"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},
            {"__typename":"CheckRun","status":"QUEUED","conclusion":null}
        ]);
        let pending = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            parse::parse_pull_request(
                &pending,
                b"topic",
                b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            )
            .unwrap()
            .checks
            .status,
            PullRequestChecksStatus::Pending
        );

        value["state"] = serde_json::Value::Null;
        value["mergeable"] = serde_json::Value::Null;
        value["mergeStateStatus"] = serde_json::Value::Null;
        value["statusCheckRollup"] = serde_json::Value::Null;
        let null_enums = serde_json::to_vec(&value).unwrap();
        let info = parse::parse_pull_request(
            &null_enums,
            b"topic",
            b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )
        .unwrap();
        assert_eq!(info.state, PullRequestState::Unknown("UNKNOWN".to_owned()));
        assert_eq!(
            info.mergeable,
            PullRequestMergeable::Unknown("UNKNOWN".to_owned())
        );
        assert_eq!(
            info.merge_state,
            PullRequestMergeState::Unknown("UNKNOWN".to_owned())
        );
        assert_eq!(info.checks.status, PullRequestChecksStatus::None);
    }

    #[test]
    fn pull_request_parser_rejects_wrong_shapes_urls_and_stale_heads() {
        let valid = br#"{
            "url":"https://github.com/muxy/repo/pull/9",
            "number":9,
            "state":"OPEN",
            "isDraft":false,
            "baseRefName":"main",
            "mergeable":"MERGEABLE",
            "mergeStateStatus":"CLEAN",
            "statusCheckRollup":[],
            "isCrossRepository":false,
            "headRefOid":"cccccccccccccccccccccccccccccccccccccccc",
            "headRefName":"topic"
        }"#;

        assert!(parse::parse_pull_request(b"[]", b"topic", b"cccc").is_err());
        assert!(parse::parse_pull_request(b"{}", b"topic", b"cccc").is_err());
        assert!(parse::parse_pull_request(b"{", b"topic", b"cccc").is_err());
        assert!(
            parse::parse_pull_request(valid, b"other", b"cccccccccccccccccccccccccccccccccccccccc")
                .is_err()
        );
        assert!(
            parse::parse_pull_request(valid, b"topic", b"dddddddddddddddddddddddddddddddddddddddd")
                .is_err()
        );
        let invalid_url = String::from_utf8(valid.to_vec())
            .unwrap()
            .replace("https://github.com", "http://github.com");
        assert!(
            parse::parse_pull_request(
                invalid_url.as_bytes(),
                b"topic",
                b"cccccccccccccccccccccccccccccccccccccccc"
            )
            .is_err()
        );
        assert!(matches!(
            parse::parse_pull_request(
                &vec![b' '; 2 * 1_024 * 1_024 + 1],
                b"topic",
                b"cccccccccccccccccccccccccccccccccccccccc"
            ),
            Err(PullRequestParseError::Oversized)
        ));
        assert!(
            parse::parse_github_repository(
                br#"{"nameWithOwner":"other/repo","url":"https://github.com/muxy/repo"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn pull_request_list_selects_only_an_exact_branch_and_oid() {
        let json = br#"[
            {
                "url":"https://github.com/muxy/repo/pull/1",
                "number":1,
                "state":"OPEN",
                "isDraft":false,
                "baseRefName":"main",
                "mergeable":"MERGEABLE",
                "mergeStateStatus":"CLEAN",
                "statusCheckRollup":[],
                "isCrossRepository":false,
                "headRefOid":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "headRefName":"other"
            },
            {
                "url":"https://github.com/muxy/repo/pull/2",
                "number":2,
                "state":"OPEN",
                "isDraft":false,
                "baseRefName":"main",
                "mergeable":"MERGEABLE",
                "mergeStateStatus":"CLEAN",
                "statusCheckRollup":[],
                "isCrossRepository":false,
                "headRefOid":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "headRefName":"topic"
            }
        ]"#;

        let info = parse::parse_pull_request_list(
            json,
            b"topic",
            b"EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE",
        )
        .unwrap()
        .unwrap();
        assert_eq!(info.number, 2);
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_lookup_bootstraps_identity_sanitizes_and_binds_every_pr_call() {
        let fixture = GitHubFixture::new("list", true);
        let PullRequestLookup::Found(resolved) = fixture.lookup().unwrap() else {
            panic!("pull request");
        };
        assert_eq!(resolved.info.number, 42);
        let calls = fixture.calls();
        assert_eq!(
            &calls[0][1..],
            ["repo", "view", "--json", "nameWithOwner,url"]
        );
        assert!(calls.iter().all(|call| call[0] == "ENV=unset"));
        for call in calls
            .iter()
            .filter(|call| call.get(1).map(String::as_str) == Some("pr"))
        {
            let repo = call
                .iter()
                .position(|argument| argument == "--repo")
                .unwrap();
            assert_eq!(call[repo + 1], "github.com/muxy/repo");
        }
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.get(1).map(String::as_str) == Some("pr"))
                .map(|call| call[2].as_str())
                .collect::<Vec<_>>(),
            ["view", "list"]
        );
        assert!(calls.iter().all(|call| {
            call.get(1).map(String::as_str) != Some("pr")
                || call.get(2).map(String::as_str) != Some("view")
                || call.get(3).map(String::as_str) == Some("topic")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_lookup_honors_configured_and_branch_order() {
        let configured = GitHubFixture::new("configured", true);
        git(
            Some(&configured.repository),
            &["config", "branch.topic.muxy-pr-number", "17"],
        );
        assert!(matches!(
            configured.lookup(),
            Ok(PullRequestLookup::Found(_))
        ));
        let configured_calls = configured.calls();
        assert!(configured_calls.iter().any(|call| {
            call.get(1).map(String::as_str) == Some("pr")
                && call.get(2).map(String::as_str) == Some("view")
                && call.get(3).map(String::as_str) == Some("17")
        }));

        let branch = GitHubFixture::new("branch", true);
        assert!(matches!(branch.lookup(), Ok(PullRequestLookup::Found(_))));
        let branch_calls = branch.calls();
        assert_eq!(
            branch_calls
                .iter()
                .filter(|call| call.get(1).map(String::as_str) == Some("pr"))
                .count(),
            1
        );
        assert!(branch_calls.iter().any(|call| {
            call.get(1).map(String::as_str) == Some("pr")
                && call.get(2).map(String::as_str) == Some("view")
                && call.get(3).map(String::as_str) == Some("topic")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_distinguishes_missing_no_pr_and_ambiguous_failures() {
        let missing = GitHubFixture::new("none", false);
        assert!(matches!(
            missing.lookup(),
            Err(GitHubError::MissingExecutable)
        ));

        let none = GitHubFixture::new("none", true);
        assert!(matches!(
            none.lookup(),
            Ok(PullRequestLookup::NoPullRequest(_))
        ));

        let ambiguous = GitHubFixture::new("ambiguous", true);
        assert!(matches!(
            ambiguous.lookup(),
            Err(GitHubError::Command { .. })
        ));

        let stale = GitHubFixture::new("stale", true);
        assert!(matches!(stale.lookup(), Err(GitHubError::StalePullRequest)));
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_create_reads_back_and_preserves_a_partial_url() {
        let fixture = GitHubFixture::new("create", true);
        let PullRequestLookup::NoPullRequest(context) = fixture.lookup().unwrap() else {
            panic!("no pull request");
        };
        let request = CreatePullRequestRequest {
            branch: "topic".to_owned(),
            base: "main".to_owned(),
            title: "A title".to_owned(),
            body: "A body".to_owned(),
            draft: false,
        };
        let outcome = fixture
            .service
            .create_pull_request(
                &fixture.repository,
                &context,
                &request,
                &GitHubControl::default(),
            )
            .unwrap();
        assert!(matches!(outcome, CreatePullRequestOutcome::Created(_)));
        assert!(fixture.created.exists());
        let create = fixture
            .calls()
            .into_iter()
            .find(|call| call.get(2).map(String::as_str) == Some("create"))
            .unwrap();
        assert!(!create.iter().any(|argument| argument == "--draft"));
        assert!(
            create
                .windows(2)
                .any(|pair| pair == ["--repo", "github.com/muxy/repo"])
        );

        let partial = GitHubFixture::new("create-unreadable", true);
        let PullRequestLookup::NoPullRequest(context) = partial.lookup().unwrap() else {
            panic!("no pull request");
        };
        let outcome = partial
            .service
            .create_pull_request(
                &partial.repository,
                &context,
                &request,
                &GitHubControl::default(),
            )
            .unwrap();
        let CreatePullRequestOutcome::CreatedUnreadable { url, message } = outcome else {
            panic!("partial create");
        };
        assert_eq!(
            url.unwrap().as_str(),
            "https://github.com/muxy/repo/pull/42"
        );
        assert!(message.contains("created"));
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_actions_use_exact_flags_and_refuse_changed_remote_context() {
        for (method, flag) in [
            (PullRequestMergeMethod::Squash, "--squash"),
            (PullRequestMergeMethod::Merge, "--merge"),
            (PullRequestMergeMethod::Rebase, "--rebase"),
        ] {
            let fixture = GitHubFixture::new("action", true);
            let PullRequestLookup::Found(resolved) = fixture.lookup().unwrap() else {
                panic!("pull request");
            };
            assert_eq!(
                fixture
                    .service
                    .merge_pull_request(
                        &fixture.repository,
                        &resolved,
                        method,
                        &GitHubControl::default(),
                    )
                    .unwrap(),
                PullRequestMergeOutcome::Success
            );
            let merge = fixture
                .calls()
                .into_iter()
                .find(|call| call.get(2).map(String::as_str) == Some("merge"))
                .unwrap();
            assert!(merge.iter().any(|argument| argument == flag));
            assert!(!merge.iter().any(|argument| argument == "--delete-branch"));
        }

        let fixture = GitHubFixture::new("action", true);
        let PullRequestLookup::Found(resolved) = fixture.lookup().unwrap() else {
            panic!("pull request");
        };
        let replacement = fixture._temp.path().join("replacement.git");
        git(
            None,
            &["init", "-q", "--bare", replacement.to_str().unwrap()],
        );
        git(
            Some(&fixture.repository),
            &["remote", "set-url", "origin", replacement.to_str().unwrap()],
        );
        assert!(matches!(
            fixture.service.close_pull_request(
                &fixture.repository,
                &resolved,
                &GitHubControl::default()
            ),
            Err(GitHubError::RemoteContextChanged)
        ));
        assert!(
            !fixture
                .calls()
                .iter()
                .any(|call| call.get(2).map(String::as_str) == Some("close"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_github_close_update_cancellation_and_errors_are_bounded() {
        let close = GitHubFixture::new("action", true);
        let PullRequestLookup::Found(mut resolved) = close.lookup().unwrap() else {
            panic!("pull request");
        };
        resolved.info.number = 99;
        close
            .service
            .close_pull_request(&close.repository, &resolved, &GitHubControl::default())
            .unwrap();
        let close_call = close
            .calls()
            .into_iter()
            .find(|call| call.get(2).map(String::as_str) == Some("close"))
            .unwrap();
        assert_eq!(close_call[3], "42");

        let update = GitHubFixture::new("update", true);
        let PullRequestLookup::Found(resolved) = update.lookup().unwrap() else {
            panic!("pull request");
        };
        assert_eq!(
            update
                .service
                .update_pull_request(&update.repository, &resolved, &GitHubControl::default(),)
                .unwrap(),
            MutationOutcome::Success
        );

        let not_behind = GitHubFixture::new("action", true);
        let PullRequestLookup::Found(resolved) = not_behind.lookup().unwrap() else {
            panic!("pull request");
        };
        assert!(matches!(
            not_behind.service.update_pull_request(
                &not_behind.repository,
                &resolved,
                &GitHubControl::default()
            ),
            Err(GitHubError::PullRequestNotMergeable)
        ));

        let dirty = GitHubFixture::new("update", true);
        let PullRequestLookup::Found(resolved) = dirty.lookup().unwrap() else {
            panic!("pull request");
        };
        std::fs::write(dirty.repository.join("tracked"), "dirty\n").unwrap();
        assert!(matches!(
            dirty.service.update_pull_request(
                &dirty.repository,
                &resolved,
                &GitHubControl::default()
            ),
            Err(GitHubError::DirtyRepository)
        ));

        let cross = GitHubFixture::new("update-cross", true);
        let PullRequestLookup::Found(resolved) = cross.lookup().unwrap() else {
            panic!("pull request");
        };
        assert!(matches!(
            cross.service.update_pull_request(
                &cross.repository,
                &resolved,
                &GitHubControl::default()
            ),
            Err(GitHubError::CrossRepositoryUpdate)
        ));

        let cancelled = GitHubFixture::new("slow-close", true);
        let PullRequestLookup::Found(resolved) = cancelled.lookup().unwrap() else {
            panic!("pull request");
        };
        let cancellation = CancellationSignal::new();
        let thread_cancellation = cancellation.clone();
        let service = cancelled.service.clone();
        let repository = cancelled.repository.clone();
        let handle = std::thread::spawn(move || {
            service.close_pull_request(
                &repository,
                &resolved,
                &GitHubControl::with_cancellation(thread_cancellation),
            )
        });
        for _ in 0..100 {
            if cancelled
                .calls()
                .iter()
                .any(|call| call.get(2).map(String::as_str) == Some("close"))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        cancellation.cancel();
        let error = handle.join().unwrap().unwrap_err();
        assert_eq!(
            error.mutation_effect(),
            Some(GitHubMutationEffect::Uncertain)
        );

        let verbose = GitHubFixture::new("verbose-error", true);
        let error = verbose.lookup().unwrap_err();
        let GitHubError::Command { message, .. } = error else {
            panic!("command error");
        };
        assert!(message.len() <= 1_000);
    }

    #[test]
    fn pull_request_public_contract_exposes_lookup_and_native_actions() {
        let _ = GitHubControl::default();
        let _lookup: Option<PullRequestLookup> = None;
        let _request = CreatePullRequestRequest {
            branch: "topic".to_owned(),
            base: "main".to_owned(),
            title: "Title".to_owned(),
            body: "Body".to_owned(),
            draft: false,
        };
        let _method = PullRequestMergeMethod::Squash;
        let _outcome: Option<CreatePullRequestOutcome> = None;
        let _merge: Option<PullRequestMergeOutcome> = None;
        for message in [
            "no pull requests found",
            "no pull request found",
            "could not resolve to a PullRequest",
            "no commits between main and topic",
        ] {
            assert!(no_pull_request_error(message.as_bytes()));
        }
        assert!(!no_pull_request_error(b"authentication failed"));
    }
}
