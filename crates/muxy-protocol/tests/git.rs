use muxy_protocol::{
    GitAction, GitDiff, GitDiffRequest, GitMergeMethod, GitPullRequestAction as Pr,
    GitPullRequestFilter, GitPushDestination, GitRequest, ProjectId, ServerPath,
};

#[test]
fn extension_requests_reject_missing_values_option_injection_and_excessive_limits() {
    let mut actions = vec![
        GitAction::Commit {
            message: " \n".into(),
            stage_all: true,
        },
        GitAction::Commit {
            message: "x".repeat(65537),
            stage_all: false,
        },
        GitAction::Log {
            max_count: 1001,
            skip: 0,
        },
        GitAction::Diff(GitDiffRequest::default()),
        GitAction::Diff(GitDiffRequest {
            raw: true,
            line_limit: Some(100_001),
            ..GitDiffRequest::default()
        }),
        GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(vec![0])),
            ..GitDiffRequest::default()
        }),
        GitAction::Checkout("--orphan".into()),
        GitAction::CherryPick("HEAD..main".into()),
        GitAction::Revert(String::new()),
        GitAction::DeleteLocalBranch {
            name: "--all".into(),
            force: true,
        },
        GitAction::DeleteRemoteBranch("--all".into()),
        GitAction::CreateTag {
            name: "--delete".into(),
            hash: "abcd".into(),
        },
        GitAction::PullRequest(Pr::Close { number: 0 }),
        GitAction::PullRequest(Pr::List {
            filter: GitPullRequestFilter::All,
            limit: 201,
            checks: true,
        }),
        GitAction::PullRequest(Pr::Diff {
            number: 42,
            line_limit: Some(0),
        }),
        GitAction::PullRequest(Pr::Create {
            title: "Title".into(),
            body: "\0".into(),
            base_branch: None,
            draft: false,
        }),
        GitAction::PullRequest(Pr::Merge {
            number: 42,
            method: GitMergeMethod::Merge,
            delete_branch: false,
            expected_head: Some("HEAD~1".into()),
        }),
        GitAction::ChangesPreview {
            line_limit: Some(0),
        },
        GitAction::CommitAll {
            message: "Message".into(),
            expected_head: None,
            expected_tree: "--amend".into(),
        },
        GitAction::CommitAll {
            message: " ".into(),
            expected_head: None,
            expected_tree: "abc123".into(),
        },
        GitAction::PublishBranch {
            branch: "feature".into(),
            destination: GitPushDestination {
                remote: "--mirror".into(),
                branch: "feature".into(),
            },
        },
        GitAction::SwitchToBase("--detach".into()),
    ];
    actions.push(GitAction::Stage(vec![ServerPath(b"file".to_vec()); 4097]));
    for action in actions {
        assert!(
            GitRequest {
                project: ProjectId::new(),
                action
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn staged_review_requests_reject_bad_limits_heads_and_messages() {
    for action in [
        GitAction::StagedPreview {
            line_limit: Some(0),
        },
        GitAction::CommitStaged {
            message: "Message".into(),
            expected_head: Some("HEAD~1".into()),
            expected_tree: "abc123".into(),
        },
        GitAction::CommitStaged {
            message: "\0".into(),
            expected_head: None,
            expected_tree: "abc123".into(),
        },
    ] {
        assert!(
            GitRequest {
                project: ProjectId::new(),
                action
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn diffs_from_builds_before_the_binary_flag_read_as_text() -> Result<(), Box<dyn std::error::Error>>
{
    // [rows, additions, deletions, truncated], as written before the flag existed.
    let mut older = Vec::new();
    minicbor::Encoder::new(&mut older)
        .array(4)?
        .array(0)?
        .u64(1)?
        .u64(2)?
        .bool(false)?;
    let diff: GitDiff = minicbor::decode(&older)?;
    assert_eq!((diff.additions, diff.deletions, diff.binary), (1, 2, false));
    Ok(())
}
