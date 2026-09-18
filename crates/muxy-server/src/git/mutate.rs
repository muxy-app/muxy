use super::{Result, command, error, read, run, text, validate_branch};
use muxy_protocol::{GitAction, GitReply};
use std::path::Path;

pub(super) fn apply(repository: &Path, action: &GitAction) -> Result<GitReply> {
    match action {
        GitAction::Commit { message, stage_all } => {
            if *stage_all {
                run(repository, &["add", "-A"])?;
            }
            run(repository, &["commit", "-m", message.trim()])?;
            return Ok(GitReply::Commit(text(&run(
                repository,
                &["rev-parse", "HEAD"],
            )?)?));
        }
        GitAction::Push { set_upstream } => push(repository, *set_upstream)?,
        GitAction::Pull => {
            command::network(repository, &["pull"])?;
        }
        GitAction::Checkout(hash) => {
            run(repository, &["checkout", "--detach", hash])?;
        }
        GitAction::CherryPick(hash) => {
            run(repository, &["cherry-pick", hash])?;
        }
        GitAction::Revert(hash) => {
            run(repository, &["revert", "--no-commit", hash])?;
        }
        GitAction::DeleteLocalBranch { name, force } => {
            validate_branch(repository, name)?;
            run(
                repository,
                &["branch", if *force { "-D" } else { "-d" }, "--", name],
            )?;
        }
        GitAction::DeleteRemoteBranch(name) => {
            validate_branch(repository, name)?;
            command::network(
                repository,
                &["push", "--delete", "origin", &format!("refs/heads/{name}")],
            )?;
        }
        GitAction::CreateTag { name, hash } => {
            run(
                repository,
                &["check-ref-format", &format!("refs/tags/{name}")],
            )?;
            run(repository, &["tag", "--", name, hash])?;
        }
        _ => return Err(error("Unsupported Git mutation")),
    }
    Ok(GitReply::Done)
}

pub(super) fn push(repository: &Path, set_upstream: bool) -> Result<()> {
    if !set_upstream {
        let summary = read::summary(repository)?;
        if let (Some(branch), Some(upstream)) = (&summary.branch, &summary.upstream)
            && run(
                repository,
                &[
                    "config",
                    "--get",
                    &format!("branch.{branch}.muxy-pr-number"),
                ],
            )
            .is_ok()
            && let Some((remote, branch)) = upstream.split_once('/')
        {
            command::network(
                repository,
                &["push", "--", remote, &format!("HEAD:refs/heads/{branch}")],
            )?;
            return Ok(());
        }
        match command::network(repository, &["push"]) {
            Ok(_) => return Ok(()),
            Err(error) if error.message().contains("has no upstream branch") => (),
            Err(error) => return Err(error),
        }
    }
    let branch = read::summary(repository)?
        .branch
        .ok_or_else(|| error("Cannot set upstream for a detached HEAD"))?;
    validate_branch(repository, &branch)?;
    command::network(repository, &["push", "--set-upstream", "origin", &branch])?;
    Ok(())
}
