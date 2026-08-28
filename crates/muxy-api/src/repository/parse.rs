use super::model::{
    ChangedFile, ChangedFiles, LineStat, RepositoryHead, RepositoryParseError, RepositorySummary,
};
use super::{
    GitHubRepositoryIdentity, PullRequestChecks, PullRequestChecksStatus, PullRequestInfo,
    PullRequestMergeState, PullRequestMergeable, PullRequestParseError, PullRequestState,
    ValidatedExternalUrl,
};
use serde::Deserialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const GH_STDOUT_LIMIT: usize = 2 * 1_024 * 1_024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPullRequest {
    url: String,
    number: u64,
    state: Value,
    is_draft: bool,
    base_ref_name: String,
    mergeable: Value,
    merge_state_status: Value,
    status_check_rollup: Value,
    is_cross_repository: bool,
    head_ref_oid: String,
    head_ref_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGitHubRepository {
    name_with_owner: String,
    url: String,
}

pub(crate) fn parse_github_repository(
    input: &[u8],
) -> Result<GitHubRepositoryIdentity, PullRequestParseError> {
    ensure_gh_size(input)?;
    let raw: RawGitHubRepository =
        serde_json::from_slice(input).map_err(|_| PullRequestParseError::Json)?;
    let url = ValidatedExternalUrl::parse(raw.url)?;
    let remainder = url
        .as_str()
        .strip_prefix("https://")
        .ok_or(PullRequestParseError::RepositoryIdentity)?;
    let segments: Vec<&str> = remainder.split('/').collect();
    if segments.len() != 3
        || segments
            .iter()
            .any(|segment| !valid_identity_segment(segment))
    {
        return Err(PullRequestParseError::RepositoryIdentity);
    }
    let named: Vec<&str> = raw.name_with_owner.split('/').collect();
    let matches = match named.as_slice() {
        [owner, name] => *owner == segments[1] && *name == segments[2],
        [host, owner, name] => {
            *host == segments[0] && *owner == segments[1] && *name == segments[2]
        }
        _ => false,
    };
    if !matches || named.iter().any(|segment| !valid_identity_segment(segment)) {
        return Err(PullRequestParseError::RepositoryIdentity);
    }
    Ok(GitHubRepositoryIdentity {
        host: segments[0].to_owned(),
        owner: segments[1].to_owned(),
        name: segments[2].to_owned(),
    })
}

pub(crate) fn parse_pull_request(
    input: &[u8],
    expected_branch: &[u8],
    expected_oid: &[u8],
) -> Result<PullRequestInfo, PullRequestParseError> {
    ensure_gh_size(input)?;
    let raw: RawPullRequest =
        serde_json::from_slice(input).map_err(|_| PullRequestParseError::Json)?;
    parse_raw_pull_request(raw, expected_branch, expected_oid)
}

pub(crate) fn parse_pull_request_list(
    input: &[u8],
    expected_branch: &[u8],
    expected_oid: &[u8],
) -> Result<Option<PullRequestInfo>, PullRequestParseError> {
    ensure_gh_size(input)?;
    let raw: Vec<RawPullRequest> =
        serde_json::from_slice(input).map_err(|_| PullRequestParseError::Json)?;
    for candidate in raw {
        if candidate.head_ref_name.as_bytes() == expected_branch
            && candidate.head_ref_oid.eq_ignore_ascii_case(
                std::str::from_utf8(expected_oid)
                    .map_err(|_| PullRequestParseError::HeadIdentity)?,
            )
        {
            return parse_raw_pull_request(candidate, expected_branch, expected_oid).map(Some);
        }
    }
    Ok(None)
}

fn parse_raw_pull_request(
    raw: RawPullRequest,
    expected_branch: &[u8],
    expected_oid: &[u8],
) -> Result<PullRequestInfo, PullRequestParseError> {
    if raw.number == 0
        || raw.base_ref_name.is_empty()
        || raw.head_ref_name.as_bytes() != expected_branch
        || !raw
            .head_ref_oid
            .as_bytes()
            .eq_ignore_ascii_case(expected_oid)
    {
        return Err(PullRequestParseError::HeadIdentity);
    }
    let checks = parse_pull_request_checks(&raw.status_check_rollup)?;
    Ok(PullRequestInfo {
        url: ValidatedExternalUrl::parse(raw.url)?,
        number: raw.number,
        state: parse_pr_state(&raw.state)?,
        is_draft: raw.is_draft,
        base_branch: raw.base_ref_name,
        mergeable: parse_mergeable(&raw.mergeable)?,
        merge_state: parse_merge_state(&raw.merge_state_status)?,
        checks,
        is_cross_repository: raw.is_cross_repository,
        head_oid: raw.head_ref_oid,
        head_branch: raw.head_ref_name,
    })
}

fn parse_pr_state(value: &Value) -> Result<PullRequestState, PullRequestParseError> {
    let raw = nullable_enum(value)?;
    Ok(match raw {
        "OPEN" => PullRequestState::Open,
        "CLOSED" => PullRequestState::Closed,
        "MERGED" => PullRequestState::Merged,
        other => PullRequestState::Unknown(other.to_owned()),
    })
}

fn parse_mergeable(value: &Value) -> Result<PullRequestMergeable, PullRequestParseError> {
    let raw = nullable_enum(value)?;
    Ok(match raw {
        "MERGEABLE" => PullRequestMergeable::Mergeable,
        "CONFLICTING" => PullRequestMergeable::Conflicting,
        other => PullRequestMergeable::Unknown(other.to_owned()),
    })
}

fn parse_merge_state(value: &Value) -> Result<PullRequestMergeState, PullRequestParseError> {
    let raw = nullable_enum(value)?;
    Ok(match raw {
        "CLEAN" => PullRequestMergeState::Clean,
        "HAS_HOOKS" => PullRequestMergeState::HasHooks,
        "UNSTABLE" => PullRequestMergeState::Unstable,
        "BEHIND" => PullRequestMergeState::Behind,
        "BLOCKED" => PullRequestMergeState::Blocked,
        "DIRTY" => PullRequestMergeState::Dirty,
        "DRAFT" => PullRequestMergeState::Draft,
        other => PullRequestMergeState::Unknown(other.to_owned()),
    })
}

fn nullable_enum(value: &Value) -> Result<&str, PullRequestParseError> {
    match value {
        Value::String(value) => Ok(value),
        Value::Null => Ok("UNKNOWN"),
        _ => Err(PullRequestParseError::Shape),
    }
}

fn parse_pull_request_checks(value: &Value) -> Result<PullRequestChecks, PullRequestParseError> {
    let entries = match value {
        Value::Null => &[][..],
        Value::Array(entries) => entries.as_slice(),
        _ => return Err(PullRequestParseError::Shape),
    };
    let mut passing = 0_usize;
    let mut failing = 0_usize;
    let mut pending = 0_usize;
    for entry in entries {
        let object = entry.as_object().ok_or(PullRequestParseError::Shape)?;
        let typename = object
            .get("__typename")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let outcome = if typename == "CheckRun" {
            if object.get("status").and_then(Value::as_str) != Some("COMPLETED") {
                "PENDING"
            } else {
                object
                    .get("conclusion")
                    .and_then(Value::as_str)
                    .unwrap_or("PENDING")
            }
        } else {
            object
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("PENDING")
        };
        match outcome.to_ascii_uppercase().as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => passing += 1,
            "FAILURE" | "ERROR" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED"
            | "STARTUP_FAILURE" => failing += 1,
            _ => pending += 1,
        }
    }
    let total = passing
        .checked_add(failing)
        .and_then(|total| total.checked_add(pending))
        .ok_or(PullRequestParseError::Shape)?;
    let status = if failing > 0 {
        PullRequestChecksStatus::Failure
    } else if pending > 0 {
        PullRequestChecksStatus::Pending
    } else if passing > 0 {
        PullRequestChecksStatus::Success
    } else {
        PullRequestChecksStatus::None
    };
    Ok(PullRequestChecks {
        status,
        passing,
        failing,
        pending,
        total,
    })
}

fn ensure_gh_size(input: &[u8]) -> Result<(), PullRequestParseError> {
    if input.len() > GH_STDOUT_LIMIT {
        Err(PullRequestParseError::Oversized)
    } else {
        Ok(())
    }
}

fn valid_identity_segment(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatusRecord {
    pub path: Vec<u8>,
    pub old_path: Option<Vec<u8>>,
    pub x_status: u8,
    pub y_status: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NumstatRecord {
    pub path: Vec<u8>,
    pub old_path: Option<Vec<u8>>,
    pub stat: LineStat,
}

pub(crate) fn parse_summary(input: &[u8]) -> Result<RepositorySummary, RepositoryParseError> {
    let input = std::str::from_utf8(input).map_err(|_| RepositoryParseError::Summary)?;
    let mut oid = None;
    let mut branch = None;
    let mut upstream = None;
    let mut ahead_behind = None;
    let mut changed_count = 0_usize;
    let mut staged_count = 0_usize;
    let mut unstaged_count = 0_usize;
    let mut untracked_count = 0_usize;
    let mut conflicted_count = 0_usize;

    for raw_line in input.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.oid ") {
            set_once(&mut oid, value.to_owned(), RepositoryParseError::Summary)?;
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.head ") {
            if value.is_empty() {
                return Err(RepositoryParseError::Summary);
            }
            set_once(&mut branch, value.to_owned(), RepositoryParseError::Summary)?;
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.upstream ") {
            if value.is_empty() {
                return Err(RepositoryParseError::Summary);
            }
            set_once(
                &mut upstream,
                value.to_owned(),
                RepositoryParseError::Summary,
            )?;
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.ab ") {
            let counts: Vec<&str> = value.split(' ').collect();
            if counts.len() != 2 {
                return Err(RepositoryParseError::Summary);
            }
            let ahead = counts[0]
                .strip_prefix('+')
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or(RepositoryParseError::Summary)?;
            let behind = counts[1]
                .strip_prefix('-')
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or(RepositoryParseError::Summary)?;
            set_once(
                &mut ahead_behind,
                (ahead, behind),
                RepositoryParseError::Summary,
            )?;
            continue;
        }
        if line.starts_with("# ") {
            continue;
        }
        if let Some(path) = line.strip_prefix("? ") {
            if path.is_empty() {
                return Err(RepositoryParseError::Summary);
            }
            increment(&mut changed_count)?;
            increment(&mut untracked_count)?;
            continue;
        }
        if let Some(path) = line.strip_prefix("! ") {
            if path.is_empty() {
                return Err(RepositoryParseError::Summary);
            }
            continue;
        }
        if line.starts_with("u ") {
            let fields: Vec<&str> = line.splitn(11, ' ').collect();
            if fields.len() != 11 || !valid_v2_xy(fields[1]) || fields[10].is_empty() {
                return Err(RepositoryParseError::Summary);
            }
            increment(&mut changed_count)?;
            increment(&mut staged_count)?;
            increment(&mut unstaged_count)?;
            increment(&mut conflicted_count)?;
            continue;
        }
        let field_count = if line.starts_with("1 ") {
            9
        } else if line.starts_with("2 ") {
            10
        } else {
            return Err(RepositoryParseError::Summary);
        };
        let fields: Vec<&str> = line.splitn(field_count, ' ').collect();
        if fields.len() != field_count
            || !valid_v2_xy(fields[1])
            || fields[field_count - 1].is_empty()
            || (field_count == 10 && !fields[9].contains('\t'))
        {
            return Err(RepositoryParseError::Summary);
        }
        increment(&mut changed_count)?;
        let status = fields[1].as_bytes();
        if status[0] != b'.' {
            increment(&mut staged_count)?;
        }
        if status[1] != b'.' {
            increment(&mut unstaged_count)?;
        }
    }

    let oid = oid.ok_or(RepositoryParseError::Summary)?;
    let branch = branch.ok_or(RepositoryParseError::Summary)?;
    if upstream.is_some() != ahead_behind.is_some() {
        return Err(RepositoryParseError::Summary);
    }
    let head = if oid == "(initial)" {
        RepositoryHead::Unborn
    } else if oid.len() >= 7 && oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        RepositoryHead::Commit(oid)
    } else {
        return Err(RepositoryParseError::Summary);
    };
    let is_detached = branch == "(detached)";
    if is_detached && head == RepositoryHead::Unborn {
        return Err(RepositoryParseError::Summary);
    }
    let (ahead, behind) = ahead_behind.unwrap_or_default();
    Ok(RepositorySummary {
        branch,
        head,
        is_detached,
        upstream,
        ahead,
        behind,
        changed_count,
        staged_count,
        unstaged_count,
        untracked_count,
        conflicted_count,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    error: RepositoryParseError,
) -> Result<(), RepositoryParseError> {
    if slot.replace(value).is_some() {
        return Err(error);
    }
    Ok(())
}

fn increment(value: &mut usize) -> Result<(), RepositoryParseError> {
    *value = value.checked_add(1).ok_or(RepositoryParseError::Summary)?;
    Ok(())
}

fn valid_v2_xy(value: &str) -> bool {
    value.len() == 2
        && value
            .bytes()
            .all(|byte| matches!(byte, b'.' | b'A' | b'C' | b'D' | b'M' | b'R' | b'T' | b'U'))
}

pub(crate) fn parse_status(input: &[u8]) -> Result<Vec<StatusRecord>, RepositoryParseError> {
    let mut cursor = 0;
    let mut records = Vec::new();
    while cursor < input.len() {
        let token = take_nul(input, &mut cursor).ok_or(RepositoryParseError::Status)?;
        if token.len() < 4 || token[2] != b' ' || token[3..].is_empty() {
            return Err(RepositoryParseError::Status);
        }
        let x_status = token[0];
        let y_status = token[1];
        if !valid_v1_status(x_status, y_status) {
            return Err(RepositoryParseError::Status);
        }
        if x_status == b'!' && y_status == b'!' {
            continue;
        }
        let path = token[3..].to_vec();
        let renamed = matches!(x_status, b'R' | b'C') || matches!(y_status, b'R' | b'C');
        let old_path = if renamed {
            let old_path = take_nul(input, &mut cursor).ok_or(RepositoryParseError::Status)?;
            if old_path.is_empty() {
                return Err(RepositoryParseError::Status);
            }
            Some(old_path.to_vec())
        } else {
            None
        };
        records.push(StatusRecord {
            path,
            old_path,
            x_status,
            y_status,
        });
    }
    Ok(records)
}

fn valid_v1_status(x_status: u8, y_status: u8) -> bool {
    if matches!((x_status, y_status), (b'?', b'?') | (b'!', b'!')) {
        return true;
    }
    let valid = |status| {
        matches!(
            status,
            b' ' | b'A' | b'C' | b'D' | b'M' | b'R' | b'T' | b'U'
        )
    };
    valid(x_status) && valid(y_status) && x_status != b'?' && y_status != b'?'
}

pub(crate) fn parse_numstat(input: &[u8]) -> Result<Vec<NumstatRecord>, RepositoryParseError> {
    let mut cursor = 0;
    let mut records = Vec::new();
    while cursor < input.len() {
        let record = take_nul(input, &mut cursor).ok_or(RepositoryParseError::Numstat)?;
        let first_tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(RepositoryParseError::Numstat)?;
        let second_tab = record[first_tab + 1..]
            .iter()
            .position(|byte| *byte == b'\t')
            .map(|position| first_tab + 1 + position)
            .ok_or(RepositoryParseError::Numstat)?;
        let additions = &record[..first_tab];
        let deletions = &record[first_tab + 1..second_tab];
        let path = &record[second_tab + 1..];
        let stat = parse_line_stat(additions, deletions)?;
        if path.is_empty() {
            let old_path = take_nul(input, &mut cursor).ok_or(RepositoryParseError::Numstat)?;
            let new_path = take_nul(input, &mut cursor).ok_or(RepositoryParseError::Numstat)?;
            if old_path.is_empty() || new_path.is_empty() {
                return Err(RepositoryParseError::Numstat);
            }
            records.push(NumstatRecord {
                path: new_path.to_vec(),
                old_path: Some(old_path.to_vec()),
                stat,
            });
        } else {
            records.push(NumstatRecord {
                path: path.to_vec(),
                old_path: None,
                stat,
            });
        }
    }
    Ok(records)
}

fn parse_line_stat(additions: &[u8], deletions: &[u8]) -> Result<LineStat, RepositoryParseError> {
    if additions == b"-" && deletions == b"-" {
        return Ok(LineStat {
            additions: None,
            deletions: None,
            binary: true,
        });
    }
    if additions == b"-" || deletions == b"-" {
        return Err(RepositoryParseError::Numstat);
    }
    let additions = std::str::from_utf8(additions)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(RepositoryParseError::Numstat)?;
    let deletions = std::str::from_utf8(deletions)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(RepositoryParseError::Numstat)?;
    Ok(LineStat {
        additions: Some(additions),
        deletions: Some(deletions),
        binary: false,
    })
}

fn take_nul<'a>(input: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let end = input[*cursor..].iter().position(|byte| *byte == 0)? + *cursor;
    let value = &input[*cursor..end];
    *cursor = end + 1;
    Some(value)
}

pub(crate) fn aggregate(
    status: Vec<StatusRecord>,
    combined: Vec<NumstatRecord>,
    staged: Vec<NumstatRecord>,
    unstaged: Vec<NumstatRecord>,
) -> Result<ChangedFiles, RepositoryParseError> {
    let conflicts: HashSet<Vec<u8>> = status
        .iter()
        .filter(|record| is_conflicted(record.x_status, record.y_status))
        .map(|record| record.path.clone())
        .collect();
    let combined = numstat_map(combined, &conflicts)?;
    let staged = numstat_map(staged, &conflicts)?;
    let unstaged = numstat_map(unstaged, &conflicts)?;
    let mut files = Vec::with_capacity(status.len());
    for status in status {
        let combined_stat = matching_stat(&status, &combined)?;
        let staged_stat = matching_stat(&status, &staged)?;
        let unstaged_stat = matching_stat(&status, &unstaged)?;
        let is_untracked = status.x_status == b'?' && status.y_status == b'?';
        let is_conflicted = is_conflicted(status.x_status, status.y_status);
        let is_staged = !matches!(status.x_status, b' ' | b'?');
        let is_unstaged = is_untracked || !matches!(status.y_status, b' ' | b'?');
        let is_binary = [combined_stat, staged_stat, unstaged_stat]
            .into_iter()
            .flatten()
            .any(|stat| stat.binary);
        files.push(ChangedFile {
            path: status.path,
            old_path: status.old_path,
            x_status: status.x_status,
            y_status: status.y_status,
            is_staged,
            is_unstaged,
            is_untracked,
            is_conflicted,
            is_binary,
            combined_stat,
            staged_stat,
            unstaged_stat,
        });
    }
    files.sort_by(|left, right| natural_cmp(&left.path, &right.path));
    let mut changes = ChangedFiles {
        files,
        ..ChangedFiles::default()
    };
    for file in &changes.files {
        changes.total_lines.add(file.combined_stat);
        if file.is_conflicted {
            changes.conflict_lines.add(file.combined_stat);
        } else {
            if file.is_staged {
                changes.staged_lines.add(file.staged_stat);
            }
            if file.is_unstaged {
                changes.unstaged_lines.add(file.unstaged_stat);
            }
        }
    }
    Ok(changes)
}

fn numstat_map(
    records: Vec<NumstatRecord>,
    conflicts: &HashSet<Vec<u8>>,
) -> Result<HashMap<Vec<u8>, NumstatRecord>, RepositoryParseError> {
    let mut map: HashMap<Vec<u8>, NumstatRecord> = HashMap::with_capacity(records.len());
    for record in records {
        if let Some(existing) = map.get_mut(&record.path) {
            if !conflicts.contains(&record.path) || existing.old_path != record.old_path {
                return Err(RepositoryParseError::DuplicateNumstat);
            }
            existing.stat = merge_line_stats(existing.stat, record.stat)?;
        } else {
            map.insert(record.path.clone(), record);
        }
    }
    Ok(map)
}

fn merge_line_stats(left: LineStat, right: LineStat) -> Result<LineStat, RepositoryParseError> {
    if left.binary || right.binary {
        return Ok(LineStat {
            additions: None,
            deletions: None,
            binary: true,
        });
    }
    let additions = left
        .additions
        .zip(right.additions)
        .and_then(|(left, right)| left.checked_add(right))
        .ok_or(RepositoryParseError::Numstat)?;
    let deletions = left
        .deletions
        .zip(right.deletions)
        .and_then(|(left, right)| left.checked_add(right))
        .ok_or(RepositoryParseError::Numstat)?;
    Ok(LineStat {
        additions: Some(additions),
        deletions: Some(deletions),
        binary: false,
    })
}

fn matching_stat(
    status: &StatusRecord,
    stats: &HashMap<Vec<u8>, NumstatRecord>,
) -> Result<Option<LineStat>, RepositoryParseError> {
    let Some(record) = stats.get(&status.path) else {
        return Ok(None);
    };
    if let (Some(status_old_path), Some(stat_old_path)) =
        (status.old_path.as_deref(), record.old_path.as_deref())
        && status_old_path != stat_old_path
    {
        return Err(RepositoryParseError::Numstat);
    }
    Ok(Some(record.stat))
}

fn is_conflicted(x_status: u8, y_status: u8) -> bool {
    matches!(
        (x_status, y_status),
        (b'D', b'D')
            | (b'A', b'U')
            | (b'U', b'D')
            | (b'U', b'A')
            | (b'D', b'U')
            | (b'A', b'A')
            | (b'U', b'U')
    )
}

pub(crate) fn parse_remote_heads(input: &[u8]) -> Result<Vec<Vec<u8>>, RepositoryParseError> {
    let mut branches = Vec::new();
    let mut seen = HashSet::new();
    for line in input.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let tab = line
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(RepositoryParseError::RemoteBranches)?;
        if tab == 0 || line[tab + 1..].contains(&b'\t') {
            return Err(RepositoryParseError::RemoteBranches);
        }
        let reference = &line[tab + 1..];
        if let Some(branch) = reference.strip_prefix(b"refs/heads/") {
            if branch.is_empty() {
                return Err(RepositoryParseError::RemoteBranches);
            }
            if seen.insert(branch.to_vec()) {
                branches.push(branch.to_vec());
            }
        }
    }
    branches.sort_by(|left, right| natural_cmp(left, right));
    Ok(branches)
}

pub(crate) fn parse_remote_default_branch(
    input: &[u8],
) -> Result<Option<Vec<u8>>, RepositoryParseError> {
    let mut branch = None;
    for line in input.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let tab = line
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or(RepositoryParseError::RemoteBranches)?;
        if &line[tab + 1..] != b"HEAD" {
            continue;
        }
        let Some(reference) = line[..tab].strip_prefix(b"ref: ") else {
            continue;
        };
        let Some(value) = reference.strip_prefix(b"refs/heads/") else {
            return Err(RepositoryParseError::RemoteBranches);
        };
        if value.is_empty() || branch.replace(value.to_vec()).is_some() {
            return Err(RepositoryParseError::RemoteBranches);
        }
    }
    Ok(branch)
}

pub(crate) fn parse_subjects(input: &[u8]) -> Result<Vec<String>, RepositoryParseError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input.last() != Some(&0) {
        return Err(RepositoryParseError::CommitSubjects);
    }
    input[..input.len() - 1]
        .split(|byte| *byte == 0)
        .take(12)
        .map(|subject| {
            let subject =
                std::str::from_utf8(subject).map_err(|_| RepositoryParseError::CommitSubjects)?;
            let mut end = subject.len().min(4_096);
            while !subject.is_char_boundary(end) {
                end -= 1;
            }
            Ok(subject[..end].to_owned())
        })
        .collect()
}

pub(crate) fn natural_cmp(left: &[u8], right: &[u8]) -> Ordering {
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = digit_end(left, left_index);
            let right_end = digit_end(right, right_index);
            let left_digits = trim_zeroes(&left[left_index..left_end]);
            let right_digits = trim_zeroes(&right[right_index..right_end]);
            let order = left_digits
                .len()
                .cmp(&right_digits.len())
                .then_with(|| left_digits.cmp(right_digits))
                .then_with(|| (left_end - left_index).cmp(&(right_end - right_index)));
            if order != Ordering::Equal {
                return order;
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }
        let left_byte = left[left_index].to_ascii_lowercase();
        let right_byte = right[right_index].to_ascii_lowercase();
        let order = left_byte
            .cmp(&right_byte)
            .then_with(|| left[left_index].cmp(&right[right_index]));
        if order != Ordering::Equal {
            return order;
        }
        left_index += 1;
        right_index += 1;
    }
    left.len().cmp(&right.len())
}

fn digit_end(value: &[u8], mut index: usize) -> usize {
    while index < value.len() && value[index].is_ascii_digit() {
        index += 1;
    }
    index
}

fn trim_zeroes(value: &[u8]) -> &[u8] {
    let first = value
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(value.len().saturating_sub(1));
    &value[first..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{RepositoryHead, display_path_lossy};

    #[test]
    fn repository_parse_summary_handles_clean_upstream_and_ahead_behind() {
        let summary = parse_summary(
            b"# branch.oid abcdef1234567890\n\
              # branch.head feature/topbar\n\
              # branch.upstream origin/feature/topbar\n\
              # branch.ab +2 -1\n",
        )
        .unwrap();

        assert_eq!(summary.branch, "feature/topbar");
        assert_eq!(summary.display_branch(), "feature/topbar");
        assert_eq!(
            summary.head,
            RepositoryHead::Commit("abcdef1234567890".into())
        );
        assert_eq!(summary.upstream.as_deref(), Some("origin/feature/topbar"));
        assert_eq!(summary.ahead, 2);
        assert_eq!(summary.behind, 1);
        assert!(!summary.is_dirty());
    }

    #[test]
    fn repository_parse_summary_counts_every_change_class() {
        let summary = parse_summary(
            b"# branch.oid abcdef1234567890\n\
              # branch.head main\n\
              1 M. N... 100644 100644 100644 abc abc staged.swift\n\
              1 .M N... 100644 100644 100644 abc abc unstaged.swift\n\
              1 MM N... 100644 100644 100644 abc abc both.swift\n\
              2 R. N... 100644 100644 100644 abc abc R100 renamed.swift\told.swift\n\
              ? new.swift\n\
              u UU N... 100644 100644 100644 100644 abc abc abc conflict.swift\n",
        )
        .unwrap();

        assert_eq!(summary.changed_count, 6);
        assert_eq!(summary.staged_count, 4);
        assert_eq!(summary.unstaged_count, 3);
        assert_eq!(summary.untracked_count, 1);
        assert_eq!(summary.conflicted_count, 1);
        assert!(summary.is_dirty());
        assert_eq!(summary.upstream, None);
    }

    #[test]
    fn repository_parse_summary_handles_detached_and_unborn_heads() {
        let detached =
            parse_summary(b"# branch.oid abcdef1234567890\n# branch.head (detached)\n").unwrap();
        assert!(detached.is_detached);
        assert_eq!(detached.display_branch(), "Detached abcdef1");

        let unborn = parse_summary(b"# branch.oid (initial)\n# branch.head main\n").unwrap();
        assert_eq!(unborn.head, RepositoryHead::Unborn);
        assert!(!unborn.is_detached);
        assert_eq!(unborn.display_branch(), "main");
    }

    #[test]
    fn repository_parse_summary_rejects_incomplete_or_conflicting_metadata() {
        for input in [
            b"? file\n".as_slice(),
            b"# branch.head main\n".as_slice(),
            b"# branch.oid abc\n".as_slice(),
            b"# branch.oid abcdef1\n# branch.head main\n# branch.upstream origin/main\n"
                .as_slice(),
            b"# branch.oid abcdef1\n# branch.head main\n# branch.ab +1 -0\n".as_slice(),
            b"# branch.oid abcdef1\n# branch.oid abcdef2\n# branch.head main\n"
                .as_slice(),
            b"# branch.oid nope\n# branch.head main\n".as_slice(),
            b"# branch.oid abcdef1\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +x -0\n"
                .as_slice(),
        ] {
            assert!(parse_summary(input).is_err());
        }
    }

    #[test]
    fn repository_parse_status_preserves_raw_paths_and_rename_pairs() {
        let mut input = Vec::new();
        input.extend_from_slice(b"M  path with space\0");
        input.extend_from_slice(b"?? tab\tline\n");
        input.extend_from_slice(&[0xff, 0xfe]);
        input.push(0);
        input.extend_from_slice(b"R  renamed\0old name\0");
        input.extend_from_slice(b" C copied\0source\0");

        let status = parse_status(&input).unwrap();

        assert_eq!(status.len(), 4);
        assert_eq!(status[0].path, b"path with space");
        assert_eq!(status[1].path, b"tab\tline\n\xff\xfe");
        assert_eq!(display_path_lossy(&status[1].path), "tab\tline\n��");
        assert_eq!(status[2].path, b"renamed");
        assert_eq!(status[2].old_path.as_deref(), Some(b"old name".as_slice()));
        assert_eq!(status[3].path, b"copied");
        assert_eq!(status[3].old_path.as_deref(), Some(b"source".as_slice()));
    }

    #[test]
    fn repository_parse_status_rejects_malformed_and_incomplete_records() {
        for input in [
            b"M  missing-nul".as_slice(),
            b"M short\0".as_slice(),
            b"M? invalid-separator\0".as_slice(),
            b"R  renamed\0".as_slice(),
            b"R  renamed\0\0".as_slice(),
            b"?? \0".as_slice(),
        ] {
            assert!(parse_status(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn repository_parse_numstat_handles_normal_rename_copy_binary_and_raw_bytes() {
        let mut input = Vec::new();
        input.extend_from_slice(b"5\t3\tpath with\ttab\nand ");
        input.extend_from_slice(&[0xff]);
        input.push(0);
        input.extend_from_slice(b"1\t2\t\0old\0new\0");
        input.extend_from_slice(b"-\t-\tbinary.png\0");

        let records = parse_numstat(&input).unwrap();

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].path, b"path with\ttab\nand \xff");
        assert_eq!(records[0].stat.additions, Some(5));
        assert_eq!(records[0].stat.deletions, Some(3));
        assert!(!records[0].stat.binary);
        assert_eq!(records[1].path, b"new");
        assert_eq!(records[1].old_path.as_deref(), Some(b"old".as_slice()));
        assert_eq!(records[2].stat.additions, None);
        assert_eq!(records[2].stat.deletions, None);
        assert!(records[2].stat.binary);
    }

    #[test]
    fn repository_parse_numstat_rejects_malformed_framing_and_counts() {
        for input in [
            b"1\t2\tpath".as_slice(),
            b"1\t2\t\0old\0".as_slice(),
            b"1\t2\t\0old\0\0".as_slice(),
            b"1\t2\t\0\0new\0".as_slice(),
            b"x\t2\tpath\0".as_slice(),
            b"-\t2\tpath\0".as_slice(),
            b"1\t2\t\0old\0new".as_slice(),
            b"1\t2\t\0old\0new\0tail".as_slice(),
        ] {
            assert!(parse_numstat(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn repository_aggregation_builds_sections_stats_related_paths_and_stable_ids() {
        let status =
            parse_status(b"UU conflict\0MM both\0R  renamed\0old\0?? untracked\0M  binary\0")
                .unwrap();
        let combined = parse_numstat(
            b"4\t1\tconflict\0\
              7\t3\tboth\0\
              2\t2\t\0old\0renamed\0\
              -\t-\tbinary\0",
        )
        .unwrap();
        let staged = parse_numstat(
            b"2\t0\tboth\0\
              2\t2\t\0old\0renamed\0\
              -\t-\tbinary\0",
        )
        .unwrap();
        let unstaged = parse_numstat(b"5\t3\tboth\0").unwrap();

        let changes = aggregate(status, combined, staged, unstaged).unwrap();

        assert_eq!(changes.files.len(), 5);
        assert_eq!(changes.conflicts().len(), 1);
        assert_eq!(changes.staged().len(), 3);
        assert_eq!(changes.unstaged().len(), 2);
        let both = changes
            .files
            .iter()
            .find(|file| file.path == b"both")
            .unwrap();
        assert!(both.is_staged);
        assert!(both.is_unstaged);
        assert_eq!(both.combined_stat.unwrap().additions, Some(7));
        assert_eq!(both.staged_stat.unwrap().additions, Some(2));
        assert_eq!(both.unstaged_stat.unwrap().additions, Some(5));
        let renamed = changes
            .files
            .iter()
            .find(|file| file.path == b"renamed")
            .unwrap();
        assert_eq!(
            renamed.related_paths(),
            [b"old".as_slice(), b"renamed".as_slice()]
        );
        assert_ne!(
            renamed.stable_id(),
            crate::repository::ChangedFileId {
                path: b"renamed".to_vec(),
                old_path: Some(b"other".to_vec()),
            }
        );
        assert_eq!(changes.total_lines.additions, 13);
        assert_eq!(changes.total_lines.deletions, 6);
        assert_eq!(changes.total_lines.binary_files, 1);
        assert_eq!(changes.total_lines.unknown_files, 1);
        assert_eq!(changes.staged_lines.additions, 4);
        assert_eq!(changes.unstaged_lines.additions, 5);
        assert_eq!(changes.conflict_lines.additions, 4);
    }

    #[test]
    fn repository_aggregation_rejects_duplicate_stats_and_sorts_naturally() {
        let status = parse_status(b"M  file10\0M  file2\0M  file1\0").unwrap();
        let duplicate = parse_numstat(b"1\t0\tfile1\x002\t0\tfile1\0").unwrap();
        assert!(aggregate(status.clone(), duplicate, Vec::new(), Vec::new()).is_err());

        let changes = aggregate(status, Vec::new(), Vec::new(), Vec::new()).unwrap();
        let paths: Vec<&[u8]> = changes
            .files
            .iter()
            .map(|file| file.path.as_slice())
            .collect();
        assert_eq!(paths, [b"file1".as_slice(), b"file2", b"file10"]);
    }

    #[test]
    fn repository_parse_remote_heads_and_default_branch_are_strict_and_natural() {
        let heads = parse_remote_heads(
            b"abc\trefs/heads/feature10\n\
              def\trefs/tags/ignored\n\
              123\trefs/heads/feature2\n\
              456\trefs/heads/main\n",
        )
        .unwrap();
        assert_eq!(
            heads,
            [
                b"feature2".to_vec(),
                b"feature10".to_vec(),
                b"main".to_vec()
            ]
        );
        assert_eq!(
            parse_remote_default_branch(b"ref: refs/heads/main\tHEAD\nabc\tHEAD\n").unwrap(),
            Some(b"main".to_vec())
        );
        assert_eq!(parse_remote_default_branch(b"abc\tHEAD\n").unwrap(), None);
        assert!(parse_remote_heads(b"abc refs/heads/main\n").is_err());
        assert!(parse_remote_default_branch(b"ref: refs/tags/main\tHEAD\n").is_err());
    }

    #[test]
    fn repository_parse_subjects_caps_records_and_utf8_bytes_without_splitting_scalars() {
        let mut input = Vec::new();
        for index in 0..14 {
            input.extend_from_slice(format!("subject {index}").as_bytes());
            input.push(0);
        }
        let subjects = parse_subjects(&input).unwrap();
        assert_eq!(subjects.len(), 12);
        assert_eq!(subjects[0], "subject 0");
        assert_eq!(subjects[11], "subject 11");

        let long = format!("{}é", "a".repeat(4_095));
        let mut input = long.into_bytes();
        input.push(0);
        let subject = parse_subjects(&input).unwrap().remove(0);
        assert!(subject.len() <= 4_096);
        assert!(subject.is_char_boundary(subject.len()));
        assert!(parse_subjects(&[0xff, 0]).is_err());
        assert!(parse_subjects(b"missing terminator").is_err());
    }
}
