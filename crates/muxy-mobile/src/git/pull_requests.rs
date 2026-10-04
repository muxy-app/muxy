//! Pull requests on GitHub, which the computer reaches with its `gh` command.

use muxy_protocol::{GitAction, GitPullRequestAction, GitReply};

use super::GitRepository;
use super::records::{GitMergeMethod, GitPullRequest, GitPullRequestFilter, GitRawDiff, collect};
use super::repository::{raw_diff, unexpected};
use crate::MobileError;

#[uniffi::export]
impl GitRepository {
    /// The current branch's open pull request, including drafts, if any.
    pub fn pull_request(&self) -> Result<Option<GitPullRequest>, MobileError> {
        match self.request(GitAction::PullRequest(GitPullRequestAction::Info))? {
            GitReply::PullRequest(pull_request) => Ok(pull_request.map(|found| (*found).into())),
            other => Err(unexpected(other)),
        }
    }

    /// Only the current branch's open pull request number, which is quicker to find.
    pub fn pull_request_number(&self) -> Result<Option<u64>, MobileError> {
        match self.request(GitAction::PullRequest(GitPullRequestAction::Number))? {
            GitReply::PullRequestNumber(number) => Ok(number),
            other => Err(unexpected(other)),
        }
    }

    /// Up to `limit` pull requests, at most 200. `checks` also reads their CI
    /// results, which takes longer.
    pub fn pull_requests(
        &self,
        filter: GitPullRequestFilter,
        limit: u32,
        checks: bool,
    ) -> Result<Vec<GitPullRequest>, MobileError> {
        let action = GitAction::PullRequest(GitPullRequestAction::List {
            filter: filter.into(),
            limit,
            checks,
        });
        match self.request(action)? {
            GitReply::PullRequests(pull_requests) => Ok(collect(pull_requests)),
            other => Err(unexpected(other)),
        }
    }

    pub fn pull_request_diff(
        &self,
        number: u64,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        let action = GitAction::PullRequest(GitPullRequestAction::Diff { number, line_limit });
        raw_diff(self.request(action)?)
    }

    /// Opens a pull request for the current branch, against the repository's
    /// default branch when `base_branch` is `None`.
    pub fn create_pull_request(
        &self,
        title: String,
        body: String,
        base_branch: Option<String>,
        draft: bool,
    ) -> Result<GitPullRequest, MobileError> {
        let action = GitAction::PullRequest(GitPullRequestAction::Create {
            title,
            body,
            base_branch,
            draft,
        });
        match self.request(action)? {
            GitReply::PullRequest(Some(created)) => Ok((*created).into()),
            other => Err(unexpected(other)),
        }
    }

    /// Refuses the merge when the pull request's head moved past `expected_head`.
    pub fn merge_pull_request(
        &self,
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        expected_head: Option<String>,
    ) -> Result<(), MobileError> {
        self.run(GitAction::PullRequest(GitPullRequestAction::Merge {
            number,
            method: method.into(),
            delete_branch,
            expected_head,
        }))
    }

    pub fn close_pull_request(&self, number: u64) -> Result<(), MobileError> {
        self.run(GitAction::PullRequest(GitPullRequestAction::Close {
            number,
        }))
    }

    /// Switches this project to the pull request's branch.
    pub fn checkout_pull_request(&self, number: u64) -> Result<(), MobileError> {
        self.run(GitAction::PullRequest(GitPullRequestAction::Checkout {
            number,
        }))
    }

    /// Merges the base branch into the checked-out pull request branch and
    /// pushes it, if the pull request's head is still `expected_head`.
    pub fn update_pull_request_branch(
        &self,
        number: u64,
        expected_head: String,
    ) -> Result<(), MobileError> {
        self.run(GitAction::PullRequest(GitPullRequestAction::UpdateBranch {
            number,
            expected_head,
        }))
    }
}
