use muxy_protocol::{
    CONTROL, GitAction, GitDiffRequest, GitMergeMethod, GitPullRequestAction as Pr,
    GitPullRequestFilter, GitRequest, Message, OperationId, ProjectId, RequestBody, RequestId,
    ServerPath, WorktreeAction, WorktreeIntent,
    wire::{Decoder, encode},
};

#[test]
fn extension_operations_round_trip_through_the_control_wire()
-> Result<(), Box<dyn std::error::Error>> {
    let actions = vec![
        GitAction::Status { local: true },
        GitAction::RepoInfo,
        GitAction::RemoteBranches,
        GitAction::Log {
            max_count: 100,
            skip: 50,
        },
        GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"raw-\xff\n".to_vec())),
            staged: true,
            raw: false,
            line_limit: Some(100),
        }),
        GitAction::Init,
        GitAction::Commit {
            message: "Message\n\nBody".into(),
            stage_all: true,
        },
        GitAction::Push { set_upstream: true },
        GitAction::Pull,
        GitAction::Checkout("abcd1234".into()),
        GitAction::CherryPick("abcd1234".into()),
        GitAction::Revert("abcd1234".into()),
        GitAction::DeleteLocalBranch {
            name: "feature".into(),
            force: false,
        },
        GitAction::DeleteRemoteBranch("feature".into()),
        GitAction::CreateTag {
            name: "v1".into(),
            hash: "abcd1234".into(),
        },
        GitAction::Stage(vec![]),
        GitAction::Unstage(vec![]),
        GitAction::Discard(vec![]),
        GitAction::PullRequest(Pr::Info),
        GitAction::PullRequest(Pr::Number),
        GitAction::PullRequest(Pr::List {
            filter: GitPullRequestFilter::All,
            limit: 100,
            checks: false,
        }),
        GitAction::PullRequest(Pr::Diff {
            number: 42,
            line_limit: Some(100),
        }),
        GitAction::PullRequest(Pr::Create {
            title: "Title".into(),
            body: "body".into(),
            base_branch: Some("main".into()),
            draft: true,
        }),
        GitAction::PullRequest(Pr::Merge {
            number: 42,
            method: GitMergeMethod::Squash,
            delete_branch: true,
        }),
        GitAction::PullRequest(Pr::Close { number: 42 }),
        GitAction::PullRequest(Pr::Checkout { number: 42 }),
        GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            action: WorktreeAction::CheckoutPullRequest {
                project: ProjectId::new(),
                directory: ServerPath(b"/tmp/pr".to_vec()),
                number: 42,
            },
        }),
    ];
    for action in actions {
        let request = GitRequest {
            project: ProjectId::new(),
            action,
        };
        assert_eq!(request.validate(), Ok(()));
        let message = Message::Request {
            id: RequestId(1),
            body: RequestBody::Git(request),
        };
        let mut bytes = Vec::new();
        encode(&message, CONTROL, &mut bytes)?;
        assert_eq!(Decoder::new(bytes.as_slice()).next()?, (CONTROL, message));
    }
    Ok(())
}

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
