use super::*;
use muxy_protocol::{
    GitBranch, GitChecks, GitPreviewFile, GitPullRequest, GitRawDiff, GitStatus, GitSummary,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[derive(Default)]
struct FakeGit {
    replies: Mutex<VecDeque<Result<GitReply, String>>>,
    actions: Mutex<Vec<GitAction>>,
    files: HashMap<&'static str, &'static str>,
}

impl FakeGit {
    fn new(replies: Vec<Result<GitReply, String>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            ..Self::default()
        }
    }

    fn actions(&self) -> Vec<GitAction> {
        self.actions.lock().unwrap().clone()
    }
}

impl Git for FakeGit {
    fn call(&self, _: ProjectId, action: GitAction) -> Result<GitReply, String> {
        self.actions.lock().unwrap().push(action);
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("missing fake reply".into()))
    }

    fn read(&self, _: ProjectId, path: &str) -> Option<String> {
        self.files.get(path).map(|content| (*content).to_owned())
    }
}

fn status(branch: &str, conflicted: u32) -> GitReply {
    GitReply::Status(Box::new(GitStatus {
        summary: GitSummary {
            branch: Some(branch.into()),
            head: Some("abc".into()),
            changed: 1,
            conflicted,
            ..GitSummary::default()
        },
        default_branch: Some("main".into()),
        branches: vec![GitBranch {
            name: branch.into(),
            current: true,
            checked_out: true,
            default: branch == "main",
        }],
        files: vec![],
        pull_request: None,
    }))
}

fn preview_of(branch: &str, files: &[(&str, bool)], remote: bool) -> GitChangesPreview {
    GitChangesPreview {
        branch: Some(branch.into()),
        head: Some("abc".into()),
        tree: "tree1".into(),
        diff: GitRawDiff {
            diff: "diff --git a/file b/file\n+new line\n".into(),
            truncated: false,
        },
        files: files
            .iter()
            .map(|(path, untracked)| GitPreviewFile {
                path: ServerPath(path.as_bytes().to_vec()),
                added: Some(1),
                removed: Some(0),
                untracked: *untracked,
            })
            .collect(),
        destination: remote.then(|| GitPushDestination {
            remote: "origin".into(),
            branch: branch.into(),
        }),
    }
}

fn preview(branch: &str, files: &[(&str, bool)], remote: bool) -> GitReply {
    GitReply::ChangesPreview(Box::new(preview_of(branch, files, remote)))
}

fn log() -> GitReply {
    GitReply::Log(vec![])
}

fn pull_request(url: &str) -> GitReply {
    GitReply::PullRequest(Some(Box::new(GitPullRequest {
        number: 12,
        url: url.into(),
        title: "Fix status".into(),
        author: "user".into(),
        head_branch: "fix/status".into(),
        head_oid: "def".into(),
        base_branch: "main".into(),
        state: "OPEN".into(),
        draft: false,
        updated_at: None,
        mergeable: None,
        merge_state: "UNKNOWN".into(),
        cross_repository: false,
        checks: GitChecks::default(),
    })))
}

fn plan(action: Action, mode: Mode, branch: &str, files: &[(&str, bool)]) -> Plan {
    Plan {
        project: ProjectId::new(),
        action,
        mode,
        choices: CommitChoices::default(),
        branch: branch.into(),
        default_branch: Some("main".into()),
        preview: preview_of(branch, files, true),
        subjects: vec!["Fix: earlier change".into()],
        local_branches: vec!["main".into(), "feature".into()],
        remote_branches: vec!["main".into(), "release".into(), "feature".into()],
        branch_diff: None,
        template: None,
    }
}

fn pull_request_draft(branch: &str) -> Draft {
    Draft::PullRequest {
        title: "Fix status".into(),
        summary: "Summary".into(),
        branch: branch.into(),
        target: "main".into(),
    }
}

#[test]
fn branch_validation_rejects_remote_prefixes_and_git_ref_syntax() {
    for name in [
        "",
        "-bad",
        "/bad",
        "bad/",
        "bad..name",
        "bad.lock",
        "bad name",
        "origin/@{upstream}",
    ] {
        assert!(!valid_branch(name), "{name}");
    }
    assert!(valid_branch("feat/fix-status"));
}

#[test]
fn conflicts_and_stale_branches_are_refused_before_reading_changes() {
    let git = FakeGit::new(vec![Ok(status("feature", 1))]);
    let error = prepare(
        &git,
        ProjectId::new(),
        Action::Commit,
        CommitChoices::default(),
        "feature",
        Some("abc"),
    )
    .err()
    .unwrap();
    assert_eq!(error, "Resolve merge conflicts first");
    assert_eq!(git.actions().len(), 1);

    let git = FakeGit::new(vec![Ok(status("other", 0))]);
    let error = prepare(
        &git,
        ProjectId::new(),
        Action::Commit,
        CommitChoices::default(),
        "feature",
        Some("abc"),
    )
    .err()
    .unwrap();
    assert_eq!(error, CHANGED);
}

#[test]
fn commit_uses_the_reviewed_tree_then_pushes_to_its_destination() {
    let plan = plan(Action::Commit, Mode::Commit, "feature", &[("file", false)]);
    let git = FakeGit::new(vec![
        Ok(status("feature", 0)),
        Ok(GitReply::Commit("deadbeef00".into())),
        Ok(GitReply::Done),
    ]);
    let outcome = apply(
        &git,
        &plan,
        &Draft::Commit {
            message: "Explain the change".into(),
        },
    )
    .unwrap();
    assert_eq!(
        outcome,
        Outcome::Committed {
            hash: "deadbeef00".into(),
            branch: "feature".into(),
            pushed: plan.preview.destination.clone(),
        }
    );
    let actions = git.actions();
    assert_eq!(
        actions[1],
        GitAction::CommitAll {
            message: "Explain the change".into(),
            expected_head: Some("abc".into()),
            expected_tree: "tree1".into(),
        }
    );
    assert_eq!(
        actions[2],
        GitAction::PublishBranch {
            branch: "feature".into(),
            destination: GitPushDestination {
                remote: "origin".into(),
                branch: "feature".into(),
            },
        }
    );
}

const STAGED_ONLY: CommitChoices = CommitChoices {
    include_unstaged: false,
    push: false,
};

#[test]
fn staged_only_commits_leave_the_index_alone_and_skip_pushing() {
    let git = FakeGit::new(vec![
        Ok(status("feature", 0)),
        Ok(preview("feature", &[("staged", false)], true)),
        Ok(log()),
    ]);
    let plan = prepare(
        &git,
        ProjectId::new(),
        Action::Commit,
        STAGED_ONLY,
        "feature",
        Some("abc"),
    )
    .unwrap();
    assert_eq!(
        git.actions()[1],
        GitAction::StagedPreview {
            line_limit: Some(DIFF_LINES),
        }
    );

    let git = FakeGit::new(vec![
        Ok(status("feature", 0)),
        Ok(GitReply::Commit("deadbeef00".into())),
    ]);
    let outcome = apply(
        &git,
        &plan,
        &Draft::Commit {
            message: "Explain the change".into(),
        },
    )
    .unwrap();
    assert_eq!(
        outcome,
        Outcome::Committed {
            hash: "deadbeef00".into(),
            branch: "feature".into(),
            pushed: None,
        }
    );
    assert_eq!(
        git.actions()[1..],
        [GitAction::CommitStaged {
            message: "Explain the change".into(),
            expected_head: Some("abc".into()),
            expected_tree: "tree1".into(),
        }]
    );
}

#[test]
fn staged_only_commits_need_staged_changes() {
    let git = FakeGit::new(vec![
        Ok(status("feature", 0)),
        Ok(preview("feature", &[], true)),
    ]);
    let error = prepare(
        &git,
        ProjectId::new(),
        Action::Commit,
        STAGED_ONLY,
        "feature",
        Some("abc"),
    )
    .err()
    .unwrap();
    assert_eq!(
        error,
        "Nothing is staged. Stage changes first or include unstaged changes."
    );
    assert!(
        !git.actions()
            .iter()
            .any(|action| matches!(action, GitAction::Stage(_)))
    );
}

#[test]
fn repository_changes_before_apply_are_refused_without_mutating() {
    let plan = plan(Action::Commit, Mode::Commit, "feature", &[("file", false)]);
    let git = FakeGit::new(vec![Ok(GitReply::Status(Box::new(GitStatus {
        summary: GitSummary {
            branch: Some("feature".into()),
            head: Some("moved".into()),
            ..GitSummary::default()
        },
        default_branch: None,
        branches: vec![],
        files: vec![],
        pull_request: None,
    })))]);
    let failure = apply(
        &git,
        &plan,
        &Draft::Commit {
            message: "Change".into(),
        },
    )
    .unwrap_err();
    assert_eq!(failure.title, "No commit was created");
    assert_eq!(git.actions().len(), 1);
}

#[test]
fn pull_requests_from_the_default_branch_move_changes_to_a_new_branch() {
    let git = FakeGit {
        files: HashMap::from([(".github/pull_request_template.md", "## Summary")]),
        ..FakeGit::new(vec![
            Ok(status("main", 0)),
            Ok(GitReply::PullRequest(None)),
            Ok(GitReply::Done),
            Ok(preview("main", &[("file", false)], true)),
            Ok(log()),
            Ok(GitReply::RemoteBranches(vec!["main".into()])),
            Ok(GitReply::RawDiff(GitRawDiff {
                diff: String::new(),
                truncated: false,
            })),
        ])
    };
    let plan = prepare(
        &git,
        ProjectId::new(),
        Action::CreatePullRequest,
        CommitChoices::default(),
        "main",
        Some("abc"),
    )
    .unwrap();
    assert_eq!(plan.mode, Mode::NewBranch);
    assert_eq!(plan.template.as_deref(), Some("## Summary"));
    let draft = parse_draft(
        &plan,
        r#"{"title":"Fix status","summary":"Summary","newBranchName":"fix/status","targetBranchName":"main"}"#,
    )
    .unwrap();
    assert_eq!(
        draft,
        Draft::PullRequest {
            title: "Fix status".into(),
            summary: "Summary".into(),
            branch: "fix/status".into(),
            target: "main".into(),
        }
    );

    let git = FakeGit::new(vec![
        Ok(status("main", 0)),
        Ok(GitReply::Done),
        Ok(GitReply::Commit("def0000".into())),
        Ok(GitReply::Done),
        Ok(pull_request("https://github.com/example/repo/pull/12")),
    ]);
    let outcome = apply(&git, &plan, &draft).unwrap();
    assert_eq!(
        outcome,
        Outcome::PullRequest("https://github.com/example/repo/pull/12".into())
    );
    let actions = git.actions();
    assert_eq!(actions[1], GitAction::CreateBranch("fix/status".into()));
    assert!(
        matches!(actions[2], GitAction::CommitAll { ref message, .. } if message == "Fix status")
    );
    assert_eq!(
        actions[3],
        GitAction::PublishBranch {
            branch: "fix/status".into(),
            destination: GitPushDestination {
                remote: "origin".into(),
                branch: "fix/status".into(),
            },
        }
    );
    assert!(matches!(
        &actions[4],
        GitAction::PullRequest(GitPullRequestAction::Create { base_branch: Some(base), .. }) if base == "main"
    ));
}

#[test]
fn reviewed_drafts_are_validated_before_any_change() {
    let plan = plan(
        Action::CreatePullRequest,
        Mode::NewBranch,
        "main",
        &[("file", false)],
    );
    for (draft, error) in [
        (
            pull_request_draft("feature"),
            "Choose a new branch name that isn't used yet",
        ),
        (
            pull_request_draft("bad name"),
            "Choose a new branch name that isn't used yet",
        ),
        (
            Draft::PullRequest {
                title: "Fix status".into(),
                summary: "Summary".into(),
                branch: "fix-x".into(),
                target: "missing".into(),
            },
            "Choose a target branch that exists on the remote",
        ),
        (
            Draft::PullRequest {
                title: " ".into(),
                summary: "Summary".into(),
                branch: "fix-x".into(),
                target: "main".into(),
            },
            "Enter a title of up to 256 characters",
        ),
    ] {
        let git = FakeGit::new(vec![]);
        let failure = apply(&git, &plan, &draft).unwrap_err();
        assert_eq!(
            failure.detail,
            format!("{error}\n\nChanges were staged before generating AI metadata.")
        );
        assert!(git.actions().is_empty());
    }
}
