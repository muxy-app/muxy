use super::*;
use crate::fake::{Fake, wait};
use muxy_protocol::{ErrorCode, ErrorReply, GitChecks};

fn path(text: &str) -> ServerPath {
    ServerPath(text.as_bytes().to_vec())
}

/// A reply of another kind than `reply`.
fn other_than(reply: &GitReply) -> GitReply {
    if *reply == GitReply::Done {
        GitReply::Log(Vec::new())
    } else {
        GitReply::Done
    }
}

/// Runs a typed method and its async twin against `$reply`, then checks
/// that both refuse a reply of another kind.
macro_rules! case {
    ($fake:expr, $project:expr, $name:ident, $name_async:ident($($arg:expr),*),
        $action:expr, $reply:expr, $expected:expr) => {{
        let (fake, project): (&Fake, ProjectId) = ($fake, $project);
        let sent = RequestBody::Git(GitRequest { project, action: $action });
        let (request, result) =
            fake.answer(ReplyBody::Git($reply), |client| client.$name(project, $($arg),*));
        assert_eq!(request, sent, stringify!($name));
        assert_eq!(result.expect(stringify!($name)), $expected, stringify!($name));
        let (request, result) = fake.answer(ReplyBody::Git($reply), |client| {
            wait(client.$name_async(project, $($arg),*))
        });
        assert_eq!(request, sent, stringify!($name_async));
        assert_eq!(result.expect(stringify!($name_async)), $expected, stringify!($name_async));
        let (_, result) = fake.answer(ReplyBody::Git(other_than(&$reply)), |client| {
            client.$name(project, $($arg),*)
        });
        assert!(
            matches!(result, Err(ClientError::UnexpectedReply(_))),
            "{} accepted another reply",
            stringify!($name)
        );
        let (_, result) = fake.answer(ReplyBody::Detached, |client| {
            wait(client.$name_async(project, $($arg),*))
        });
        assert!(
            matches!(result, Err(ClientError::UnexpectedReply(_))),
            "{} accepted another reply",
            stringify!($name_async)
        );
    }};
}

fn pull_request() -> GitPullRequest {
    GitPullRequest {
        number: 7,
        url: "https://example.com/pr/7".into(),
        title: "Title".into(),
        author: "author".into(),
        head_branch: "feature".into(),
        head_oid: "abc".into(),
        base_branch: "main".into(),
        state: "OPEN".into(),
        draft: false,
        updated_at: None,
        mergeable: Some(true),
        merge_state: "CLEAN".into(),
        cross_repository: false,
        checks: GitChecks::default(),
    }
}

fn raw() -> GitRawDiff {
    GitRawDiff {
        diff: "diff".into(),
        truncated: false,
    }
}

fn preview() -> GitChangesPreview {
    GitChangesPreview {
        branch: Some("main".into()),
        head: Some("abc".into()),
        tree: "def".into(),
        diff: raw(),
        files: Vec::new(),
        destination: None,
    }
}

fn removal() -> WorktreeRemoval {
    WorktreeRemoval {
        directory: path("/worktree"),
        device: 1,
        inode: 2,
        dirty: false,
        status: Vec::new(),
        head: None,
        branch: Some("feature".into()),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One case per Git action, read as a table"
)]
fn each_git_method_sends_its_action_and_accepts_only_its_reply() {
    let fake = Fake::start();
    let project = ProjectId::new();
    let fake = &fake;
    let summary = GitSummary {
        branch: Some("main".into()),
        ..GitSummary::default()
    };
    case!(
        fake,
        project,
        git_summary,
        git_summary_async(),
        GitAction::Summary,
        GitReply::Summary(Some(summary.clone())),
        Some(summary.clone())
    );
    case!(
        fake,
        project,
        git_summary,
        git_summary_async(),
        GitAction::Summary,
        GitReply::Summary(None),
        None
    );
    let branch = GitBranch {
        name: "main".into(),
        current: true,
        checked_out: true,
        default: true,
    };
    case!(
        fake,
        project,
        git_branches,
        git_branches_async(),
        GitAction::Branches,
        GitReply::Branches(vec![branch.clone()]),
        vec![branch.clone()]
    );
    let file = GitFile {
        path: path("a"),
        original_path: None,
        index: b'M',
        worktree: b' ',
        added: Some(1),
        removed: Some(0),
    };
    case!(
        fake,
        project,
        git_changes,
        git_changes_async(),
        GitAction::Changes,
        GitReply::Changes(vec![file.clone()]),
        vec![file.clone()]
    );
    case!(
        fake,
        project,
        git_switch_branch,
        git_switch_branch_async("b".into()),
        GitAction::SwitchBranch("b".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_create_branch,
        git_create_branch_async("b".into()),
        GitAction::CreateBranch("b".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_delete_branch,
        git_delete_branch_async("b".into()),
        GitAction::DeleteBranch("b".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_stage,
        git_stage_async(vec![path("a")]),
        GitAction::Stage(vec![path("a")]),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_unstage,
        git_unstage_async(vec![path("a")]),
        GitAction::Unstage(vec![path("a")]),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_discard,
        git_discard_async(vec![path("a")]),
        GitAction::Discard(vec![path("a")]),
        GitReply::Done,
        ()
    );
    let worktree = GitWorktree {
        directory: path("/worktree"),
        head: None,
        branch: Some("feature".into()),
        primary: false,
        locked: false,
        bare: false,
        detached: false,
        prunable: false,
        registered: None,
    };
    case!(
        fake,
        project,
        git_worktrees,
        git_worktrees_async(),
        GitAction::Worktrees,
        GitReply::Worktrees(vec![worktree.clone()]),
        vec![worktree.clone()]
    );
    case!(
        fake,
        project,
        git_inspect_removal,
        git_inspect_removal_async(),
        GitAction::InspectRemoval,
        GitReply::Removal(removal()),
        removal()
    );
    let intent = WorktreeIntent {
        operation: OperationId::new(),
        action: WorktreeAction::Register {
            project: ProjectId::new(),
            directory: path("/worktree"),
        },
        options: None,
    };
    let added = ProjectDescriptor::new(path("/worktree"), []);
    case!(
        fake,
        project,
        git_add_worktree,
        git_add_worktree_async(intent.clone()),
        GitAction::Worktree(intent.clone()),
        GitReply::Project(added.clone()),
        AddedWorktree {
            project: added.clone(),
            setup_failure: None
        }
    );
    case!(
        fake,
        project,
        git_add_worktree,
        git_add_worktree_async(intent.clone()),
        GitAction::Worktree(intent.clone()),
        GitReply::WorktreeSetupFailed {
            project: added.clone(),
            message: "hook".into()
        },
        AddedWorktree {
            project: added.clone(),
            setup_failure: Some("hook".into())
        }
    );
    case!(
        fake,
        project,
        git_watch,
        git_watch_async(),
        GitAction::Watch,
        GitReply::Done,
        ()
    );
    let status = GitStatus {
        summary: summary.clone(),
        default_branch: Some("main".into()),
        branches: vec![branch],
        files: Vec::new(),
        pull_request: Some(pull_request()),
    };
    case!(
        fake,
        project,
        git_status,
        git_status_async(true, false),
        GitAction::Status {
            local: true,
            fresh: false
        },
        GitReply::Status(Box::new(status.clone())),
        status.clone()
    );
    let info = GitRepoInfo {
        root: path("/repository"),
        git_dir: path("/repository/.git"),
        is_worktree: false,
        current_branch: Some("main".into()),
    };
    case!(
        fake,
        project,
        git_repo_info,
        git_repo_info_async(),
        GitAction::RepoInfo,
        GitReply::RepoInfo(info.clone()),
        info.clone()
    );
    case!(
        fake,
        project,
        git_remote_branches,
        git_remote_branches_async(),
        GitAction::RemoteBranches,
        GitReply::RemoteBranches(vec!["main".into()]),
        vec!["main".to_owned()]
    );
    let commit = GitCommit {
        hash: "abc".into(),
        short_hash: "ab".into(),
        subject: "Subject".into(),
        author_name: "author".into(),
        author_date: "today".into(),
        parent_hashes: Vec::new(),
        refs: Vec::new(),
    };
    case!(
        fake,
        project,
        git_log,
        git_log_async(10, 5),
        GitAction::Log {
            max_count: 10,
            skip: 5
        },
        GitReply::Log(vec![commit.clone()]),
        vec![commit.clone()]
    );
    let diff = GitDiff {
        rows: Vec::new(),
        additions: 1,
        deletions: 2,
        truncated: false,
        binary: false,
    };
    case!(
        fake,
        project,
        git_diff,
        git_diff_async(path("a"), true, Some(9)),
        GitAction::Diff(GitDiffRequest {
            path: Some(path("a")),
            staged: true,
            raw: false,
            line_limit: Some(9),
        }),
        GitReply::Diff(diff.clone()),
        diff.clone()
    );
    case!(
        fake,
        project,
        git_raw_diff,
        git_raw_diff_async(None, false, None),
        GitAction::Diff(GitDiffRequest {
            path: None,
            staged: false,
            raw: true,
            line_limit: None
        }),
        GitReply::RawDiff(raw()),
        raw()
    );
    case!(
        fake,
        project,
        git_init,
        git_init_async(),
        GitAction::Init,
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_commit,
        git_commit_async("m".into(), true),
        GitAction::Commit {
            message: "m".into(),
            stage_all: true
        },
        GitReply::Commit("abc".into()),
        "abc".to_owned()
    );
    case!(
        fake,
        project,
        git_push,
        git_push_async(true),
        GitAction::Push { set_upstream: true },
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_pull,
        git_pull_async(),
        GitAction::Pull,
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_checkout,
        git_checkout_async("abc".into()),
        GitAction::Checkout("abc".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_cherry_pick,
        git_cherry_pick_async("abc".into()),
        GitAction::CherryPick("abc".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_revert,
        git_revert_async("abc".into()),
        GitAction::Revert("abc".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_delete_local_branch,
        git_delete_local_branch_async("b".into(), true),
        GitAction::DeleteLocalBranch {
            name: "b".into(),
            force: true
        },
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_delete_remote_branch,
        git_delete_remote_branch_async("b".into()),
        GitAction::DeleteRemoteBranch("b".into()),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_create_tag,
        git_create_tag_async("v1".into(), "abc".into()),
        GitAction::CreateTag {
            name: "v1".into(),
            hash: "abc".into()
        },
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_pull_request,
        git_pull_request_async(),
        GitAction::PullRequest(GitPullRequestAction::Info),
        GitReply::PullRequest(Some(Box::new(pull_request()))),
        Some(pull_request())
    );
    case!(
        fake,
        project,
        git_pull_request,
        git_pull_request_async(),
        GitAction::PullRequest(GitPullRequestAction::Info),
        GitReply::PullRequest(None),
        None
    );
    case!(
        fake,
        project,
        git_pull_request_number,
        git_pull_request_number_async(),
        GitAction::PullRequest(GitPullRequestAction::Number),
        GitReply::PullRequestNumber(Some(7)),
        Some(7)
    );
    case!(
        fake,
        project,
        git_pull_request_diff,
        git_pull_request_diff_async(7, Some(3)),
        GitAction::PullRequest(GitPullRequestAction::Diff {
            number: 7,
            line_limit: Some(3)
        }),
        GitReply::RawDiff(raw()),
        raw()
    );
    case!(
        fake,
        project,
        git_pull_requests,
        git_pull_requests_async(GitPullRequestFilter::Open, 20, true),
        GitAction::PullRequest(GitPullRequestAction::List {
            filter: GitPullRequestFilter::Open,
            limit: 20,
            checks: true,
        }),
        GitReply::PullRequests(vec![pull_request()]),
        vec![pull_request()]
    );
    case!(
        fake,
        project,
        git_create_pull_request,
        git_create_pull_request_async("t".into(), "b".into(), Some("main".into()), true),
        GitAction::PullRequest(GitPullRequestAction::Create {
            title: "t".into(),
            body: "b".into(),
            base_branch: Some("main".into()),
            draft: true,
        }),
        GitReply::PullRequest(Some(Box::new(pull_request()))),
        pull_request()
    );
    case!(
        fake,
        project,
        git_merge_pull_request,
        git_merge_pull_request_async(7, GitMergeMethod::Squash, true, Some("abc".into())),
        GitAction::PullRequest(GitPullRequestAction::Merge {
            number: 7,
            method: GitMergeMethod::Squash,
            delete_branch: true,
            expected_head: Some("abc".into()),
        }),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_close_pull_request,
        git_close_pull_request_async(7),
        GitAction::PullRequest(GitPullRequestAction::Close { number: 7 }),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_checkout_pull_request,
        git_checkout_pull_request_async(7),
        GitAction::PullRequest(GitPullRequestAction::Checkout { number: 7 }),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_update_pull_request_branch,
        git_update_pull_request_branch_async(7, "abc".into()),
        GitAction::PullRequest(GitPullRequestAction::UpdateBranch {
            number: 7,
            expected_head: "abc".into(),
        }),
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_branch_diff,
        git_branch_diff_async("main".into(), Some(4)),
        GitAction::BranchDiff {
            base: "main".into(),
            line_limit: Some(4)
        },
        GitReply::RawDiff(raw()),
        raw()
    );
    case!(
        fake,
        project,
        git_changes_preview,
        git_changes_preview_async(Some(4)),
        GitAction::ChangesPreview {
            line_limit: Some(4)
        },
        GitReply::ChangesPreview(Box::new(preview())),
        preview()
    );
    case!(
        fake,
        project,
        git_commit_all,
        git_commit_all_async("m".into(), Some("abc".into()), "def".into()),
        GitAction::CommitAll {
            message: "m".into(),
            expected_head: Some("abc".into()),
            expected_tree: "def".into(),
        },
        GitReply::Commit("123".into()),
        "123".to_owned()
    );
    let destination = GitPushDestination {
        remote: "origin".into(),
        branch: "feature".into(),
    };
    case!(
        fake,
        project,
        git_publish_branch,
        git_publish_branch_async("feature".into(), destination.clone()),
        GitAction::PublishBranch {
            branch: "feature".into(),
            destination: destination.clone()
        },
        GitReply::Done,
        ()
    );
    case!(
        fake,
        project,
        git_switch_to_base,
        git_switch_to_base_async("main".into()),
        GitAction::SwitchToBase("main".into()),
        GitReply::BaseSwitch(GitBaseSwitch::Updated),
        GitBaseSwitch::Updated
    );
    let hooks = vec![WorktreeHook {
        command: "true".into(),
        name: None,
        project: true,
    }];
    case!(
        fake,
        project,
        git_worktree_hooks,
        git_worktree_hooks_async(true),
        GitAction::WorktreeHooks { teardown: true },
        GitReply::WorktreeHooks(hooks.clone()),
        hooks.clone()
    );
    case!(
        fake,
        project,
        git_staged_preview,
        git_staged_preview_async(None),
        GitAction::StagedPreview { line_limit: None },
        GitReply::ChangesPreview(Box::new(preview())),
        preview()
    );
    case!(
        fake,
        project,
        git_commit_staged,
        git_commit_staged_async("m".into(), None, "def".into()),
        GitAction::CommitStaged {
            message: "m".into(),
            expected_head: None,
            expected_tree: "def".into()
        },
        GitReply::Commit("123".into()),
        "123".to_owned()
    );
}

#[test]
fn a_pull_request_that_was_not_created_is_unexpected() {
    let fake = Fake::start();
    let (_, created) = fake.answer(ReplyBody::Git(GitReply::PullRequest(None)), |client| {
        client.git_create_pull_request(ProjectId::new(), "t".into(), "b".into(), None, false)
    });
    assert!(matches!(created, Err(ClientError::UnexpectedReply(_))));
}

#[test]
fn removing_a_worktree_sends_a_new_operation_and_only_approved_hooks() {
    let fake = Fake::start();
    let project = ProjectId::new();
    let hooks = vec![WorktreeHook {
        command: "true".into(),
        name: None,
        project: true,
    }];
    for approved in [None, Some(hooks)] {
        let (request, result) = fake.answer(ReplyBody::Git(GitReply::Done), |client| {
            client.git_remove_worktree(project, removal(), approved.clone())
        });
        result.expect("removed");
        let RequestBody::Git(GitRequest {
            project: sent,
            action: GitAction::Worktree(intent),
        }) = request
        else {
            panic!("expected a worktree request, got {request:?}");
        };
        assert_eq!(sent, project);
        assert_eq!(
            intent.action,
            WorktreeAction::Remove {
                expected: removal()
            }
        );
        assert_eq!(
            intent.options,
            approved.map(|hooks| WorktreeOptions {
                name: None,
                hooks: Some(hooks),
            })
        );
    }
}

#[test]
fn server_errors_reach_typed_callers_unchanged() {
    let fake = Fake::start();
    let error = ErrorReply {
        code: ErrorCode::BadRequest,
        message: "gone".into(),
    };
    let (_, sync) = fake.answer(ReplyBody::Error(error.clone()), |client| {
        client.git_summary(ProjectId::new())
    });
    assert!(matches!(sync, Err(ClientError::Server(reply)) if reply == error));
    let (_, async_) = fake.answer(ReplyBody::Error(error.clone()), |client| {
        wait(client.git_summary_async(ProjectId::new()))
    });
    assert!(matches!(async_, Err(ClientError::Server(reply)) if reply == error));
}

#[test]
fn worktree_request_budget_covers_git_hooks_and_reply_headroom() {
    let normal = git_timeout(&GitAction::Summary);
    let mut intent = WorktreeIntent {
        operation: OperationId::new(),
        action: WorktreeAction::Create {
            project: ProjectId::new(),
            directory: ServerPath(b"/tmp/worktree".to_vec()),
            branch: "feature".into(),
            base: Some("main".into()),
        },
        options: None,
    };
    assert_eq!(git_timeout(&GitAction::Worktree(intent.clone())), normal);
    for hooks in [None, Some(vec![])] {
        intent.options = Some(WorktreeOptions { name: None, hooks });
        assert_eq!(git_timeout(&GitAction::Worktree(intent.clone())), normal);
    }
    intent.options.as_mut().expect("options").hooks = Some(vec![WorktreeHook {
        command: "sleep 295".into(),
        name: None,
        project: true,
    }]);
    let budget = git_timeout(&GitAction::Worktree(intent));
    assert!(budget > normal + Duration::from_secs(300));
    assert!(budget > Duration::from_secs(10 + 295));
}
