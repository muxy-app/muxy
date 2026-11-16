use std::path::PathBuf;

use muxy_protocol::{
    ErrorCode, GitAction, GitBaseSwitch, GitPushDestination, GitReply, GitRequest,
};

use super::Repository;
use crate::git::{Result, branches, run, text};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn destination(remote: &str, branch: &str) -> GitPushDestination {
    GitPushDestination {
        remote: remote.into(),
        branch: branch.into(),
    }
}

fn request(repository: &Repository, action: GitAction) -> Result<GitReply> {
    repository.registry.git(&GitRequest {
        project: repository.project,
        action,
    })
}

fn local_origin(repository: &Repository) -> TestResult<PathBuf> {
    let remote = repository.path.join("target/origin.git");
    let path = remote.to_str().ok_or("non-Unicode test path")?;
    run(&repository.path, &["init", "--bare", "--quiet", path])?;
    run(&repository.path, &["remote", "set-url", "origin", path])?;
    Ok(remote)
}

fn commit(repository: &Repository) -> Result<Vec<u8>> {
    run(
        &repository.path,
        &[
            "-c",
            "user.name=Muxy",
            "-c",
            "user.email=muxy@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Next",
        ],
    )?;
    run(&repository.path, &["rev-parse", "HEAD"])
}

#[test]
fn destinations_distinguish_matching_upstreams_and_pull_request_checkouts() -> TestResult {
    for (remote, merge, marker, expected) in [
        (
            "fork",
            "refs/heads/main",
            None,
            Some(destination("fork", "main")),
        ),
        (
            "fork",
            "refs/heads/topic",
            None,
            Some(destination("origin", "main")),
        ),
        (
            "fork",
            "refs/heads/topic",
            Some("7"),
            Some(destination("fork", "topic")),
        ),
        (
            ".",
            "refs/heads/main",
            None,
            Some(destination("origin", "main")),
        ),
        (".", "refs/heads/main", Some("7"), None),
        ("fork", "refs/tags/main", Some("7"), None),
    ] {
        let repository = Repository::new();
        let path = &repository.path;
        run(path, &["config", "branch.main.remote", remote])?;
        run(path, &["config", "branch.main.merge", merge])?;
        if let Some(marker) = marker {
            run(path, &["config", "branch.main.muxy-pr-number", marker])?;
        }
        assert_eq!(branches::destination(path, "main")?, expected);
    }
    Ok(())
}

#[test]
fn destinations_require_a_remote_and_do_not_redirect_unconfigured_pr_checkouts() -> TestResult {
    let repository = Repository::new();
    let path = &repository.path;
    assert_eq!(
        branches::destination(path, "main")?,
        Some(destination("origin", "main"))
    );
    run(path, &["remote", "remove", "origin"])?;
    assert_eq!(branches::destination(path, "main")?, None);
    run(path, &["config", "branch.main.remote", "fork"])?;
    run(path, &["config", "branch.main.merge", "refs/heads/main"])?;
    assert_eq!(
        branches::destination(path, "main")?,
        Some(destination("fork", "main"))
    );
    run(path, &["config", "--remove-section", "branch.main"])?;
    run(
        path,
        &["remote", "add", "origin", "/nonexistent/muxy/origin.git"],
    )?;
    run(path, &["config", "branch.main.muxy-pr-number", "7"])?;
    assert_eq!(branches::destination(path, "main")?, None);
    Ok(())
}

#[test]
fn publishing_rejects_stale_requests_before_updating_the_remote() -> TestResult {
    let repository = Repository::new();
    let remote = local_origin(&repository)?;
    for (branch, destination, message) in [
        (
            "old",
            destination("origin", "old"),
            "The current branch changed; review the changes again",
        ),
        (
            "main",
            destination("fork", "main"),
            "The push destination changed to origin/main; review the changes again",
        ),
    ] {
        let error = request(
            &repository,
            GitAction::PublishBranch {
                branch: branch.into(),
                destination,
            },
        )
        .err()
        .ok_or("stale publish request was accepted")?;
        assert_eq!(error.code(), ErrorCode::BadRequest);
        assert_eq!(error.message(), message);
        assert!(run(&remote, &["for-each-ref"])?.is_empty());
    }
    assert_eq!(
        request(
            &repository,
            GitAction::PublishBranch {
                branch: "main".into(),
                destination: destination("origin", "main"),
            }
        )?,
        GitReply::Done,
    );
    assert_eq!(
        run(&remote, &["rev-parse", "refs/heads/main"])?,
        run(&repository.path, &["rev-parse", "HEAD"])?
    );
    assert_eq!(
        text(&run(
            &repository.path,
            &["rev-parse", "--abbrev-ref", "@{upstream}"]
        )?)?,
        "origin/main"
    );
    let next = commit(&repository)?;
    request(
        &repository,
        GitAction::PublishBranch {
            branch: "main".into(),
            destination: destination("origin", "main"),
        },
    )?;
    assert_eq!(run(&remote, &["rev-parse", "refs/heads/main"])?, next);
    Ok(())
}

#[test]
fn publishing_a_pr_checkout_updates_its_original_remote_branch() -> TestResult {
    let repository = Repository::new();
    let remote = local_origin(&repository)?;
    let path = &repository.path;
    run(path, &["config", "branch.main.remote", "origin"])?;
    run(path, &["config", "branch.main.merge", "refs/heads/topic"])?;
    run(path, &["config", "branch.main.muxy-pr-number", "7"])?;
    request(
        &repository,
        GitAction::PublishBranch {
            branch: "main".into(),
            destination: destination("origin", "topic"),
        },
    )?;
    assert_eq!(
        run(&remote, &["rev-parse", "refs/heads/topic"])?,
        run(path, &["rev-parse", "HEAD"])?
    );
    assert!(run(&remote, &["show-ref", "--verify", "refs/heads/main"]).is_err());
    assert_eq!(
        text(&run(path, &["config", "branch.main.merge"])?)?,
        "refs/heads/topic"
    );
    Ok(())
}

#[test]
fn returning_to_base_respects_other_worktrees_then_fast_forwards() -> TestResult {
    let repository = Repository::new();
    local_origin(&repository)?;
    let path = &repository.path;
    run(path, &["push", "--set-upstream", "origin", "main"])?;
    run(path, &["switch", "-c", "topic"])?;
    let next = commit(&repository)?;
    run(path, &["push", "origin", "HEAD:refs/heads/main"])?;
    let worktree = path.join("target").join("base");
    let directory = worktree.to_str().ok_or("non-Unicode test path")?;
    run(path, &["worktree", "add", directory, "main"])?;
    assert_eq!(
        request(&repository, GitAction::SwitchToBase("main".into()))?,
        GitReply::BaseSwitch(GitBaseSwitch::CheckedOutElsewhere(worktree.clone().into())),
    );
    assert_eq!(
        text(&run(path, &["symbolic-ref", "--short", "HEAD"])?)?,
        "topic"
    );
    run(path, &["worktree", "remove", directory])?;
    assert_eq!(
        request(&repository, GitAction::SwitchToBase("main".into()))?,
        GitReply::BaseSwitch(GitBaseSwitch::Updated),
    );
    assert_eq!(
        text(&run(path, &["symbolic-ref", "--short", "HEAD"])?)?,
        "main"
    );
    assert_eq!(run(path, &["rev-parse", "HEAD"])?, next);
    Ok(())
}

#[test]
fn returning_to_base_reports_whether_switching_or_pulling_failed() -> TestResult {
    let repository = Repository::new();
    let path = &repository.path;
    run(path, &["switch", "-c", "topic"])?;
    let error = request(&repository, GitAction::SwitchToBase("missing".into()))
        .err()
        .ok_or("missing branch was accepted")?;
    assert_eq!(error.code(), ErrorCode::BadRequest);
    assert!(
        error.message().starts_with("Couldn't switch to missing: "),
        "{error}"
    );
    assert_eq!(
        text(&run(path, &["symbolic-ref", "--short", "HEAD"])?)?,
        "topic"
    );
    let error = request(&repository, GitAction::SwitchToBase("main".into()))
        .err()
        .ok_or("pull from a missing remote succeeded")?;
    assert_eq!(error.code(), ErrorCode::BadRequest);
    assert!(
        error
            .message()
            .starts_with("Switched to main, but couldn't fast-forward it: "),
        "{error}"
    );
    assert_eq!(
        text(&run(path, &["symbolic-ref", "--short", "HEAD"])?)?,
        "main"
    );
    Ok(())
}
