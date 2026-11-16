use super::{Result, command, error, read, run, same_canonical, text, validate_branch};
use muxy_protocol::{GitBaseSwitch, GitPushDestination};
use std::path::Path;

pub(super) fn destination(repository: &Path, branch: &str) -> Result<Option<GitPushDestination>> {
    let upstream = upstream(repository, branch);
    if is_pull_request_checkout(repository, branch) {
        return Ok(upstream);
    }
    if let Some(upstream) = upstream.filter(|upstream| upstream.branch == branch) {
        return Ok(Some(upstream));
    }
    if config(repository, "remote.origin.url").is_none() {
        return Ok(None);
    }
    validate_branch(repository, branch)?;
    Ok(Some(GitPushDestination {
        remote: "origin".into(),
        branch: branch.to_owned(),
    }))
}

pub(super) fn publish(
    repository: &Path,
    branch: &str,
    expected: &GitPushDestination,
) -> Result<()> {
    if read::summary(repository)?.branch.as_deref() != Some(branch) {
        return Err(error(
            "The current branch changed; review the changes again",
        ));
    }
    let destination =
        destination(repository, branch)?.ok_or_else(|| error("No remote to push to"))?;
    if destination != *expected {
        return Err(error(format!(
            "The push destination changed to {}/{}; review the changes again",
            destination.remote, destination.branch
        )));
    }
    validate_branch(repository, &destination.branch)?;
    let refspec = format!("HEAD:refs/heads/{}", destination.branch);
    let mut args = vec!["push"];
    if upstream(repository, branch).as_ref() != Some(&destination) {
        args.push("--set-upstream");
    }
    args.extend(["--", &destination.remote, &refspec]);
    command::network(repository, &args).map(|_| ())
}

pub(super) fn switch_to_base(repository: &Path, base: &str) -> Result<GitBaseSwitch> {
    validate_branch(repository, base)?;
    if read::summary(repository)?.branch.as_deref() != Some(base) {
        let here = muxy_core::files::canonicalize(repository).map_err(error)?;
        let elsewhere = read::worktrees(repository)?.into_iter().find(|worktree| {
            worktree.branch.as_deref() == Some(base)
                && super::path(&worktree.directory)
                    .and_then(|directory| muxy_core::files::canonicalize(&directory).map_err(error))
                    .is_ok()
                && !same_canonical(&worktree.directory, &here)
        });
        if let Some(worktree) = elsewhere {
            return Ok(GitBaseSwitch::CheckedOutElsewhere(worktree.directory));
        }
        run(repository, &["switch", "--", base])
            .map_err(|cause| error(format!("Couldn't switch to {base}: {}", cause.message())))?;
    }
    command::network(repository, &["pull", "--ff-only"]).map_err(|cause| {
        error(format!(
            "Switched to {base}, but couldn't fast-forward it: {}",
            cause.message()
        ))
    })?;
    Ok(GitBaseSwitch::Updated)
}

pub(super) fn is_pull_request_checkout(repository: &Path, branch: &str) -> bool {
    config(repository, &format!("branch.{branch}.muxy-pr-number")).is_some()
}

fn upstream(repository: &Path, branch: &str) -> Option<GitPushDestination> {
    let remote = config(repository, &format!("branch.{branch}.remote")).filter(|r| r != ".")?;
    let merge = config(repository, &format!("branch.{branch}.merge"))?;
    Some(GitPushDestination {
        remote,
        branch: merge.strip_prefix("refs/heads/")?.to_owned(),
    })
}

fn config(repository: &Path, key: &str) -> Option<String> {
    run(repository, &["config", "--get", key])
        .ok()
        .and_then(|bytes| text(&bytes).ok())
        .filter(|value| !value.is_empty())
}
