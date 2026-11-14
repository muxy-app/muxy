//! Pull requests on GitHub, which the computer reaches with its `gh` command.

use super::GitRepository;
use super::records::{GitMergeMethod, GitPullRequest, GitPullRequestFilter, GitRawDiff, collect};
use crate::MobileError;

#[uniffi::export]
impl GitRepository {
    /// The current branch's open pull request, including drafts, if any.
    pub fn pull_request(&self) -> Result<Option<GitPullRequest>, MobileError> {
        Ok(self.client.git_pull_request(self.project)?.map(Into::into))
    }

    /// Only the current branch's open pull request number, which is quicker to find.
    pub fn pull_request_number(&self) -> Result<Option<u64>, MobileError> {
        Ok(self.client.git_pull_request_number(self.project)?)
    }

    /// Up to `limit` pull requests, at most 200. `checks` also reads their CI
    /// results, which takes longer.
    pub fn pull_requests(
        &self,
        filter: GitPullRequestFilter,
        limit: u32,
        checks: bool,
    ) -> Result<Vec<GitPullRequest>, MobileError> {
        Ok(collect(self.client.git_pull_requests(
            self.project,
            filter.into(),
            limit,
            checks,
        )?))
    }

    pub fn pull_request_diff(
        &self,
        number: u64,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        Ok(self
            .client
            .git_pull_request_diff(self.project, number, line_limit)?
            .into())
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
        Ok(self
            .client
            .git_create_pull_request(self.project, title, body, base_branch, draft)?
            .into())
    }

    /// Refuses the merge when the pull request's head moved past `expected_head`.
    pub fn merge_pull_request(
        &self,
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        expected_head: Option<String>,
    ) -> Result<(), MobileError> {
        Ok(self.client.git_merge_pull_request(
            self.project,
            number,
            method.into(),
            delete_branch,
            expected_head,
        )?)
    }

    pub fn close_pull_request(&self, number: u64) -> Result<(), MobileError> {
        Ok(self.client.git_close_pull_request(self.project, number)?)
    }

    /// Switches this project to the pull request's branch.
    pub fn checkout_pull_request(&self, number: u64) -> Result<(), MobileError> {
        Ok(self
            .client
            .git_checkout_pull_request(self.project, number)?)
    }

    /// Merges the base branch into the checked-out pull request branch and
    /// pushes it, if the pull request's head is still `expected_head`.
    pub fn update_pull_request_branch(
        &self,
        number: u64,
        expected_head: String,
    ) -> Result<(), MobileError> {
        Ok(self
            .client
            .git_update_pull_request_branch(self.project, number, expected_head)?)
    }
}
