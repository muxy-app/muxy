use super::parse::{self, RawPullRequest, fields, is_open, no_pr, valid_segment};
use super::{CurrentBranch, Github, Remote};
use crate::git::{Result, command, diff, error, read, run, snapshot, text, validate_branch};
use muxy_protocol::{GitMergeMethod, GitPullRequest, GitPullRequestFilter, GitReply};
use serde_json::Value;
use std::path::Path;

impl Github {
    pub(super) fn diff(
        &self,
        repository: &Path,
        identity: &str,
        number: u64,
        line_limit: Option<u32>,
    ) -> Result<GitReply> {
        let (bytes, truncated) = self.cli.run_bounded(
            repository,
            &[
                "pr",
                "diff",
                &number.to_string(),
                "--repo",
                identity,
                "--color",
                "never",
            ],
            1024 * 1024,
        )?;
        Ok(GitReply::RawDiff(diff::bounded(
            &bytes, truncated, line_limit,
        )))
    }

    pub(super) fn merge(
        &self,
        repository: &Path,
        identity: &str,
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        expected_head: Option<&str>,
    ) -> Result<GitReply> {
        let flag = match method {
            GitMergeMethod::Merge => "--merge",
            GitMergeMethod::Squash => "--squash",
            GitMergeMethod::Rebase => "--rebase",
        };
        let number = number.to_string();
        let mut args = vec!["pr", "merge", &number, "--repo", identity, flag];
        if delete_branch {
            args.push("--delete-branch");
        }
        if let Some(head) = expected_head {
            args.extend(["--match-head-commit", head]);
        }
        self.cli.run(repository, &args)?;
        Ok(GitReply::Done)
    }

    pub(super) fn update_branch(
        &self,
        repository: &Path,
        remote: &Remote,
        identity: &str,
        number: u64,
        expected_head: &str,
    ) -> Result<GitReply> {
        let pr = remote
            .current
            .as_ref()
            .map(|current| self.current(repository, current, identity))
            .transpose()?
            .flatten()
            .filter(|pr| pr.number == number)
            .ok_or_else(|| {
                error("This branch no longer has that pull request; refresh and try again")
            })?;
        if pr.state != "OPEN" {
            return Err(error("The pull request is no longer open"));
        }
        if pr.head_oid != expected_head {
            return Err(error(
                "The pull request changed on GitHub; refresh and try again",
            ));
        }
        if pr.cross_repository {
            return Err(error("Pull requests from forks can't be updated here"));
        }
        validate_branch(repository, &pr.base_branch)?;
        let summary = read::summary(repository)?;
        let branch = summary
            .branch
            .clone()
            .filter(|branch| {
                *branch == pr.head_branch
                    || super::configured_number(repository, branch) == Some(number)
            })
            .ok_or_else(|| {
                error(format!(
                    "Switch to {} before updating the pull request",
                    pr.head_branch
                ))
            })?;
        if summary.head.as_deref() != Some(expected_head) {
            return Err(error(
                "This branch isn't at the pull request's head commit; pull or push it first",
            ));
        }
        if summary.changed > summary.untracked {
            return Err(error(
                "Commit or stash your changes before updating the branch",
            ));
        }
        let destination = snapshot::destination(repository, &branch)?
            .ok_or_else(|| error("No remote to push the updated branch to"))?;
        command::network(repository, &["fetch", "origin", &pr.base_branch])?;
        if let Err(cause) = run(
            repository,
            &[
                "merge",
                "--no-edit",
                &format!("refs/remotes/origin/{}", pr.base_branch),
            ],
        ) {
            let _ = run(repository, &["merge", "--abort"]);
            return Err(error(format!(
                "Couldn't merge origin/{}, so the merge was aborted: {}",
                pr.base_branch,
                cause.message()
            )));
        }
        snapshot::publish(repository, &branch, &destination).map_err(|cause| {
            error(format!(
                "Merged origin/{} into {branch} locally, but couldn't push it: {}. Push the branch to finish.",
                pr.base_branch,
                cause.message()
            ))
        })?;
        Ok(GitReply::Done)
    }

    pub(super) fn create(
        &self,
        repository: &Path,
        identity: &str,
        title: &str,
        body: &str,
        base: &str,
        draft: bool,
    ) -> Result<GitReply> {
        let branch = read::summary(repository)?
            .branch
            .ok_or_else(|| error("Cannot create a PR from detached HEAD"))?;
        validate_branch(repository, base)?;
        let destination = snapshot::destination(repository, &branch)?
            .ok_or_else(|| error("No remote to push the pull request branch to"))?;
        snapshot::publish(repository, &branch, &destination)?;
        let mut args = vec![
            "pr",
            "create",
            "--repo",
            identity,
            "--head",
            &destination.branch,
            "--base",
            base,
            "--title",
            title.trim(),
            "--body",
            body,
        ];
        if draft {
            args.push("--draft");
        }
        let created = text(&self.cli.run(repository, &args)?)?;
        let url = created
            .split_whitespace()
            .find(|value| value.starts_with("https://"))
            .ok_or_else(|| {
                error("Pull request created, but its URL was not returned; refresh before retrying")
            })?;
        let number = url
            .rsplit('/')
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|number| *number > 0)
            .ok_or_else(|| {
                error(format!(
                    "Pull request created at {url}, but its number could not be read"
                ))
            })?;
        self.view(repository, identity, &number.to_string())
            .map(|pr| GitReply::PullRequest(Some(Box::new(pr))))
            .map_err(|cause| {
                error(format!(
                    "Pull request created at {url}, but readback failed: {}",
                    cause.message()
                ))
            })
    }

    pub(super) fn number(
        &self,
        repository: &Path,
        remote: &Remote,
        identity: &str,
    ) -> Result<GitReply> {
        let Some(current) = &remote.current else {
            return Ok(GitReply::PullRequestNumber(None));
        };
        let number = self
            .lookup_current(
                repository,
                current,
                identity,
                "number,headRefName,headRefOid,state,isCrossRepository",
            )?
            .map(|value| {
                value
                    .get("number")
                    .and_then(Value::as_u64)
                    .filter(|number| *number > 0)
                    .ok_or_else(|| error("Invalid pull request number"))
            })
            .transpose()?;
        Ok(GitReply::PullRequestNumber(number))
    }

    pub(super) fn list(
        &self,
        repository: &Path,
        identity: &str,
        filter: GitPullRequestFilter,
        limit: u32,
        checks: bool,
    ) -> Result<GitReply> {
        let filter = match filter {
            GitPullRequestFilter::Open => "open",
            GitPullRequestFilter::Closed => "closed",
            GitPullRequestFilter::Merged => "merged",
            GitPullRequestFilter::All => "all",
        };
        let fields = fields(checks);
        let bytes = self.cli.run(
            repository,
            &[
                "pr",
                "list",
                "--repo",
                identity,
                "--state",
                filter,
                "--limit",
                &limit.to_string(),
                "--json",
                &fields,
            ],
        )?;
        let raw: Vec<RawPullRequest> = serde_json::from_slice(&bytes).map_err(error)?;
        if raw.len() > limit as usize {
            return Err(error("Too many pull requests"));
        }
        Ok(GitReply::PullRequests(
            raw.into_iter()
                .map(parse::pull_request)
                .collect::<Result<_>>()?,
        ))
    }

    fn view(&self, repository: &Path, identity: &str, selector: &str) -> Result<GitPullRequest> {
        let bytes = self.cli.run(
            repository,
            &[
                "pr",
                "view",
                selector,
                "--repo",
                identity,
                "--json",
                &fields(true),
            ],
        )?;
        parse::pull_request(serde_json::from_slice(&bytes).map_err(error)?)
    }

    pub(super) fn current(
        &self,
        repository: &Path,
        current: &CurrentBranch,
        identity: &str,
    ) -> Result<Option<GitPullRequest>> {
        self.lookup_current(repository, current, identity, &fields(true))?
            .map(|value| parse::pull_request(serde_json::from_value(value).map_err(error)?))
            .transpose()
    }

    fn lookup_current(
        &self,
        repository: &Path,
        current: &CurrentBranch,
        identity: &str,
        fields: &str,
    ) -> Result<Option<Value>> {
        let mut first_error = None;
        if let Some(number) = current.configured_number {
            match self.cli.run(
                repository,
                &[
                    "pr",
                    "view",
                    &number.to_string(),
                    "--repo",
                    identity,
                    "--json",
                    fields,
                ],
            ) {
                Ok(bytes) => {
                    let value: Value = serde_json::from_slice(&bytes).map_err(error)?;
                    if is_open(&value) {
                        return Ok(Some(value));
                    }
                }
                Err(cause) if no_pr(&cause) => (),
                Err(cause) => first_error = Some(cause),
            }
        }
        match self.cli.run(repository, &["pr", "view", "--json", fields]) {
            Ok(bytes) => {
                let value: Value = serde_json::from_slice(&bytes).map_err(error)?;
                if current.matches_view(&value) {
                    return Ok(Some(value));
                }
            }
            Err(cause) if no_pr(&cause) => (),
            Err(cause) => {
                first_error.get_or_insert(cause);
            }
        }
        if current.branch.parse::<u64>().is_err() {
            match self.cli.run(
                repository,
                &[
                    "pr",
                    "view",
                    &current.branch,
                    "--repo",
                    identity,
                    "--json",
                    fields,
                ],
            ) {
                Ok(bytes) => {
                    let value: Value = serde_json::from_slice(&bytes).map_err(error)?;
                    if current.matches_view(&value) {
                        return Ok(Some(value));
                    }
                }
                Err(cause) if no_pr(&cause) => (),
                Err(cause) => {
                    first_error.get_or_insert(cause);
                }
            }
        }
        let bytes = match self.cli.run(
            repository,
            &[
                "pr",
                "list",
                "--repo",
                identity,
                "--head",
                &current.branch,
                "--state",
                "open",
                "--limit",
                "100",
                "--json",
                fields,
            ],
        ) {
            Ok(bytes) => bytes,
            Err(cause) if no_pr(&cause) => return first_error.map_or(Ok(None), Err),
            Err(cause) => return Err(cause),
        };
        let values: Vec<Value> = serde_json::from_slice(&bytes).map_err(error)?;
        let matched = values
            .into_iter()
            .find(|value| is_open(value) && current.matches_head(value));
        match (matched, first_error) {
            (Some(value), _) => Ok(Some(value)),
            (None, Some(cause)) => Err(cause),
            (None, None) => Ok(None),
        }
    }

    pub(super) fn prepare_checkout(
        &self,
        repository: &Path,
        identity: &str,
        number: u64,
    ) -> Result<String> {
        let bytes = self.cli.run(
            repository,
            &[
                "pr",
                "view",
                &number.to_string(),
                "--repo",
                identity,
                "--json",
                "headRefName,headRepository,headRepositoryOwner",
            ],
        )?;
        let value: Value = serde_json::from_slice(&bytes).map_err(error)?;
        let head = value
            .get("headRefName")
            .and_then(Value::as_str)
            .ok_or_else(|| error("Missing PR branch"))?;
        let name = value
            .pointer("/headRepository/name")
            .and_then(Value::as_str)
            .ok_or_else(|| error("PR head repository was deleted"))?;
        let owner = value
            .pointer("/headRepositoryOwner/login")
            .and_then(Value::as_str)
            .ok_or_else(|| error("Missing PR owner"))?;
        validate_branch(repository, head)?;
        if !valid_segment(name) || !valid_segment(owner) {
            return Err(error("Invalid PR repository"));
        }
        let host = identity
            .strip_prefix("https://")
            .and_then(|value| value.split('/').next())
            .ok_or_else(|| error("Invalid GitHub host"))?;
        let remote = format!("pr-{number}-{owner}-{name}");
        let url = format!("https://{host}/{owner}/{name}.git");
        match run(
            repository,
            &["config", "--get", &format!("remote.{remote}.url")],
        ) {
            Ok(existing) if text(&existing)? != url => {
                return Err(error("PR remote points to a different repository"));
            }
            Ok(_) => (),
            Err(_) => {
                run(repository, &["remote", "add", &remote, &url])?;
            }
        }
        command::network(
            repository,
            &[
                "fetch",
                &remote,
                &format!("refs/heads/{head}:refs/remotes/{remote}/{head}"),
            ],
        )?;
        let branch = format!("pr/{number}/{}", safe_component(head));
        let upstream = format!("{remote}/{head}");
        if run(
            repository,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ],
        )
        .is_ok()
        {
            run(
                repository,
                &["branch", &format!("--set-upstream-to={upstream}"), &branch],
            )?;
        } else {
            run(
                repository,
                &[
                    "branch",
                    "--track",
                    &branch,
                    &format!("refs/remotes/{upstream}"),
                ],
            )?;
        }
        run(
            repository,
            &[
                "config",
                &format!("branch.{branch}.muxy-pr-number"),
                &number.to_string(),
            ],
        )?;
        Ok(branch)
    }
}

impl CurrentBranch {
    fn matches_head(&self, value: &Value) -> bool {
        value.get("headRefName").and_then(Value::as_str) == Some(&self.branch)
            && value
                .get("headRefOid")
                .and_then(Value::as_str)
                .is_some_and(|head| head.eq_ignore_ascii_case(&self.head))
    }

    fn matches_view(&self, value: &Value) -> bool {
        is_open(value)
            && value.get("headRefName").and_then(Value::as_str) == Some(&self.branch)
            && (value.get("isCrossRepository").and_then(Value::as_bool) == Some(false)
                || self.matches_head(value))
    }
}

fn safe_component(value: &str) -> String {
    let component = value
        .split('/')
        .map(|part| {
            part.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .split('-')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if component.is_empty() {
        "head".into()
    } else {
        component
    }
}
