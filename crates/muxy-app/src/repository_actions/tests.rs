use super::*;
use muxy_protocol::{
    GitAction, GitBranch, GitChecks, GitPreviewFile, GitPullRequestAction, GitReply, GitSummary,
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

impl FakeGit {
    /// Records `action` and decodes the next scripted reply.
    fn answer<T>(
        &self,
        action: GitAction,
        decode: impl FnOnce(GitReply) -> Option<T>,
    ) -> Result<T, String> {
        self.actions.lock().unwrap().push(action);
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err("missing fake reply".into()))?;
        decode(reply).ok_or_else(|| "unexpected fake reply".into())
    }
}

fn previewed(reply: GitReply) -> Option<GitChangesPreview> {
    match reply {
        GitReply::ChangesPreview(preview) => Some(*preview),
        _ => None,
    }
}

fn committed(reply: GitReply) -> Option<String> {
    match reply {
        GitReply::Commit(hash) => Some(hash),
        _ => None,
    }
}

impl Git for FakeGit {
    fn status(&self, _: ProjectId) -> Result<GitStatus, String> {
        let action = GitAction::Status {
            local: true,
            fresh: false,
        };
        self.answer(action, |reply| match reply {
            GitReply::Status(status) => Some(*status),
            _ => None,
        })
    }

    fn pull_request(&self, _: ProjectId) -> Result<Option<GitPullRequest>, String> {
        let action = GitAction::PullRequest(GitPullRequestAction::Info);
        self.answer(action, |reply| match reply {
            GitReply::PullRequest(found) => Some(found.map(|pr| *pr)),
            _ => None,
        })
    }

    fn stage_all(&self, _: ProjectId) -> Result<(), String> {
        self.answer(GitAction::Stage(vec![]), |reply| {
            (reply == GitReply::Done).then_some(())
        })
    }

    fn changes_preview(
        &self,
        _: ProjectId,
        line_limit: Option<u32>,
    ) -> Result<GitChangesPreview, String> {
        self.answer(GitAction::ChangesPreview { line_limit }, previewed)
    }

    fn staged_preview(
        &self,
        _: ProjectId,
        line_limit: Option<u32>,
    ) -> Result<GitChangesPreview, String> {
        self.answer(GitAction::StagedPreview { line_limit }, previewed)
    }

    fn log(&self, _: ProjectId, max_count: u32) -> Result<Vec<GitCommit>, String> {
        let action = GitAction::Log { max_count, skip: 0 };
        self.answer(action, |reply| match reply {
            GitReply::Log(commits) => Some(commits),
            _ => None,
        })
    }

    fn remote_branches(&self, _: ProjectId) -> Result<Vec<String>, String> {
        self.answer(GitAction::RemoteBranches, |reply| match reply {
            GitReply::RemoteBranches(branches) => Some(branches),
            _ => None,
        })
    }

    fn branch_diff(
        &self,
        _: ProjectId,
        base: String,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, String> {
        self.answer(
            GitAction::BranchDiff { base, line_limit },
            |reply| match reply {
                GitReply::RawDiff(diff) => Some(diff),
                _ => None,
            },
        )
    }

    fn commit_all(&self, _: ProjectId, commit: Commit) -> Result<String, String> {
        let action = GitAction::CommitAll {
            message: commit.message,
            expected_head: commit.expected_head,
            expected_tree: commit.expected_tree,
        };
        self.answer(action, committed)
    }

    fn commit_staged(&self, _: ProjectId, commit: Commit) -> Result<String, String> {
        let action = GitAction::CommitStaged {
            message: commit.message,
            expected_head: commit.expected_head,
            expected_tree: commit.expected_tree,
        };
        self.answer(action, committed)
    }

    fn publish_branch(
        &self,
        _: ProjectId,
        branch: String,
        destination: GitPushDestination,
    ) -> Result<(), String> {
        self.answer(
            GitAction::PublishBranch {
                branch,
                destination,
            },
            |reply| (reply == GitReply::Done).then_some(()),
        )
    }

    fn create_branch(&self, _: ProjectId, name: String) -> Result<(), String> {
        self.answer(GitAction::CreateBranch(name), |reply| {
            (reply == GitReply::Done).then_some(())
        })
    }

    fn create_pull_request(
        &self,
        _: ProjectId,
        title: String,
        body: String,
        base_branch: String,
    ) -> Result<Option<GitPullRequest>, String> {
        let action = GitAction::PullRequest(GitPullRequestAction::Create {
            title,
            body,
            base_branch: Some(base_branch),
            draft: false,
        });
        self.answer(action, |reply| match reply {
            GitReply::PullRequest(Some(created)) => Some(Some(*created)),
            _ => Some(None),
        })
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
                path: ServerPath::from(*path),
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
        |_| (),
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
        |_| (),
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
        |_| (),
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
    let outcome = apply(&git, &plan, &draft, |_| ()).unwrap();
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
        let failure = apply(&git, &plan, &draft, |_| ()).unwrap_err();
        assert_eq!(
            failure.detail,
            format!("{error}\n\nChanges were staged before generating AI metadata.")
        );
        assert!(git.actions().is_empty());
    }
}
