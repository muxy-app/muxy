use super::super::{Result, error};
use muxy_protocol::{GitChecks, GitPullRequest};
use serde::Deserialize;
use serde_json::Value;

const FIELDS: &str = "number,url,title,author,headRefName,headRefOid,baseRefName,state,isDraft,updatedAt,mergeable,mergeStateStatus,isCrossRepository";

pub(super) fn fields(checks: bool) -> String {
    if checks {
        format!("{FIELDS},statusCheckRollup")
    } else {
        FIELDS.into()
    }
}

/// The repository URL `gh repo view --json url` printed.
pub(super) fn identity(bytes: &[u8]) -> Result<String> {
    let value: Value = serde_json::from_slice(bytes).map_err(error)?;
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

pub(super) fn is_open(value: &Value) -> bool {
    value.get("state").and_then(Value::as_str) == Some("OPEN")
}

/// The GitHub CLI failed because there is no such pull request.
pub(super) fn no_pr(cause: &crate::ServerError) -> bool {
    let message = cause.message().to_ascii_lowercase();
    message.contains("no pull requests found")
        || message.contains("no pull request found")
        || message.contains("could not resolve")
        || message.contains("no commits between")
}

pub(super) fn valid_segment(value: &str) -> bool {
    !matches!(value, "" | "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPullRequest {
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

pub(super) fn pull_request(raw: RawPullRequest) -> Result<GitPullRequest> {
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
        checks: checks(&raw.status_check_rollup)?,
    })
}

fn checks(value: &Value) -> Result<GitChecks> {
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
