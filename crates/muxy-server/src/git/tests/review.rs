use super::extensions::{commit, fake_gh, pr, remote};
use super::*;
use muxy_protocol::{
    GitBaseSwitch, GitChangesPreview, GitPullRequestAction as Pr, GitPushDestination,
};
use std::os::unix::fs::PermissionsExt;

fn preview(repo: &Repo) -> GitChangesPreview {
    let GitReply::ChangesPreview(preview) = repo
        .git(GitAction::ChangesPreview {
            line_limit: Some(800),
        })
        .unwrap()
    else {
        panic!()
    };
    *preview
}

fn commit_preview(repo: &Repo, preview: &GitChangesPreview) -> Result<GitReply> {
    repo.git(GitAction::CommitAll {
        message: "Reviewed change".into(),
        expected_head: preview.head.clone(),
        expected_tree: preview.tree.clone(),
    })
}

fn porcelain(repo: &Repo) -> Vec<u8> {
    run(
        &repo.path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .unwrap()
}

fn destination(remote: &str, branch: &str) -> GitPushDestination {
    GitPushDestination {
        remote: remote.into(),
        branch: branch.into(),
    }
}

#[test]
fn preview_describes_every_change_without_touching_the_index() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("tracked"), "one\n").unwrap();
    commit(&repo, "tracked");
    std::fs::write(repo.path.join("tracked"), "one\ntwo\n").unwrap();
    std::fs::write(repo.path.join("staged"), "staged\n").unwrap();
    run(&repo.path, &["add", "staged"]).unwrap();
    std::fs::write(repo.path.join("untracked"), "new\n").unwrap();
    let before = porcelain(&repo);

    let preview = preview(&repo);

    assert_eq!(porcelain(&repo), before);
    assert_eq!(preview.branch.as_deref(), Some("main"));
    assert_eq!(preview.head, repo.summary().head);
    let files: Vec<_> = preview
        .files
        .iter()
        .map(|file| {
            (
                String::from_utf8_lossy(&file.path.0).into_owned(),
                file.added,
                file.removed,
                file.untracked,
            )
        })
        .collect();
    assert_eq!(
        files,
        [
            ("staged".into(), Some(1), Some(0), false),
            ("tracked".into(), Some(1), Some(0), false),
            ("untracked".into(), Some(1), Some(0), true),
        ]
    );
    assert!(preview.diff.diff.contains("+two"));
    assert_eq!(preview.destination, None);
    assert!(
        std::fs::read_dir(repo.path.join(".git"))
            .unwrap()
            .flatten()
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with("muxy-preview-"))
    );
}

#[test]
fn commit_all_commits_the_previewed_tree_and_refuses_later_edits() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("a"), "a\n").unwrap();
    std::fs::write(repo.path.join("b"), "b\n").unwrap();
    let stale = preview(&repo);
    std::fs::write(repo.path.join("a"), "edited after the review\n").unwrap();
    let before = porcelain(&repo);

    let error = commit_preview(&repo, &stale).unwrap_err();

    assert!(error.message().contains("review the changes again"));
    assert_eq!(porcelain(&repo), before);
    let reviewed = preview(&repo);
    let GitReply::Commit(hash) = commit_preview(&repo, &reviewed).unwrap() else {
        panic!()
    };
    assert_eq!(repo.summary().head.as_deref(), Some(hash.as_str()));
    assert_eq!(repo.summary().changed, 0);
    assert_eq!(
        text(&run(&repo.path, &["rev-parse", "HEAD^{tree}"]).unwrap()).unwrap(),
        reviewed.tree
    );
    std::fs::write(repo.path.join("c"), "c\n").unwrap();
    let moved = GitChangesPreview {
        tree: preview(&repo).tree,
        ..reviewed
    };
    assert!(
        commit_preview(&repo, &moved)
            .unwrap_err()
            .message()
            .contains("branch moved")
    );
}

#[test]
fn failed_commit_keeps_what_the_user_had_staged() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("tracked"), "one\n").unwrap();
    commit(&repo, "tracked");
    std::fs::write(repo.path.join("tracked"), "one\ntwo\n").unwrap();
    std::fs::write(repo.path.join("staged"), "staged\n").unwrap();
    run(&repo.path, &["add", "staged"]).unwrap();
    std::fs::write(repo.path.join("untracked"), "new\n").unwrap();
    let hooks = repo.path.join(".git/test-hooks");
    std::fs::create_dir(&hooks).unwrap();
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, "#!/bin/sh\necho blocked by hook >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    run(
        &repo.path,
        &[
            OsString::from("config"),
            "core.hooksPath".into(),
            hooks.as_os_str().to_owned(),
        ],
    )
    .unwrap();
    let before = porcelain(&repo);

    let error = commit_preview(&repo, &preview(&repo)).unwrap_err();

    assert!(
        error.message().contains("blocked by hook"),
        "{}",
        error.message()
    );
    assert_eq!(porcelain(&repo), before);
    assert!(!repo.path.join(".git/index.lock").exists());
}

#[test]
fn preview_and_commit_refuse_unresolved_conflicts() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("file"), "base\n").unwrap();
    commit(&repo, "base");
    repo.git(GitAction::CreateBranch("other".into())).unwrap();
    std::fs::write(repo.path.join("file"), "other\n").unwrap();
    commit(&repo, "other");
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    std::fs::write(repo.path.join("file"), "main\n").unwrap();
    commit(&repo, "main");
    assert!(run(&repo.path, &["merge", "other"]).is_err());

    let error = repo
        .git(GitAction::ChangesPreview { line_limit: None })
        .unwrap_err();
    assert!(error.message().contains("Resolve merge conflicts"));
    let error = repo
        .git(GitAction::CommitAll {
            message: "Merge".into(),
            expected_head: repo.summary().head,
            expected_tree: "abcd".into(),
        })
        .unwrap_err();
    assert!(error.message().contains("Resolve merge conflicts"));
    assert!(repo.path.join(".git/MERGE_HEAD").exists());
}

#[test]
fn publish_pushes_the_same_branch_name_even_when_tracking_another_branch() {
    let repo = Repo::new(true);
    let remote = remote(&repo);
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    run(&repo.path, &["switch", "-c", "feature", "origin/main"]).unwrap();
    assert_eq!(repo.summary().upstream.as_deref(), Some("origin/main"));
    std::fs::write(repo.path.join("work"), "work\n").unwrap();
    let preview = preview(&repo);
    assert_eq!(preview.destination, Some(destination("origin", "feature")));
    commit_preview(&repo, &preview).unwrap();
    let main_before = run(&remote, &["rev-parse", "main"]).unwrap();

    repo.git(GitAction::PublishBranch {
        branch: "feature".into(),
        destination: destination("origin", "feature"),
    })
    .unwrap();

    assert_eq!(run(&remote, &["rev-parse", "main"]).unwrap(), main_before);
    assert_eq!(
        run(&remote, &["rev-parse", "feature"]).unwrap(),
        run(&repo.path, &["rev-parse", "HEAD"]).unwrap()
    );
    assert_eq!(repo.summary().upstream.as_deref(), Some("origin/feature"));
    let error = repo
        .git(GitAction::PublishBranch {
            branch: "feature".into(),
            destination: destination("origin", "main"),
        })
        .unwrap_err();
    assert!(error.message().contains("push destination changed"));
}

#[test]
fn pull_request_checkouts_publish_to_their_pull_request_head() {
    let repo = Repo::new(true);
    let remote = remote(&repo);
    run(&repo.path, &["push", "origin", "main:refs/heads/feature"]).unwrap();
    run(&repo.path, &["fetch", "origin"]).unwrap();
    run(
        &repo.path,
        &["branch", "--track", "pr/42/feature", "origin/feature"],
    )
    .unwrap();
    run(
        &repo.path,
        &["config", "branch.pr/42/feature.muxy-pr-number", "42"],
    )
    .unwrap();
    repo.git(GitAction::SwitchBranch("pr/42/feature".into()))
        .unwrap();
    std::fs::write(repo.path.join("fix"), "fix\n").unwrap();
    let preview = preview(&repo);
    assert_eq!(preview.destination, Some(destination("origin", "feature")));
    commit_preview(&repo, &preview).unwrap();

    repo.git(GitAction::PublishBranch {
        branch: "pr/42/feature".into(),
        destination: destination("origin", "feature"),
    })
    .unwrap();

    assert_eq!(
        run(&remote, &["rev-parse", "feature"]).unwrap(),
        run(&repo.path, &["rev-parse", "HEAD"]).unwrap()
    );
    assert!(
        run(
            &remote,
            &["rev-parse", "--verify", "refs/heads/pr/42/feature"]
        )
        .is_err()
    );
}

#[test]
fn switch_to_base_skips_other_worktrees_and_only_fast_forwards() {
    let repo = Repo::new(true);
    let remote = remote(&repo);
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    let other = repo.path.join(".git/other-clone");
    run(
        &repo.path,
        &[
            OsString::from("clone"),
            remote.as_os_str().to_owned(),
            other.as_os_str().to_owned(),
        ],
    )
    .unwrap();
    let advance = |message: &str| {
        run(
            &other,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                message,
            ],
        )
        .unwrap();
        run(&other, &["push", "origin", "main"]).unwrap();
    };
    advance("merged pull request");
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    let linked = repo.path.join(".git/linked-main");
    run(
        &repo.path,
        &[
            OsString::from("worktree"),
            "add".into(),
            linked.as_os_str().to_owned(),
            "main".into(),
        ],
    )
    .unwrap();

    let GitReply::BaseSwitch(GitBaseSwitch::CheckedOutElsewhere(directory)) =
        repo.git(GitAction::SwitchToBase("main".into())).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        path(&directory).canonicalize().unwrap(),
        linked.canonicalize().unwrap()
    );
    assert_eq!(repo.summary().branch.as_deref(), Some("feature"));

    run(
        &repo.path,
        &[
            OsString::from("worktree"),
            "remove".into(),
            linked.as_os_str().to_owned(),
        ],
    )
    .unwrap();
    assert_eq!(
        repo.git(GitAction::SwitchToBase("main".into())).unwrap(),
        GitReply::BaseSwitch(GitBaseSwitch::Updated)
    );
    assert_eq!(repo.summary().branch.as_deref(), Some("main"));
    assert_eq!(
        run(&repo.path, &["rev-parse", "HEAD"]).unwrap(),
        run(&repo.path, &["rev-parse", "origin/main"]).unwrap()
    );

    std::fs::write(repo.path.join("local"), "local only\n").unwrap();
    commit(&repo, "local only");
    repo.git(GitAction::SwitchBranch("feature".into())).unwrap();
    advance("another merge");
    let error = repo
        .git(GitAction::SwitchToBase("main".into()))
        .unwrap_err();
    assert!(
        error.message().contains("couldn't fast-forward"),
        "{}",
        error.message()
    );
    assert_eq!(repo.summary().branch.as_deref(), Some("main"));
}

fn pull_request_behind_base(repo: &mut Repo, local: &str, feature_file: &str) -> (PathBuf, String) {
    let remote = remote(repo);
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    run(&repo.path, &["switch", "-c", local]).unwrap();
    std::fs::write(repo.path.join(feature_file), "feature work\n").unwrap();
    commit(repo, "feature");
    run(
        &repo.path,
        &[
            "push",
            "--set-upstream",
            "origin",
            "HEAD:refs/heads/feature",
        ],
    )
    .unwrap();
    let head = repo.summary().head.unwrap();
    run(&repo.path, &["switch", "main"]).unwrap();
    std::fs::write(repo.path.join("base"), "new base work\n").unwrap();
    commit(repo, "base");
    run(&repo.path, &["push", "origin", "main"]).unwrap();
    run(&repo.path, &["switch", local]).unwrap();
    fake_gh(repo);
    std::fs::write(
        repo.path.join(".git/gh-pr"),
        format!(
            r#"{{"number":42,"url":"https://github.com/example/repository/pull/42","title":"A PR","author":{{"login":"contributor"}},"headRefName":"feature","headRefOid":"{head}","baseRefName":"main","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BEHIND","isCrossRepository":false,"statusCheckRollup":[]}}"#
        ),
    )
    .unwrap();
    (remote, head)
}

fn update(repo: &Repo, head: &str) -> Result<GitReply> {
    pr(
        repo,
        Pr::UpdateBranch {
            number: 42,
            expected_head: head.into(),
        },
    )
}

#[test]
fn update_branch_works_for_muxy_pull_request_checkouts_with_untracked_files() {
    let mut repo = Repo::new(true);
    let (remote, head) = pull_request_behind_base(&mut repo, "pr/42/feature", "feature");
    run(
        &repo.path,
        &["config", "branch.pr/42/feature.muxy-pr-number", "42"],
    )
    .unwrap();
    std::fs::write(repo.path.join("notes"), "untracked\n").unwrap();

    assert_eq!(update(&repo, &head).unwrap(), GitReply::Done);

    assert!(repo.path.join("base").exists());
    assert_eq!(
        run(&remote, &["rev-parse", "feature"]).unwrap(),
        run(&repo.path, &["rev-parse", "HEAD"]).unwrap()
    );
}

#[test]
fn update_branch_explains_each_refusal_and_partial_failure() {
    let mut repo = Repo::new(true);
    let (_, head) = pull_request_behind_base(&mut repo, "feature", "base");
    let error = update(&repo, &head).unwrap_err();
    assert!(
        error.message().contains("merge was aborted"),
        "{}",
        error.message()
    );
    assert!(!repo.path.join(".git/MERGE_HEAD").exists());
    assert_eq!(repo.summary().head.as_deref(), Some(head.as_str()));

    let mut repo = Repo::new(true);
    let (remote, head) = pull_request_behind_base(&mut repo, "feature", "feature");
    let hook = remote.join("hooks/pre-receive");
    std::fs::write(&hook, "#!/bin/sh\necho rejected >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = update(&repo, &head).unwrap_err();
    assert!(
        error.message().contains("locally, but couldn't push"),
        "{}",
        error.message()
    );
    assert_ne!(repo.summary().head.as_deref(), Some(head.as_str()));

    let mut repo = Repo::new(true);
    let (_, head) = pull_request_behind_base(&mut repo, "feature", "feature");
    std::fs::write(repo.path.join("local"), "unpushed\n").unwrap();
    commit(&repo, "unpushed");
    let error = update(&repo, &head).unwrap_err();
    assert!(
        error
            .message()
            .contains("isn't at the pull request's head commit")
    );

    run(&repo.path, &["switch", "main"]).unwrap();
    let error = update(&repo, &head).unwrap_err();
    assert!(error.message().contains("no longer has that pull request"));

    let mut repo = Repo::new(true);
    let (_, head) = pull_request_behind_base(&mut repo, "feature", "feature");
    let fork = std::fs::read_to_string(repo.path.join(".git/gh-pr"))
        .unwrap()
        .replace(
            r#""isCrossRepository":false"#,
            r#""isCrossRepository":true"#,
        );
    std::fs::write(repo.path.join(".git/gh-pr"), fork).unwrap();
    let error = update(&repo, &head).unwrap_err();
    assert!(error.message().contains("forks"));
}

#[test]
fn pull_request_from_the_default_branch_moves_uncommitted_work_to_a_new_branch() {
    let mut repo = Repo::new(true);
    let remote = remote(&repo);
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    let main_before = run(&repo.path, &["rev-parse", "main"]).unwrap();
    fake_gh(&mut repo);
    std::fs::write(repo.path.join(".git/gh-error"), "no pull requests found").unwrap();
    assert_eq!(pr(&repo, Pr::Info).unwrap(), GitReply::PullRequest(None));
    std::fs::remove_file(repo.path.join(".git/gh-error")).unwrap();
    std::fs::write(repo.path.join("work"), "uncommitted work\n").unwrap();
    let preview = preview(&repo);
    assert_eq!(preview.destination, Some(destination("origin", "main")));

    repo.git(GitAction::CreateBranch("fix/status".into()))
        .unwrap();
    let GitReply::Commit(hash) = commit_preview(&repo, &preview).unwrap() else {
        panic!()
    };
    repo.git(GitAction::PublishBranch {
        branch: "fix/status".into(),
        destination: destination("origin", "fix/status"),
    })
    .unwrap();
    let GitReply::PullRequest(Some(created)) = pr(
        &repo,
        Pr::Create {
            title: "Fix status".into(),
            body: "Summary".into(),
            base_branch: Some("main".into()),
            draft: false,
        },
    )
    .unwrap() else {
        panic!()
    };

    assert_eq!(created.number, 42);
    assert_eq!(repo.summary().branch.as_deref(), Some("fix/status"));
    assert_eq!(repo.summary().changed, 0);
    assert_eq!(
        text(&run(&remote, &["rev-parse", "fix/status"]).unwrap()).unwrap(),
        hash
    );
    assert_eq!(
        run(&repo.path, &["rev-parse", "main"]).unwrap(),
        main_before
    );
    assert_eq!(run(&remote, &["rev-parse", "main"]).unwrap(), main_before);
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains(
        "create\n--repo\nhttps://github.com/example/repository\n--head\nfix/status\n--base\nmain\n"
    ));
}
