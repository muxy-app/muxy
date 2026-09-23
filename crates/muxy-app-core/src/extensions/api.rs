use muxy_protocol::{
    FilesAction, FilesReply, GitAction, GitChecks, GitDiffKind, GitDiffRequest, GitFileStatus,
    GitMergeMethod, GitPullRequest, GitPullRequestAction, GitPullRequestFilter, GitRefKind,
    GitReply, ServerPath,
};
use serde_json::{Value, json};

pub fn text<'a>(args: &'a Value, field: &str) -> Result<&'a str, String> {
    args[field]
        .as_str()
        .ok_or_else(|| format!("{field} must be a string"))
}

fn path(args: &Value, field: &str) -> Result<ServerPath, String> {
    text(args, field).map(|s| ServerPath(s.as_bytes().into()))
}

fn paths(args: &Value, field: &str) -> Result<Vec<ServerPath>, String> {
    let Some(values) = args[field].as_array() else {
        return if args[field].is_null() {
            Ok(Vec::new())
        } else {
            Err(format!("{field} must be an array"))
        };
    };
    values
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| ServerPath(s.as_bytes().into()))
                .ok_or_else(|| "path must be a string".into())
        })
        .collect()
}

fn number(args: &Value, field: &str, default: u32) -> Result<u32, String> {
    if args[field].is_null() {
        return Ok(default);
    }
    args[field]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| format!("{field} must be a nonnegative integer"))
}

fn boolean(args: &Value, field: &str) -> bool {
    args[field].as_bool().unwrap_or(false)
}

pub fn files_action(verb: &str, args: &Value) -> Result<FilesAction, String> {
    Ok(match verb {
        "files.list" => FilesAction::List(path(args, "path")?),
        "files.read" => FilesAction::Read(path(args, "path")?),
        "files.stat" => FilesAction::Stat(path(args, "path")?),
        "files.write" => FilesAction::Write {
            path: path(args, "path")?,
            content: text(args, "contents")?.into(),
        },
        "files.mkdir" => FilesAction::Mkdir(path(args, "path")?),
        "files.rename" => FilesAction::Rename {
            path: path(args, "path")?,
            name: path(args, "newName")?,
        },
        "files.move" => FilesAction::Move {
            paths: paths(args, "paths")?,
            into: path(args, "into")?,
        },
        "files.delete" => FilesAction::Delete(paths(args, "paths")?),
        _ => return Err(format!("unsupported API: {verb}")),
    })
}

pub fn path_text(path: &ServerPath) -> Result<&str, String> {
    std::str::from_utf8(&path.0).map_err(|_| "path is not valid UTF-8".into())
}

pub fn files_reply(reply: FilesReply) -> Result<Value, String> {
    Ok(match reply {
        FilesReply::Entries(entries) => Value::Array(entries.iter().map(|e| Ok(json!({"name":path_text(&e.name)?,"path":path_text(&e.path)?,"isDirectory":e.is_directory,"isIgnored":e.is_ignored}))).collect::<Result<_, String>>()?),
        FilesReply::Content(file) => json!({"path":path_text(&file.path)?,"content":file.content,"size":file.size}),
        FilesReply::Info(file) => json!({"path":path_text(&file.path)?,"name":path_text(&file.name)?,"isDirectory":file.is_directory,"size":file.size}),
        FilesReply::Path(path) => json!({"path":path_text(&path)?}),
        FilesReply::Paths(paths) => json!(paths.iter().map(path_text).collect::<Result<Vec<_>,_>>()?),
        FilesReply::Done => Value::Null,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the main extension API verb mapping together"
)]
pub fn git_action(verb: &str, args: &Value) -> Result<GitAction, String> {
    let string = |key| text(args, key).map(str::to_owned);
    let number64 = |key| {
        args[key]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("{key} must be a positive integer"))
    };
    let line_limit = || {
        if args["lineLimit"].is_null() {
            Ok(None)
        } else {
            number(args, "lineLimit", 0).map(Some)
        }
    };
    Ok(match verb {
        "git.status" => GitAction::Status {
            local: boolean(args, "local"),
        },
        "git.repoInfo" => GitAction::RepoInfo,
        "git.currentBranch" | "git.aheadBehind" => GitAction::Summary,
        "git.branches" => GitAction::Branches,
        "git.remoteBranches" => GitAction::RemoteBranches,
        "git.worktrees" => GitAction::Worktrees,
        "git.log" => GitAction::Log {
            max_count: number(args, "maxCount", 100)?,
            skip: number(args, "skip", 0)?,
        },
        "git.diff" => GitAction::Diff(GitDiffRequest {
            path: args["filePath"]
                .as_str()
                .filter(|p| !p.is_empty())
                .map(|p| ServerPath(p.as_bytes().into())),
            raw: boolean(args, "raw"),
            staged: boolean(args, "staged"),
            line_limit: line_limit()?,
        }),
        "git.init" => GitAction::Init,
        "git.stage" => GitAction::Stage(paths(args, "paths")?),
        "git.unstage" => GitAction::Unstage(paths(args, "paths")?),
        "git.discard" => {
            let mut values = paths(args, "paths")?;
            values.extend(paths(args, "untrackedPaths")?);
            values.sort_by(|a, b| a.0.cmp(&b.0));
            values.dedup();
            GitAction::Discard(values)
        }
        "git.commit" => GitAction::Commit {
            message: string("message")?,
            stage_all: boolean(args, "stageAll"),
        },
        "git.push" => GitAction::Push {
            set_upstream: boolean(args, "setUpstream"),
        },
        "git.pull" => GitAction::Pull,
        "git.checkout" => GitAction::Checkout(string("hash")?),
        "git.cherryPick" => GitAction::CherryPick(string("hash")?),
        "git.revert" => GitAction::Revert(string("hash")?),
        "git.branch.create" => GitAction::CreateBranch(string("name")?),
        "git.branch.switch" => GitAction::SwitchBranch(string("branch")?),
        "git.branch.delete" => GitAction::DeleteLocalBranch {
            name: string("name")?,
            force: boolean(args, "force"),
        },
        "git.branch.deleteRemote" => GitAction::DeleteRemoteBranch(string("branch")?),
        "git.tag.create" => GitAction::CreateTag {
            name: string("name")?,
            hash: string("hash")?,
        },
        "git.pr.info" => GitAction::PullRequest(GitPullRequestAction::Info),
        "git.pr.number" => GitAction::PullRequest(GitPullRequestAction::Number),
        "git.pr.diff" => GitAction::PullRequest(GitPullRequestAction::Diff {
            number: number64("number")?,
            line_limit: line_limit()?,
        }),
        "git.pr.list" => GitAction::PullRequest(GitPullRequestAction::List {
            filter: match args["filter"].as_str().unwrap_or("open") {
                "open" => GitPullRequestFilter::Open,
                "closed" => GitPullRequestFilter::Closed,
                "merged" => GitPullRequestFilter::Merged,
                "all" => GitPullRequestFilter::All,
                _ => return Err("invalid pull request filter".into()),
            },
            limit: number(args, "limit", 100)?,
            checks: args["checks"].as_bool().unwrap_or(true),
        }),
        "git.pr.create" => GitAction::PullRequest(GitPullRequestAction::Create {
            title: string("title")?,
            body: string("body")?,
            base_branch: args["baseBranch"].as_str().map(str::to_owned),
            draft: boolean(args, "draft"),
        }),
        "git.pr.merge" => GitAction::PullRequest(GitPullRequestAction::Merge {
            number: number64("number")?,
            method: match args["method"].as_str().unwrap_or("merge") {
                "merge" => GitMergeMethod::Merge,
                "squash" => GitMergeMethod::Squash,
                "rebase" => GitMergeMethod::Rebase,
                _ => return Err("invalid merge method".into()),
            },
            delete_branch: args["deleteBranch"].as_bool().unwrap_or(true),
            expected_head: None,
        }),
        "git.pr.close" => GitAction::PullRequest(GitPullRequestAction::Close {
            number: number64("number")?,
        }),
        "git.pr.checkout" => GitAction::PullRequest(GitPullRequestAction::Checkout {
            number: number64("number")?,
        }),
        _ => return Err(format!("unsupported API: {verb}")),
    })
}

fn file_status(status: &GitFileStatus, staged: bool) -> Result<Value, String> {
    let file = &status.file;
    let stat = if staged {
        &status.staged
    } else {
        &status.unstaged
    };
    Ok(
        json!({"path":path_text(&file.path)?,"oldPath":file.original_path.as_ref().map(path_text).transpose()?,
        "status":char::from(if staged {file.index} else {file.worktree}).to_string(),
        "isStaged":file.staged(),"isUnstaged":file.unstaged() || file.untracked(),"isBinary":stat.binary,
        "additions":stat.additions,"deletions":stat.deletions}),
    )
}

fn checks(value: &GitChecks) -> Value {
    let total = value.passing + value.failing + value.pending;
    json!({"passing":value.passing,"failing":value.failing,"pending":value.pending,"total":total,
        "status":if value.failing>0 {"failure"} else if value.pending>0 {"pending"} else if total>0 {"success"} else {"none"}})
}

fn pull_request(value: &GitPullRequest) -> Value {
    json!({"number":value.number,"url":value.url,"title":value.title,"author":value.author,
        "headBranch":value.head_branch,"headOid":value.head_oid,"baseBranch":value.base_branch,"state":value.state,
        "isDraft":value.draft,"updatedAt":value.updated_at,"mergeable":value.mergeable,
        "mergeStateStatus":value.merge_state,"isCrossRepository":value.cross_repository,"checks":checks(&value.checks)})
}

pub fn git_reply(verb: &str, reply: GitReply) -> Result<Value, String> {
    Ok(match reply {
        GitReply::Summary(summary) => if verb == "git.currentBranch" { json!(summary.and_then(|s| s.branch)) } else {
            let s=summary.unwrap_or_default();json!({"ahead":s.ahead,"behind":s.behind,"hasUpstream":s.upstream.is_some()})
        },
        GitReply::Status(status) => json!({"branch":status.summary.branch,"aheadBehind":{"ahead":status.summary.ahead,"behind":status.summary.behind,"hasUpstream":status.summary.upstream.is_some()},
            "defaultBranch":status.default_branch,"branches":status.branches.iter().map(|b| &b.name).collect::<Vec<_>>(),
            "stagedFiles":status.files.iter().filter(|f| f.file.staged()).map(|f| file_status(f,true)).collect::<Result<Vec<_>,_>>()?,
            "unstagedFiles":status.files.iter().filter(|f| f.file.unstaged() || f.file.untracked()).map(|f| file_status(f,false)).collect::<Result<Vec<_>,_>>()?,
            "pullRequest":status.pull_request.as_ref().map(pull_request)}),
        GitReply::Branches(branches) => json!(branches.iter().map(|b| &b.name).collect::<Vec<_>>()),
        GitReply::RemoteBranches(branches) => json!(branches),
        GitReply::RepoInfo(info) => json!({"root":path_text(&info.root)?,"gitDir":path_text(&info.git_dir)?,"isWorktree":info.is_worktree,"currentBranch":info.current_branch}),
        GitReply::Log(commits) => json!(commits.iter().map(|c| json!({"hash":c.hash,"shortHash":c.short_hash,"subject":c.subject,"authorName":c.author_name,"authorDate":c.author_date,"isMerge":c.parent_hashes.len()>1,"parentHashes":c.parent_hashes,"refs":c.refs.iter().map(|r| json!({"name":r.name,"kind":match r.kind {GitRefKind::LocalBranch=>"localBranch",GitRefKind::RemoteBranch=>"remoteBranch",GitRefKind::Tag=>"tag",GitRefKind::Head=>"head"}})).collect::<Vec<_>>()})).collect::<Vec<_>>()),
        GitReply::RawDiff(diff) => json!({"diff":diff.diff,"truncated":diff.truncated}),
        GitReply::Diff(diff) => json!({"additions":diff.additions,"deletions":diff.deletions,"truncated":diff.truncated,
            "rows":diff.rows.iter().map(|r| json!({"kind":match r.kind {GitDiffKind::Hunk=>"hunk",GitDiffKind::Context=>"context",GitDiffKind::Addition=>"addition",GitDiffKind::Deletion=>"deletion"},"oldLineNumber":r.old_line_number,"newLineNumber":r.new_line_number,"oldText":r.old_text,"newText":r.new_text,"text":r.text})).collect::<Vec<_>>()}),
        GitReply::Worktrees(trees) => Value::Array(trees.iter().map(|w| Ok(json!({"path":path_text(&w.directory)?,"branch":w.branch,"head":w.head,"isBare":w.bare,"isDetached":w.detached,"isPrunable":w.prunable}))).collect::<Result<_,String>>()?),
        GitReply::Commit(hash) => json!({"hash":hash}),
        GitReply::PullRequest(pr) => pr.as_deref().map_or(Value::Null,pull_request),
        GitReply::PullRequestNumber(number) => json!(number),
        GitReply::PullRequests(prs) => json!(prs.iter().map(pull_request).collect::<Vec<_>>()),
        GitReply::Done => Value::Null,
        _ => return Err("unexpected Git response".into()),
    })
}
