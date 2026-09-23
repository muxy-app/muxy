use super::*;
use muxy_protocol::{
    GitDiffKind, GitDiffRequest, GitMergeMethod, GitPullRequestAction as Pr, GitPullRequestFilter,
};
use std::os::unix::fs::PermissionsExt;

pub(super) fn commit(repo: &Repo, message: &str) -> String {
    let GitReply::Commit(hash) = repo
        .git(GitAction::Commit {
            message: message.into(),
            stage_all: true,
        })
        .unwrap()
    else {
        panic!()
    };
    hash
}

#[test]
fn init_stage_all_unstage_all_commit_and_history_cover_unborn_repositories() {
    let repo = Repo::new(false);
    std::fs::remove_dir_all(repo.path.join(".git")).unwrap();
    assert_eq!(
        repo.git(GitAction::Summary).unwrap(),
        GitReply::Summary(None)
    );
    repo.git(GitAction::Init).unwrap();
    run(&repo.path, &["config", "user.name", "Test"]).unwrap();
    run(
        &repo.path,
        &["config", "user.email", "test@example.invalid"],
    )
    .unwrap();
    run(&repo.path, &["config", "commit.gpgsign", "false"]).unwrap();
    assert_eq!(
        repo.git(GitAction::Log {
            max_count: 100,
            skip: 0
        })
        .unwrap(),
        GitReply::Log(vec![])
    );
    repo.git(GitAction::Unstage(vec![])).unwrap();
    std::fs::create_dir(repo.path.join("nested")).unwrap();
    std::fs::write(repo.path.join("nested/file"), "first\n").unwrap();
    repo.git(GitAction::Stage(vec![])).unwrap();
    assert_eq!(repo.summary().staged, 1);
    repo.git(GitAction::Unstage(vec![])).unwrap();
    assert_eq!(repo.summary().staged, 0);
    assert!(repo.path.join("nested/file").exists());
    let first = commit(&repo, "first π\n\nbody");
    std::fs::write(repo.path.join("nested/file"), "second\n").unwrap();
    let second = commit(&repo, "second");
    repo.git(GitAction::CreateTag {
        name: "v1.0".into(),
        hash: second.clone(),
    })
    .unwrap();
    let GitReply::Log(log) = repo
        .git(GitAction::Log {
            max_count: 1,
            skip: 0,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(log[0].hash, second);
    assert_eq!(
        log[0].parent_hashes.as_slice(),
        std::slice::from_ref(&first)
    );
    assert!(
        log[0]
            .refs
            .iter()
            .any(|r| r.kind == muxy_protocol::GitRefKind::Tag && r.name == "v1.0")
    );
    assert_eq!(log[0].author_name, "Test");
    let GitReply::Log(log) = repo
        .git(GitAction::Log {
            max_count: 1,
            skip: 1,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(log[0].hash, first);
    assert_eq!(log[0].subject, "first π");
    repo.git(GitAction::Unstage(vec![])).unwrap();
    let GitReply::RepoInfo(info) = repo.git(GitAction::RepoInfo).unwrap() else {
        panic!()
    };
    assert_eq!(
        path(&info.root).canonicalize().unwrap(),
        repo.path.canonicalize().unwrap()
    );
    assert!(!info.is_worktree);
}

#[test]
fn diffs_and_status_separate_staged_worktree_untracked_and_binary_changes() {
    let repo = Repo::new(false);
    let relative = ServerPath(b"file [*] \n.txt".to_vec());
    let file = repo.path.join(path(&relative));
    std::fs::write(&file, "one\ntwo\n").unwrap();
    commit(&repo, "initial");
    std::fs::write(&file, "one\nstaged\n").unwrap();
    repo.git(GitAction::Stage(vec![relative.clone()])).unwrap();
    std::fs::write(&file, "one\nworking\nextra\n").unwrap();
    for (staged, expected, additions) in [(true, "+staged", 1), (false, "+working", 2)] {
        let GitReply::Diff(diff) = repo
            .git(GitAction::Diff(GitDiffRequest {
                path: Some(relative.clone()),
                staged,
                ..GitDiffRequest::default()
            }))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(diff.additions, additions);
        assert_eq!(diff.deletions, 1);
        assert!(
            diff.rows
                .iter()
                .any(|row| row.text == expected && row.kind == GitDiffKind::Addition)
        );
        assert!(
            diff.rows
                .iter()
                .any(|row| row.new_line_number == Some(2) && row.kind == GitDiffKind::Addition)
        );
    }
    let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
        panic!()
    };
    assert_eq!(status.files[0].staged.additions, Some(1));
    assert_eq!(status.files[0].unstaged.additions, Some(2));
    let GitReply::RawDiff(diff) = repo
        .git(GitAction::Diff(GitDiffRequest {
            raw: true,
            line_limit: Some(2),
            ..GitDiffRequest::default()
        }))
        .unwrap()
    else {
        panic!()
    };
    assert!(diff.truncated);
    assert_eq!(diff.diff.lines().count(), 2);
    std::fs::write(repo.path.join("untracked"), "new\nlast").unwrap();
    let GitReply::Diff(diff) = repo
        .git(GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"untracked".to_vec())),
            ..GitDiffRequest::default()
        }))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(diff.additions, 2);
    std::fs::write(repo.path.join("binary"), b"a\0b").unwrap();
    repo.git(GitAction::Stage(vec![ServerPath(b"binary".to_vec())]))
        .unwrap();
    let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
        panic!()
    };
    assert!(
        status
            .files
            .iter()
            .find(|file| file.file.path.0 == b"binary")
            .unwrap()
            .staged
            .binary
    );
    for bad in [b"../outside".as_slice(), b".git/config", b"/etc/passwd"] {
        assert!(
            repo.git(GitAction::Diff(GitDiffRequest {
                path: Some(ServerPath(bad.to_vec())),
                ..GitDiffRequest::default()
            }))
            .is_err()
        );
    }
}

#[test]
fn large_diff_is_bounded_and_reports_truncation() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("large"), "a".repeat(2 * 1024 * 1024)).unwrap();
    let GitReply::Diff(diff) = repo
        .git(GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"large".to_vec())),
            ..GitDiffRequest::default()
        }))
        .unwrap()
    else {
        panic!()
    };
    assert!(diff.truncated);
    let bytes = postcard::to_allocvec(&GitReply::Diff(diff)).unwrap();
    assert!(bytes.len() < 16 * 1024 * 1024);
}

#[test]
fn branch_diff_includes_committed_branch_changes() {
    let repo = Repo::new(true);
    let remote = repo.path.join(".git/remote.git");
    run(&repo.path, &["init", "--bare", remote.to_str().unwrap()]).unwrap();
    run(
        &repo.path,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    )
    .unwrap();
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    std::fs::write(repo.path.join("committed"), "earlier branch work\n").unwrap();
    commit(&repo, "earlier work");
    let GitReply::RawDiff(diff) = repo
        .git(GitAction::BranchDiff {
            base: "main".into(),
            line_limit: Some(800),
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(diff.diff.contains("earlier branch work"));
}

#[test]
fn checkout_cherry_pick_revert_and_optional_force_deletion() {
    let repo = Repo::new(true);
    let initial = repo.summary().head.unwrap();
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    std::fs::write(repo.path.join("change"), "added\n").unwrap();
    let hash = commit(&repo, "feature");
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    assert!(
        repo.git(GitAction::DeleteLocalBranch {
            name: "feature".into(),
            force: false
        })
        .is_err()
    );
    repo.git(GitAction::CherryPick(hash.clone())).unwrap();
    assert!(repo.path.join("change").exists());
    let head = repo.summary().head.unwrap();
    repo.git(GitAction::Revert(head.clone())).unwrap();
    assert_eq!(repo.summary().head.as_ref(), Some(&head));
    assert_eq!(repo.summary().staged, 1);
    commit(&repo, "reverted");
    repo.git(GitAction::Checkout(initial)).unwrap();
    assert!(repo.summary().branch.is_none());
    repo.git(GitAction::DeleteLocalBranch {
        name: "feature".into(),
        force: true,
    })
    .unwrap();
}

pub(super) fn remote(repo: &Repo) -> PathBuf {
    let remote = repo.path.join(".git/test-remote.git");
    std::fs::create_dir(&remote).unwrap();
    run(&remote, &["init", "--bare", "-b", "main"]).unwrap();
    run(
        &repo.path,
        &[
            OsString::from("remote"),
            "add".into(),
            "origin".into(),
            remote.as_os_str().to_owned(),
        ],
    )
    .unwrap();
    remote
}

#[test]
fn push_sets_upstream_lists_and_deletes_remote_branches_and_pull_updates() {
    let repo = Repo::new(true);
    let remote = remote(&repo);
    repo.git(GitAction::Push {
        set_upstream: false,
    })
    .unwrap();
    assert_eq!(repo.summary().upstream.as_deref(), Some("origin/main"));
    assert_eq!(
        repo.git(GitAction::RemoteBranches).unwrap(),
        GitReply::RemoteBranches(vec!["main".into()])
    );
    let other = Repo::new(true);
    run(
        &other.path,
        &[
            OsString::from("remote"),
            "add".into(),
            "origin".into(),
            remote.as_os_str().to_owned(),
        ],
    )
    .unwrap();
    run(&other.path, &["fetch", "origin"]).unwrap();
    run(&other.path, &["reset", "--hard", "origin/main"]).unwrap();
    std::fs::write(other.path.join("from-other"), "pulled\n").unwrap();
    let hash = commit(&other, "from other");
    other.git(GitAction::Push { set_upstream: true }).unwrap();
    repo.git(GitAction::Pull).unwrap();
    assert_eq!(repo.summary().head.as_deref(), Some(hash.as_str()));
    repo.git(GitAction::CreateBranch("temporary".into()))
        .unwrap();
    repo.git(GitAction::Push { set_upstream: true }).unwrap();
    repo.git(GitAction::DeleteRemoteBranch("temporary".into()))
        .unwrap();
    assert_eq!(
        repo.git(GitAction::RemoteBranches).unwrap(),
        GitReply::RemoteBranches(vec!["main".into()])
    );
}

pub(super) fn fake_gh(repo: &mut Repo) {
    let executable = repo.path.join(".git/fake-gh");
    std::fs::write(&executable, r#"#!/bin/sh
printf '%s\n' "$@" >> .git/gh-calls
case "$1 $2" in
  'repo view') printf '%s\n' '{"url":"https://github.com/example/repository"}'; exit 0;;
esac
if test -f .git/gh-error; then cat .git/gh-error >&2; exit 1; fi
if test "$1 $2 $3" = 'pr view --repo'; then
  printf '%s\n' 'argument required when using the --repo flag' >&2; exit 1
fi
if test "$1 $2" = 'pr view' && test -f .git/gh-view-error; then
  cat .git/gh-view-error >&2; exit 1
fi
if test "$1 $2 $3" = 'pr view --json' && test -f .git/gh-implicit-error; then
  cat .git/gh-implicit-error >&2; exit 1
fi
if test "$1 $2 $3" = 'pr view 123'; then sed 's/"number":42/"number":123/' .git/gh-pr; exit 0; fi
case "$1 $2" in
  'pr view')
    for arg do
      case "$arg" in
        number) printf '%s\n' '{"number":42}'; exit 0;;
        headRefName,headRepository,headRepositoryOwner) if test -f .git/gh-checkout; then cat .git/gh-checkout; else printf '%s\n' '{"headRefName":"feature","headRepository":{"name":"fork"},"headRepositoryOwner":{"login":"contributor"}}'; fi; exit 0;;
      esac
    done
    cat .git/gh-pr;;
  'pr list') if test -f .git/gh-list; then cat .git/gh-list; else printf '['; cat .git/gh-pr; printf ']'; fi;;
  'pr create') printf '%s\n' 'https://github.com/example/repository/pull/42';;
  'pr diff') printf 'diff --git a/file b/file\n--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n+new\n';;
  'pr merge'|'pr close') exit 0;;
  *) exit 2;;
esac
"#).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let response = r#"{
        "number":42,"url":"https://github.com/example/repository/pull/42","title":"A PR","author":{"login":"contributor"},
        "headRefName":"main","headRefOid":"abc123","baseRefName":"main","state":"OPEN","isDraft":false,
        "updatedAt":"2026-09-18T00:00:00Z","mergeable":"UNKNOWN","mergeStateStatus":"BLOCKED","isCrossRepository":true,
        "statusCheckRollup":[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},
        {"__typename":"CheckRun","status":"IN_PROGRESS"},{"__typename":"StatusContext","state":"FAILURE"}]
    }"#
    .replace("abc123", &repo.summary().head.unwrap());
    std::fs::write(repo.path.join(".git/gh-pr"), response).unwrap();
    repo.registry.git.github.executable = Some(executable);
}

pub(super) fn pr(repo: &Repo, action: Pr) -> Result<GitReply> {
    repo.git(GitAction::PullRequest(action))
}

#[test]
fn github_reads_mutations_and_errors_have_structured_results_and_explicit_targets() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    let GitReply::PullRequest(Some(info)) = pr(&repo, Pr::Info).unwrap() else {
        panic!()
    };
    assert_eq!(info.number, 42);
    assert_eq!(info.mergeable, None);
    assert!(
        pr(
            &repo,
            Pr::UpdateBranch {
                number: 42,
                expected_head: "deadbeef".into(),
            },
        )
        .unwrap_err()
        .message()
        .contains("changed")
    );
    assert_eq!(
        (
            info.checks.passing,
            info.checks.failing,
            info.checks.pending
        ),
        (1, 1, 1)
    );
    assert_eq!(
        pr(&repo, Pr::Number).unwrap(),
        GitReply::PullRequestNumber(Some(42))
    );
    for filter in [
        GitPullRequestFilter::Open,
        GitPullRequestFilter::Closed,
        GitPullRequestFilter::Merged,
        GitPullRequestFilter::All,
    ] {
        pr(
            &repo,
            Pr::List {
                filter,
                limit: 50,
                checks: false,
            },
        )
        .unwrap();
    }
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(!calls.lines().last().unwrap().contains("statusCheckRollup"));
    let GitReply::RawDiff(diff) = pr(
        &repo,
        Pr::Diff {
            number: 42,
            line_limit: Some(2),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert!(diff.truncated);
    for method in [
        GitMergeMethod::Merge,
        GitMergeMethod::Squash,
        GitMergeMethod::Rebase,
    ] {
        pr(
            &repo,
            Pr::Merge {
                number: 42,
                method,
                delete_branch: true,
                expected_head: (method == GitMergeMethod::Squash).then(|| "abc123".into()),
            },
        )
        .unwrap();
    }
    pr(&repo, Pr::Close { number: 42 }).unwrap();
    remote(&repo);
    pr(
        &repo,
        Pr::Create {
            title: "PR title".into(),
            body: "body\nsecond line".into(),
            base_branch: Some("main".into()),
            draft: true,
        },
    )
    .unwrap();
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("--repo\nhttps://github.com/example/repository\n"));
    assert!(calls.contains("--title\nPR title\n--body\nbody\nsecond line\n--draft"));
    assert!(calls.contains("--squash\n--delete-branch\n--match-head-commit\nabc123\n"));
    assert!(calls.contains("--rebase\n--delete-branch\n"));
    assert!(!calls.contains("--rebase\n--delete-branch\n--match-head-commit"));
}

#[test]
fn github_ignores_old_prs_for_a_reused_branch_and_matches_the_current_commit() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    let response = repo.path.join(".git/gh-pr");
    let mut old: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
    old["state"] = "CLOSED".into();
    old["headRefOid"] = "old-commit".into();
    std::fs::write(&response, old.to_string()).unwrap();
    assert_eq!(pr(&repo, Pr::Info).unwrap(), GitReply::PullRequest(None));
    assert_eq!(
        pr(&repo, Pr::Number).unwrap(),
        GitReply::PullRequestNumber(None)
    );

    let mut unrelated_fork = old.clone();
    unrelated_fork["state"] = "OPEN".into();
    std::fs::write(&response, unrelated_fork.to_string()).unwrap();
    assert_eq!(pr(&repo, Pr::Info).unwrap(), GitReply::PullRequest(None));
    std::fs::write(&response, old.to_string()).unwrap();

    let mut current = old.clone();
    current["number"] = 73.into();
    current["headRefOid"] = repo.summary().head.unwrap().into();
    std::fs::write(
        repo.path.join(".git/gh-list"),
        serde_json::json!([old, current]).to_string(),
    )
    .unwrap();
    let GitReply::PullRequest(Some(info)) = pr(&repo, Pr::Info).unwrap() else {
        panic!()
    };
    assert_eq!(info.number, 73);
}

#[test]
fn github_finds_an_open_branch_pr_when_local_head_has_advanced() {
    let mut repo = Repo::new(true);
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    fake_gh(&mut repo);
    let response = repo.path.join(".git/gh-pr");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
    value["headRefName"] = "feature".into();
    value["isCrossRepository"] = false.into();
    std::fs::write(response, value.to_string()).unwrap();
    std::fs::write(
        repo.path.join(".git/gh-implicit-error"),
        "no pull requests found for current branch",
    )
    .unwrap();
    std::fs::write(repo.path.join("new-work"), "ahead of PR\n").unwrap();
    commit(&repo, "new local work");

    let GitReply::PullRequest(Some(info)) = pr(&repo, Pr::Info).unwrap() else {
        panic!()
    };
    assert_eq!(info.number, 42);
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("pr\nview\nfeature\n"));
}

#[test]
fn github_update_branch_merges_base_and_pushes_the_pr_head() {
    let mut repo = Repo::new(true);
    let remote = repo.path.join(".git/remote.git");
    run(&repo.path, &["init", "--bare", remote.to_str().unwrap()]).unwrap();
    run(
        &repo.path,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    )
    .unwrap();
    run(&repo.path, &["push", "-u", "origin", "main"]).unwrap();
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    std::fs::write(repo.path.join("feature"), "feature work\n").unwrap();
    commit(&repo, "feature");
    repo.git(GitAction::Push { set_upstream: true }).unwrap();
    let expected_head = repo.summary().head.unwrap();
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    std::fs::write(repo.path.join("base"), "new base work\n").unwrap();
    commit(&repo, "base");
    repo.git(GitAction::Push {
        set_upstream: false,
    })
    .unwrap();
    repo.git(GitAction::SwitchBranch("feature".into())).unwrap();
    fake_gh(&mut repo);
    std::fs::write(
        repo.path.join(".git/gh-pr"),
        format!(
            r#"{{"number":42,"url":"https://github.com/example/repository/pull/42","title":"A PR","author":{{"login":"contributor"}},"headRefName":"feature","headRefOid":"{expected_head}","baseRefName":"main","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BEHIND","isCrossRepository":false,"statusCheckRollup":[]}}"#
        ),
    )
    .unwrap();
    assert_eq!(
        pr(
            &repo,
            Pr::UpdateBranch {
                number: 42,
                expected_head: expected_head.clone(),
            },
        )
        .unwrap(),
        GitReply::Done
    );
    assert!(repo.path.join("base").exists());
    assert_ne!(repo.summary().head.unwrap(), expected_head);
    assert_eq!(
        run(&repo.path, &["rev-parse", "HEAD"]).unwrap(),
        run(&repo.path, &["rev-parse", "origin/feature"]).unwrap()
    );
}

#[test]
fn github_returns_no_pr_when_view_finds_none_and_branch_list_is_empty() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    std::fs::write(
        repo.path.join(".git/gh-view-error"),
        "no pull requests found for branch main",
    )
    .unwrap();
    std::fs::write(repo.path.join(".git/gh-list"), "[]").unwrap();

    assert_eq!(pr(&repo, Pr::Info).unwrap(), GitReply::PullRequest(None));
    assert_eq!(
        pr(&repo, Pr::Number).unwrap(),
        GitReply::PullRequestNumber(None)
    );
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("pr\nview\n--json\n"));
    assert!(!calls.contains("pr\nview\n--repo\n"));
}

#[test]
fn github_distinguishes_absent_prs_authentication_missing_tools_and_invalid_data() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    std::fs::write(
        repo.path.join(".git/gh-error"),
        "no pull requests found for branch",
    )
    .unwrap();
    assert_eq!(pr(&repo, Pr::Info).unwrap(), GitReply::PullRequest(None));
    assert_eq!(
        pr(&repo, Pr::Number).unwrap(),
        GitReply::PullRequestNumber(None)
    );
    std::fs::write(repo.path.join(".git/gh-error"), "authentication failed").unwrap();
    assert!(
        pr(&repo, Pr::Info)
            .unwrap_err()
            .message()
            .contains("authentication failed")
    );
    repo.git(GitAction::Status { local: false }).unwrap();
    std::fs::remove_file(repo.path.join(".git/gh-error")).unwrap();
    std::fs::write(repo.path.join(".git/gh-pr"), "bad JSON").unwrap();
    assert!(pr(&repo, Pr::Info).is_err());
    repo.registry.git.github.executable = Some(repo.path.join("missing-gh"));
    assert!(
        pr(&repo, Pr::Info)
            .unwrap_err()
            .message()
            .contains("GitHub CLI")
    );
    repo.git(GitAction::Status { local: true }).unwrap();
}

#[test]
fn executable_lookup_takes_the_first_runnable_file_and_stops_searching() {
    let repo = Repo::new(false);
    let [missing, plain, folder, installed] =
        ["missing", "plain", "folder", "installed"].map(|name| repo.path.join(name));
    std::fs::create_dir_all(folder.join("gh")).unwrap();
    for directory in [&plain, &installed] {
        std::fs::create_dir(directory).unwrap();
        std::fs::write(directory.join("gh"), "#!/bin/sh\n").unwrap();
    }
    std::fs::set_permissions(installed.join("gh"), std::fs::Permissions::from_mode(0o700)).unwrap();
    let never_searched = std::iter::once_with(|| -> PathBuf { panic!("searched past a match") });
    assert_eq!(
        command::find_executable(
            "gh",
            [missing, plain, folder, installed.clone()]
                .into_iter()
                .chain(never_searched),
        ),
        Some(installed.join("gh"))
    );
}

#[test]
fn pull_request_checkout_and_worktree_use_fork_tracking_and_replay_receipts() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    let remote = remote(&repo);
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    std::fs::write(repo.path.join("feature-file"), "feature\n").unwrap();
    commit(&repo, "feature");
    repo.git(GitAction::Push { set_upstream: true }).unwrap();
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    let replacement = format!("url.{}.insteadOf", remote.display());
    run(
        &repo.path,
        &[
            "config",
            &replacement,
            "https://github.com/contributor/fork.git",
        ],
    )
    .unwrap();
    pr(&repo, Pr::Checkout { number: 42 }).unwrap();
    assert_eq!(repo.summary().branch.as_deref(), Some("pr/42/feature"));
    std::fs::write(repo.path.join("feature-file"), "updated\n").unwrap();
    let hash = commit(&repo, "PR update");
    repo.git(GitAction::Push {
        set_upstream: false,
    })
    .unwrap();
    assert_eq!(
        text(&run(&remote, &["rev-parse", "refs/heads/feature"]).unwrap()).unwrap(),
        hash
    );
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    let action = GitAction::Worktree(WorktreeIntent {
        operation: OperationId::new(),
        action: WorktreeAction::CheckoutPullRequest {
            project: ProjectId::new(),
            directory: server_path(&repo.path.join("pr-worktree")),
            number: 42,
        },
    });
    let reply = repo.git(action.clone()).unwrap();
    assert_eq!(repo.git(action).unwrap(), reply);
    let GitReply::Project(project) = reply else {
        panic!()
    };
    assert_eq!(project.parent_id, Some(repo.project));
    let GitReply::RepoInfo(info) = repo
        .registry
        .git(&GitRequest {
            project: project.id,
            action: GitAction::RepoInfo,
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(info.is_worktree);
    assert_eq!(info.current_branch.as_deref(), Some("pr/42/feature"));
}

#[test]
fn parsed_diff_handles_conflicts_symlinks_binary_files_and_literal_paths() {
    use std::os::unix::fs::symlink;
    let repo = Repo::new(false);
    std::fs::write(repo.path.join("file"), "base\n").unwrap();
    commit(&repo, "base");
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    std::fs::write(repo.path.join("file"), "feature\n").unwrap();
    commit(&repo, "feature");
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    std::fs::write(repo.path.join("file"), "main\n").unwrap();
    commit(&repo, "main");
    assert!(run(&repo.path, &["merge", "feature"]).is_err());
    let GitReply::Diff(diff) = repo
        .git(GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"file".to_vec())),
            ..GitDiffRequest::default()
        }))
        .unwrap()
    else {
        panic!()
    };
    assert!(diff.rows.iter().any(|row| row.text.contains("<<<<<<<")));
    run(&repo.path, &["merge", "--abort"]).unwrap();
    std::fs::write(repo.path.join("binary"), b"text\0binary").unwrap();
    let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
        panic!()
    };
    assert!(
        status
            .files
            .iter()
            .find(|file| file.file.path.0 == b"binary")
            .unwrap()
            .unstaged
            .binary
    );
    symlink("/etc/passwd", repo.path.join("symlink")).unwrap();
    let GitReply::Diff(diff) = repo
        .git(GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"symlink".to_vec())),
            ..GitDiffRequest::default()
        }))
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(diff.additions, 1);
    assert!(diff.rows.iter().any(|row| row.text == "+/etc/passwd"));
    symlink("/etc", repo.path.join("outside")).unwrap();
    assert!(
        repo.git(GitAction::Diff(GitDiffRequest {
            path: Some(ServerPath(b"outside/passwd".to_vec())),
            ..GitDiffRequest::default()
        }))
        .is_err()
    );
}

#[test]
fn bounded_commands_stop_on_timeout_and_do_not_hide_failures_as_empty_results() {
    use std::process::Command;
    use std::time::{Duration, Instant};
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 10"]);
    let started = Instant::now();
    assert!(
        command::capture_with(command, Duration::from_millis(50), None, false)
            .unwrap_err()
            .message()
            .contains("timed out")
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exit 7"]);
    assert!(
        command::capture(command)
            .unwrap_err()
            .message()
            .contains('7')
    );
}

#[test]
fn staging_edits_to_an_index_rename_does_not_add_its_deleted_source() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("old"), "first\nsecond\nthird\n").unwrap();
    commit(&repo, "original");
    run(&repo.path, &["mv", "old", "new"]).unwrap();
    std::fs::write(repo.path.join("new"), "first\nsecond\nthird\nedit\n").unwrap();
    assert!(
        read::files(&read::status(&repo.path).unwrap())
            .unwrap()
            .iter()
            .any(|file| file.index == b'R' && file.worktree == b'M')
    );
    repo.git(GitAction::Stage(vec![ServerPath(b"new".to_vec())]))
        .unwrap();
    assert_eq!(repo.summary().unstaged, 0);
    assert!(!repo.path.join("old").exists());
    repo.git(GitAction::Unstage(vec![ServerPath(b"new".to_vec())]))
        .unwrap();
    assert_eq!(repo.summary().staged, 0);
}

#[test]
fn numeric_branches_are_not_treated_as_pr_numbers_unless_configured() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    repo.git(GitAction::CreateBranch("123".into())).unwrap();
    let response = repo.path.join(".git/gh-pr");
    let json = std::fs::read_to_string(&response)
        .unwrap()
        .replace("\"headRefName\":\"main\"", "\"headRefName\":\"123\"");
    std::fs::write(response, json).unwrap();
    let GitReply::PullRequest(Some(info)) = pr(&repo, Pr::Info).unwrap() else {
        panic!()
    };
    assert_eq!(info.number, 42);
    assert_eq!(
        pr(&repo, Pr::Number).unwrap(),
        GitReply::PullRequestNumber(Some(42))
    );
    let GitReply::Status(status) = repo.git(GitAction::Status { local: false }).unwrap() else {
        panic!()
    };
    assert_eq!(status.pull_request.unwrap().number, 42);
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("pr\nview\n--json\n"));
    assert!(!calls.contains("pr\nview\n123\n"));
    run(&repo.path, &["config", "branch.123.muxy-pr-number", "42"]).unwrap();
    pr(&repo, Pr::Info).unwrap();
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("pr\nview\n42\n"));
}

#[test]
fn status_prefers_remote_default_branch_over_stale_local_ref_and_caches_it() {
    let repo = Repo::new(true);
    let remote = traced_remote(&repo);
    run(
        &remote,
        &["update-ref", "refs/heads/develop", "refs/heads/main"],
    )
    .unwrap();
    run(&remote, &["symbolic-ref", "HEAD", "refs/heads/develop"]).unwrap();
    run(
        &repo.path,
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    )
    .unwrap();
    run(
        &repo.path,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    )
    .unwrap();
    for _ in 0..3 {
        let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
            panic!()
        };
        assert_eq!(status.default_branch.as_deref(), Some("develop"));
    }
    assert_eq!(
        std::fs::read(repo.path.join(".git/remote-calls")).unwrap(),
        b"x"
    );
}

#[test]
fn status_reuses_remote_default_branch_until_remote_configuration_changes() {
    let repo = Repo::new(true);
    let remote = traced_remote(&repo);
    for _ in 0..3 {
        let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
            panic!()
        };
        assert_eq!(status.default_branch.as_deref(), Some("main"));
    }
    assert_eq!(
        std::fs::read(repo.path.join(".git/remote-calls")).unwrap(),
        b"x"
    );
    run(
        &remote,
        &["update-ref", "refs/heads/develop", "refs/heads/main"],
    )
    .unwrap();
    run(&remote, &["symbolic-ref", "HEAD", "refs/heads/develop"]).unwrap();
    run(
        &repo.path,
        &[
            "remote",
            "set-url",
            "origin",
            "ext::/bin/sh .git/remote-probe changed",
        ],
    )
    .unwrap();
    let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
        panic!()
    };
    assert_eq!(status.default_branch.as_deref(), Some("develop"));
    assert_eq!(
        std::fs::read(repo.path.join(".git/remote-calls")).unwrap(),
        b"xx"
    );
}

fn traced_remote(repo: &Repo) -> PathBuf {
    let remote = remote(repo);
    run(&repo.path, &["push", "origin", "HEAD:refs/heads/main"]).unwrap();
    std::fs::write(
        repo.path.join(".git/remote-probe"),
        "printf x >> .git/remote-calls\nexec git upload-pack .git/test-remote.git\n",
    )
    .unwrap();
    run(&repo.path, &["config", "protocol.ext.allow", "always"]).unwrap();
    run(
        &repo.path,
        &[
            "remote",
            "set-url",
            "origin",
            "ext::/bin/sh .git/remote-probe",
        ],
    )
    .unwrap();
    remote
}

#[test]
fn remote_default_branch_is_reported_and_used_for_pr_creation_without_a_local_branch() {
    let mut repo = Repo::new(true);
    fake_gh(&mut repo);
    let remote = remote(&repo);
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    run(&repo.path, &["push", "origin", "HEAD:refs/heads/develop"]).unwrap();
    run(&remote, &["symbolic-ref", "HEAD", "refs/heads/develop"]).unwrap();
    let GitReply::Status(status) = repo.git(GitAction::Status { local: true }).unwrap() else {
        panic!()
    };
    assert_eq!(status.default_branch.as_deref(), Some("develop"));
    assert!(
        !status
            .branches
            .iter()
            .any(|branch| branch.name == "develop")
    );
    pr(
        &repo,
        Pr::Create {
            title: "Feature".into(),
            body: String::new(),
            base_branch: None,
            draft: false,
        },
    )
    .unwrap();
    let calls = std::fs::read_to_string(repo.path.join(".git/gh-calls")).unwrap();
    assert!(calls.contains("--base\ndevelop\n"));
    run(
        &repo.path,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/develop",
        ],
    )
    .unwrap();
    std::fs::remove_dir_all(remote).unwrap();
    assert_eq!(
        details::default_branch(&repo.path).as_deref(),
        Some("develop")
    );
}

#[test]
fn pr_checkout_and_worktrees_fall_back_for_valid_heads_without_alphanumeric_characters() {
    for head in ["🚀", "!!!"] {
        let mut repo = Repo::new(true);
        fake_gh(&mut repo);
        let remote = remote(&repo);
        repo.git(GitAction::CreateBranch(head.into())).unwrap();
        std::fs::write(repo.path.join("from-pr"), head).unwrap();
        commit(&repo, "PR head");
        repo.git(GitAction::Push { set_upstream: true }).unwrap();
        repo.git(GitAction::SwitchBranch("main".into())).unwrap();
        run(
            &repo.path,
            &[
                "config",
                &format!("url.{}.insteadOf", remote.display()),
                "https://github.com/contributor/fork.git",
            ],
        )
        .unwrap();
        std::fs::write(repo.path.join(".git/gh-checkout"), serde_json::json!({
            "headRefName": head, "headRepository": {"name":"fork"}, "headRepositoryOwner":{"login":"contributor"}
        }).to_string()).unwrap();
        pr(&repo, Pr::Checkout { number: 42 }).unwrap();
        assert_eq!(repo.summary().branch.as_deref(), Some("pr/42/head"));
        assert_eq!(
            std::fs::read_to_string(repo.path.join("from-pr")).unwrap(),
            head
        );
        repo.git(GitAction::SwitchBranch("main".into())).unwrap();
        let GitReply::Project(project) = repo
            .git(GitAction::Worktree(WorktreeIntent {
                operation: OperationId::new(),
                action: WorktreeAction::CheckoutPullRequest {
                    project: ProjectId::new(),
                    directory: server_path(&repo.path.join("pr-worktree")),
                    number: 42,
                },
            }))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(project.name, "pr/42/head");
        assert_eq!(
            std::fs::read_to_string(path(&project.directory).join("from-pr")).unwrap(),
            head
        );
    }
}
