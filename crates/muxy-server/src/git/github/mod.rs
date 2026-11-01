//! GitHub lookups and pull-request actions, through the GitHub CLI.
//!
//! Lookups that need the network are remembered briefly, failures included,
//! so refreshes don't ask GitHub every time.

mod cli;
mod parse;
mod pull_requests;

use super::cache::Cache;
use super::{Result, error, read, run, text, validate_branch};
use muxy_protocol::{GitPullRequest, GitPullRequestAction, GitReply, GitSummary};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Repository identity and default branch rarely change.
const SETTLED: Duration = Duration::from_secs(300);
const PULL_REQUEST: Duration = Duration::from_secs(30);

/// A repository and the remote URLs its answers depend on.
type RemoteKey = (PathBuf, Vec<u8>);
/// A repository and the branch and commit its pull request belongs to.
type BranchKey = (PathBuf, String, String);

#[derive(Debug)]
pub(super) struct Github {
    cli: cli::Cli,
    identities: Cache<RemoteKey, String>,
    default_branches: Cache<RemoteKey, String>,
    pull_requests: Cache<BranchKey, Option<GitPullRequest>>,
}

impl Github {
    /// Runs `executable` in place of the installed GitHub CLI.
    #[cfg(test)]
    pub(super) fn with_cli(executable: PathBuf) -> Self {
        Self {
            cli: cli::Cli {
                executable: Some(executable),
            },
            ..Self::default()
        }
    }
}

impl Default for Github {
    fn default() -> Self {
        Self {
            cli: cli::Cli::default(),
            identities: Cache::new(SETTLED),
            default_branches: Cache::new(SETTLED),
            pull_requests: Cache::new(PULL_REQUEST),
        }
    }
}

/// What remote lookups need from the repository. Read under the repository
/// lock; the lookups themselves run without it.
pub(super) struct Remote {
    urls: Vec<u8>,
    current: Option<CurrentBranch>,
}

#[derive(Clone)]
struct CurrentBranch {
    branch: String,
    head: String,
    /// The pull request a checkout from Muxy recorded for the branch.
    configured_number: Option<u64>,
}

impl Remote {
    pub(super) fn read(repository: &Path, summary: &GitSummary) -> Self {
        let urls = run(
            repository,
            &["config", "--get-regexp", r"^remote\..*\.url$"],
        )
        .unwrap_or_default();
        let current = summary
            .branch
            .clone()
            .zip(summary.head.clone())
            .map(|(branch, head)| CurrentBranch {
                configured_number: configured_number(repository, &branch),
                branch,
                head,
            });
        Self { urls, current }
    }
}

fn configured_number(repository: &Path, branch: &str) -> Option<u64> {
    run(
        repository,
        &[
            "config",
            "--get",
            &format!("branch.{branch}.muxy-pr-number"),
        ],
    )
    .ok()
    .and_then(|bytes| text(&bytes).ok())
    .and_then(|value| value.parse::<u64>().ok())
    .filter(|number| *number > 0)
}

impl Github {
    fn identity(&self, repository: &Path, remote: &Remote, fresh: bool) -> Result<String> {
        self.identities
            .get((repository.to_owned(), remote.urls.clone()), fresh, || {
                parse::identity(
                    &self
                        .cli
                        .run(repository, &["repo", "view", "--json", "url"])?,
                )
            })
    }

    pub(super) fn default_branch(
        &self,
        repository: &Path,
        remote: &Remote,
        fresh: bool,
    ) -> Option<String> {
        self.default_branches
            .get((repository.to_owned(), remote.urls.clone()), fresh, || {
                super::details::default_branch(repository)
                    .or_else(|| self.default_branch_from_github(repository))
                    .ok_or_else(|| error("No default branch"))
            })
            .ok()
    }

    fn default_branch_from_github(&self, repository: &Path) -> Option<String> {
        let bytes = self
            .cli
            .run(repository, &["repo", "view", "--json", "defaultBranchRef"])
            .ok()?;
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        let branch = value.pointer("/defaultBranchRef/name")?.as_str()?;
        validate_branch(repository, branch).ok()?;
        Some(branch.to_owned())
    }

    /// The current branch's open pull request.
    pub(super) fn pull_request(
        &self,
        repository: &Path,
        remote: &Remote,
        fresh: bool,
    ) -> Result<Option<GitPullRequest>> {
        let identity = self.identity(repository, remote, fresh)?;
        self.cached_current(repository, remote, &identity, fresh)
    }

    fn cached_current(
        &self,
        repository: &Path,
        remote: &Remote,
        identity: &str,
        fresh: bool,
    ) -> Result<Option<GitPullRequest>> {
        let Some(current) = &remote.current else {
            return Ok(None);
        };
        let key = (
            repository.to_owned(),
            current.branch.clone(),
            current.head.clone(),
        );
        self.pull_requests
            .get(key, fresh, || self.current(repository, current, identity))
    }

    /// Forgets the repository's pull requests after Muxy changed it.
    pub(super) fn forget_pull_requests(&self, repository: &Path) {
        self.pull_requests.forget(|(path, ..)| path == repository);
    }

    pub(super) fn apply(
        &self,
        repository: &Path,
        remote: &Remote,
        action: &GitPullRequestAction,
    ) -> Result<GitReply> {
        let identity = self.identity(repository, remote, false)?;
        match action {
            // Asked for directly, so always current.
            GitPullRequestAction::Info => self
                .cached_current(repository, remote, &identity, true)
                .map(|pr| GitReply::PullRequest(pr.map(Box::new))),
            GitPullRequestAction::Number => self.number(repository, remote, &identity),
            GitPullRequestAction::List {
                filter,
                limit,
                checks,
            } => self.list(repository, &identity, *filter, *limit, *checks),
            GitPullRequestAction::Diff { number, line_limit } => {
                self.diff(repository, &identity, *number, *line_limit)
            }
            GitPullRequestAction::Create {
                title,
                body,
                base_branch,
                draft,
            } => {
                let base = base_branch
                    .clone()
                    .or_else(|| self.default_branch(repository, remote, false))
                    .unwrap_or_else(|| "main".into());
                self.create(repository, &identity, title, body, &base, *draft)
            }
            GitPullRequestAction::Merge {
                number,
                method,
                delete_branch,
                expected_head,
            } => self.merge(
                repository,
                &identity,
                *number,
                *method,
                *delete_branch,
                expected_head.as_deref(),
            ),
            GitPullRequestAction::Close { number } => {
                self.cli.run(
                    repository,
                    &["pr", "close", &number.to_string(), "--repo", &identity],
                )?;
                Ok(GitReply::Done)
            }
            GitPullRequestAction::Checkout { number } => {
                let branch = self.prepare_checkout(repository, &identity, *number)?;
                run(repository, &["switch", "--", &branch])?;
                Ok(GitReply::Done)
            }
            GitPullRequestAction::UpdateBranch {
                number,
                expected_head,
            } => self.update_branch(repository, remote, &identity, *number, expected_head),
        }
    }

    pub(super) fn prepare_worktree(&self, repository: &Path, number: u64) -> Result<String> {
        let remote = Remote::read(repository, &read::summary(repository)?);
        let identity = self.identity(repository, &remote, false)?;
        self.prepare_checkout(repository, &identity, number)
    }
}
