use super::{Result, command, diff, error, read, run, snapshot, text, validate_branch};
use muxy_protocol::{
    GitChecks, GitMergeMethod, GitPullRequest, GitPullRequestAction, GitPullRequestFilter, GitReply,
};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const FIELDS: &str = "number,url,title,author,headRefName,headRefOid,baseRefName,state,isDraft,updatedAt,mergeable,mergeStateStatus,isCrossRepository";

#[derive(Debug, Default)]
pub(super) struct Github {
    /// Runs this GitHub CLI instead of the installed one.
    pub(super) executable: Option<PathBuf>,
    default_branches: super::metadata::DefaultBranches,
}

impl Github {
    fn command(&self, repository: &Path, args: &[&str]) -> Command {
        let executable = self
            .executable
            .clone()
            .or_else(command::github_cli)
            .unwrap_or_else(|| "gh".into());
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(repository)
            .env_remove("GH_REPO")
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1");
        for (name, _) in std::env::vars_os() {
            let key = name.to_string_lossy();
            if key.starts_with("GIT_")
                && !matches!(
                    key.as_ref(),
                    "GIT_SSH" | "GIT_SSH_COMMAND" | "GIT_SSH_VARIANT" | "GIT_ASKPASS"
                )
            {
                command.env_remove(name);
            }
        }
        command.env("GIT_TERMINAL_PROMPT", "0");
        command
    }

    fn run(&self, repository: &Path, args: &[&str]) -> Result<Vec<u8>> {
        command::capture_with(
            self.command(repository, args),
            Duration::from_secs(60),
            None,
            false,
        )
        .map(|output| output.0)
        .map_err(|cause| error(format!("GitHub CLI: {}", cause.message())))
    }

    fn identity(&self, repository: &Path) -> Result<String> {
        let bytes = self.run(repository, &["repo", "view", "--json", "url"])?;
        let value: Value = serde_json::from_slice(&bytes).map_err(error)?;
        let url = value
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| error("Missing GitHub repository URL"))?;
        let parts: Vec<_> = url
            .strip_prefix("https://")
            .unwrap_or_default()
            .split('/')
            .collect();
        if parts.len() != 3 || parts.iter().any(|part| !valid_segment(part)) {
            return Err(error("Invalid GitHub repository URL"));
        }
        Ok(url.to_owned())
    }

    pub(super) fn apply(
        &self,
        repository: &Path,
        action: &GitPullRequestAction,
    ) -> Result<GitReply> {
        let identity = self.identity(repository)?;
        match action {
            GitPullRequestAction::Info => self
                .current(repository, &identity)
                .map(|pr| GitReply::PullRequest(pr.map(Box::new))),
            GitPullRequestAction::Number => self.number(repository, &identity),
            GitPullRequestAction::List {
                filter,
                limit,
                checks,
            } => self.list(repository, &identity, *filter, *limit, *checks),
            GitPullRequestAction::Diff { number, line_limit } => {
                let (bytes, truncated) = command::capture_with(
                    self.command(
                        repository,
                        &[
                            "pr",
                            "diff",
                            &number.to_string(),
                            "--repo",
                            &identity,
                            "--color",
                            "never",
                        ],
                    ),
                    Duration::from_secs(60),
                    Some(1024 * 1024),
                    false,
                )?;
                Ok(GitReply::RawDiff(diff::bounded(
                    &bytes,
                    truncated,
                    *line_limit,
                )))
            }
            GitPullRequestAction::Create {
                title,
                body,
                base_branch,
                draft,
            } => self.create(
                repository,
                &identity,
                title,
                body,
                base_branch.as_deref(),
                *draft,
            ),
            GitPullRequestAction::Merge {
                number,
                method,
                delete_branch,
                expected_head,
            } => {
                let flag = match method {
                    GitMergeMethod::Merge => "--merge",
                    GitMergeMethod::Squash => "--squash",
                    GitMergeMethod::Rebase => "--rebase",
                };
                let number = number.to_string();
                let mut args = vec!["pr", "merge", &number, "--repo", &identity, flag];
                if *delete_branch {
                    args.push("--delete-branch");
                }
                if let Some(head) = expected_head {
                    args.extend(["--match-head-commit", head]);
                }
                self.run(repository, &args)?;
                Ok(GitReply::Done)
            }
            GitPullRequestAction::Close { number } => {
                self.run(
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
            } => self.update_branch(repository, &identity, *number, expected_head),
        }
    }

    fn update_branch(
        &self,
        repository: &Path,
        identity: &str,
        number: u64,
        expected_head: &str,
    ) -> Result<GitReply> {
        let pr = self
            .current(repository, identity)?
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
                    || run(
                        repository,
                        &[
                            "config",
                            "--get",
                            &format!("branch.{branch}.muxy-pr-number"),
                        ],
                    )
                    .ok()
                    .and_then(|bytes| text(&bytes).ok())
                    .is_some_and(|value| value == number.to_string())
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

    fn create(
        &self,
        repository: &Path,
        identity: &str,
        title: &str,
        body: &str,
        base_branch: Option<&str>,
        draft: bool,
    ) -> Result<GitReply> {
        let branch = read::summary(repository)?
            .branch
            .ok_or_else(|| error("Cannot create a PR from detached HEAD"))?;
        let base = if let Some(base) = base_branch {
            base.to_owned()
        } else {
            self.default_branch(repository)
                .unwrap_or_else(|| "main".into())
        };
        validate_branch(repository, &base)?;
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
            &base,
            "--title",
            title.trim(),
            "--body",
            body,
        ];
        if draft {
            args.push("--draft");
        }
        let created = text(&self.run(repository, &args)?)?;
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

    fn number(&self, repository: &Path, identity: &str) -> Result<GitReply> {
        let number = self
            .lookup_current(
                repository,
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

    fn list(
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
        let bytes = self.run(
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
            raw.into_iter().map(parse).collect::<Result<_>>()?,
        ))
    }

    pub(super) fn prepare_worktree(&self, repository: &Path, number: u64) -> Result<String> {
        let identity = self.identity(repository)?;
        self.prepare_checkout(repository, &identity, number)
    }

    fn view(&self, repository: &Path, identity: &str, selector: &str) -> Result<GitPullRequest> {
        let bytes = self.run(
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
        parse(serde_json::from_slice(&bytes).map_err(error)?)
    }

    fn current(&self, repository: &Path, identity: &str) -> Result<Option<GitPullRequest>> {
        self.lookup_current(repository, identity, &fields(true))?
            .map(|value| parse(serde_json::from_value(value).map_err(error)?))
            .transpose()
    }

    fn lookup_current(
        &self,
        repository: &Path,
        identity: &str,
        fields: &str,
    ) -> Result<Option<Value>> {
        let Some(current) = current_branch(repository)? else {
            return Ok(None);
        };
        let mut first_error = None;
        if let Some(number) = current.configured_number {
            match self.run(
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
                Ok(bytes) => return serde_json::from_slice(&bytes).map(Some).map_err(error),
                Err(cause) if no_pr(&cause) => (),
                Err(cause) => first_error = Some(cause),
            }
        }
        match self.run(repository, &["pr", "view", "--json", fields]) {
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
            match self.run(
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
        let bytes = match self.run(
            repository,
            &[
                "pr",
                "list",
                "--repo",
                identity,
                "--head",
                &current.branch,
                "--state",
                "all",
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
        let matched = values.into_iter().find(|value| current.matches_head(value));
        match (matched, first_error) {
            (Some(value), _) => Ok(Some(value)),
            (None, Some(cause)) => Err(cause),
            (None, None) => Ok(None),
        }
    }

    pub(super) fn default_branch(&self, repository: &Path) -> Option<String> {
        self.default_branches.resolve(repository, || {
            super::details::default_branch(repository).or_else(|| {
                let bytes = self
                    .run(repository, &["repo", "view", "--json", "defaultBranchRef"])
                    .ok()?;
                let value: Value = serde_json::from_slice(&bytes).ok()?;
                let branch = value.pointer("/defaultBranchRef/name")?.as_str()?;
                validate_branch(repository, branch).ok()?;
                Some(branch.to_owned())
            })
        })
    }

    fn prepare_checkout(&self, repository: &Path, identity: &str, number: u64) -> Result<String> {
        let bytes = self.run(
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

struct CurrentBranch {
    branch: String,
    head: String,
    configured_number: Option<u64>,
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
        value.get("headRefName").and_then(Value::as_str) == Some(&self.branch)
            && ((value.get("state").and_then(Value::as_str) == Some("OPEN")
                && value.get("isCrossRepository").and_then(Value::as_bool) == Some(false))
                || self.matches_head(value))
    }
}

fn current_branch(repository: &Path) -> Result<Option<CurrentBranch>> {
    let summary = read::summary(repository)?;
    let (Some(branch), Some(head)) = (summary.branch, summary.head) else {
        return Ok(None);
    };
    let number = run(
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
    .filter(|number| *number > 0);
    Ok(Some(CurrentBranch {
        branch,
        head,
        configured_number: number,
    }))
}

fn fields(checks: bool) -> String {
    if checks {
        format!("{FIELDS},statusCheckRollup")
    } else {
        FIELDS.into()
    }
}
fn no_pr(cause: &crate::ServerError) -> bool {
    let message = cause.message().to_ascii_lowercase();
    message.contains("no pull requests found")
        || message.contains("no pull request found")
        || message.contains("could not resolve")
        || message.contains("no commits between")
}
fn valid_segment(value: &str) -> bool {
    !matches!(value, "" | "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPullRequest {
    number: u64,
    url: String,
    title: String,
    author: Option<Author>,
    head_ref_name: String,
    head_ref_oid: String,
    base_ref_name: String,
    state: String,
    is_draft: bool,
    updated_at: Option<String>,
    mergeable: Option<String>,
    merge_state_status: Option<String>,
    is_cross_repository: bool,
    #[serde(default)]
    status_check_rollup: Value,
}
#[derive(Deserialize)]
struct Author {
    login: String,
}

fn parse(raw: RawPullRequest) -> Result<GitPullRequest> {
    if raw.number == 0
        || !raw.url.starts_with("https://")
        || raw.head_ref_name.is_empty()
        || raw.base_ref_name.is_empty()
    {
        return Err(error("Invalid pull request data"));
    }
    Ok(GitPullRequest {
        number: raw.number,
        url: raw.url,
        title: raw.title,
        author: raw.author.map_or_else(String::new, |author| author.login),
        head_branch: raw.head_ref_name,
        head_oid: raw.head_ref_oid,
        base_branch: raw.base_ref_name,
        state: raw.state,
        draft: raw.is_draft,
        updated_at: raw.updated_at,
        mergeable: match raw.mergeable.as_deref() {
            Some("MERGEABLE") => Some(true),
            Some("CONFLICTING") => Some(false),
            _ => None,
        },
        merge_state: raw.merge_state_status.unwrap_or_else(|| "UNKNOWN".into()),
        cross_repository: raw.is_cross_repository,
        checks: parse_checks(&raw.status_check_rollup)?,
    })
}

fn parse_checks(value: &Value) -> Result<GitChecks> {
    let entries = match value {
        Value::Null => &[][..],
        Value::Array(entries) => entries.as_slice(),
        _ => return Err(error("Invalid PR checks")),
    };
    let mut checks = GitChecks::default();
    for entry in entries {
        let object = entry.as_object().ok_or_else(|| error("Invalid PR check"))?;
        let get = |key| object.get(key).and_then(Value::as_str);
        let outcome = if get("__typename") == Some("CheckRun") {
            if get("status") == Some("COMPLETED") {
                get("conclusion").unwrap_or("PENDING")
            } else {
                "PENDING"
            }
        } else {
            get("state").unwrap_or("PENDING")
        };
        match outcome.to_ascii_uppercase().as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => checks.passing += 1,
            "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED"
            | "STARTUP_FAILURE" => checks.failing += 1,
            _ => checks.pending += 1,
        }
    }
    Ok(checks)
}
