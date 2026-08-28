use super::{LoadState, PullRequestLoadState, RepositoryState};
use muxy_api::repository::RepositorySummary;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryControlKind {
    Branch,
    Changes,
    CommitAi,
    CreatePullRequestAi,
    PullRequest,
    RepositoryUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryControl {
    pub kind: RepositoryControlKind,
    pub label: String,
    pub tooltip: String,
    pub enabled: bool,
}

pub(crate) fn repository_controls(state: &RepositoryState) -> Vec<RepositoryControl> {
    if state.key.is_none() {
        return Vec::new();
    }
    let summary = match &state.summary {
        LoadState::Error(error) => {
            return vec![control(
                RepositoryControlKind::RepositoryUnavailable,
                "Repository unavailable",
                error,
                false,
            )];
        }
        LoadState::Ready(summary) => Some(summary),
        LoadState::Idle | LoadState::Loading => None,
    };
    let mut controls = match summary {
        Some(summary) => ready_controls(summary),
        None => vec![
            control(
                RepositoryControlKind::Branch,
                "Branch",
                "Loading branch",
                false,
            ),
            control(
                RepositoryControlKind::Changes,
                "Changes",
                "Loading changes",
                false,
            ),
            control(
                RepositoryControlKind::CommitAi,
                "Commit",
                "Repository is loading",
                false,
            ),
        ],
    };
    controls.push(match &state.pull_request {
        PullRequestLoadState::Idle => control(
            RepositoryControlKind::PullRequest,
            "Pull Request",
            "Pull request status is not loaded",
            false,
        ),
        PullRequestLoadState::Loading => control(
            RepositoryControlKind::PullRequest,
            "Pull Request",
            "Loading pull request",
            false,
        ),
        PullRequestLoadState::NoPullRequest => control(
            RepositoryControlKind::CreatePullRequestAi,
            "Create PR",
            "Create a pull request with AI",
            summary.is_some(),
        ),
        PullRequestLoadState::Unavailable(error) => control(
            RepositoryControlKind::PullRequest,
            "Pull Request",
            error,
            false,
        ),
        PullRequestLoadState::Found(pull_request) => control(
            RepositoryControlKind::PullRequest,
            format!("#{}", pull_request.number),
            format!("Pull request #{}", pull_request.number),
            true,
        ),
    });
    controls
}

fn ready_controls(summary: &RepositorySummary) -> Vec<RepositoryControl> {
    let branch_tooltip = if summary.is_detached {
        "Detached HEAD".to_owned()
    } else if let Some(upstream) = &summary.upstream {
        format!(
            "{upstream} · {} ahead · {} behind",
            summary.ahead, summary.behind
        )
    } else {
        "No upstream".to_owned()
    };
    let changes_label = match summary.changed_count {
        0 => "Clean".to_owned(),
        1 => "1 Change".to_owned(),
        count => format!("{count} Changes"),
    };
    let changes_tooltip = format!(
        "{} staged · {} unstaged · {} untracked · {} conflicted",
        summary.staged_count,
        summary.unstaged_count,
        summary.untracked_count,
        summary.conflicted_count
    );
    vec![
        control(
            RepositoryControlKind::Branch,
            summary.display_branch(),
            branch_tooltip,
            true,
        ),
        control(
            RepositoryControlKind::Changes,
            changes_label,
            changes_tooltip,
            true,
        ),
        control(
            RepositoryControlKind::CommitAi,
            "Commit",
            "Create a commit with AI",
            summary.is_dirty(),
        ),
    ]
}

fn control(
    kind: RepositoryControlKind,
    label: impl Into<String>,
    tooltip: impl Into<String>,
    enabled: bool,
) -> RepositoryControl {
    RepositoryControl {
        kind,
        label: label.into(),
        tooltip: tooltip.into(),
        enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{LoadState, PullRequestLoadState, RepositoryKey, RepositoryState};
    use muxy_api::repository::{
        PullRequestChecks, PullRequestChecksStatus, PullRequestInfo, PullRequestMergeState,
        PullRequestMergeable, PullRequestState, RepositoryHead, RepositorySummary,
        ValidatedExternalUrl,
    };
    use std::path::PathBuf;

    fn state() -> RepositoryState {
        RepositoryState {
            key: Some(RepositoryKey {
                project_id: "project".to_owned(),
                worktree_id: "primary".to_owned(),
                normalized_path: PathBuf::from("/repo"),
            }),
            ..RepositoryState::default()
        }
    }

    fn summary(branch: &str, head: RepositoryHead) -> RepositorySummary {
        RepositorySummary {
            branch: branch.to_owned(),
            head,
            is_detached: false,
            upstream: None,
            ahead: 0,
            behind: 0,
            changed_count: 0,
            staged_count: 0,
            unstaged_count: 0,
            untracked_count: 0,
            conflicted_count: 0,
        }
    }

    #[test]
    fn repository_presentation_models_loading_clean_dirty_detached_and_unborn() {
        let mut repository = state();
        repository.summary = LoadState::Loading;
        let loading = repository_controls(&repository);
        assert_eq!(loading[0].kind, RepositoryControlKind::Branch);
        assert_eq!(loading[0].label, "Branch");

        repository.summary = LoadState::Ready(summary(
            "main",
            RepositoryHead::Commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
        ));
        let clean = repository_controls(&repository);
        assert_eq!(clean[0].label, "main");
        assert_eq!(clean[1].label, "Clean");
        assert!(clean[0].tooltip.contains("No upstream"));

        let LoadState::Ready(current_summary) = &mut repository.summary else {
            panic!("summary")
        };
        current_summary.changed_count = 4;
        current_summary.staged_count = 2;
        current_summary.untracked_count = 1;
        current_summary.ahead = 2;
        current_summary.behind = 3;
        current_summary.upstream = Some("origin/main".to_owned());
        let dirty = repository_controls(&repository);
        assert_eq!(dirty[1].label, "4 Changes");
        assert!(dirty[0].tooltip.contains("2 ahead"));
        assert!(dirty[0].tooltip.contains("3 behind"));

        let LoadState::Ready(current_summary) = &mut repository.summary else {
            panic!("summary")
        };
        current_summary.is_detached = true;
        let detached = repository_controls(&repository);
        assert_eq!(detached[0].label, "Detached aaaaaaa");

        repository.summary = LoadState::Ready(summary("topic", RepositoryHead::Unborn));
        assert_eq!(repository_controls(&repository)[0].label, "topic");
    }

    #[test]
    fn repository_presentation_models_errors_and_every_pull_request_state() {
        let mut repository = state();
        repository.summary = LoadState::Error("failed".to_owned());
        let failed = repository_controls(&repository);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].kind, RepositoryControlKind::RepositoryUnavailable);
        assert!(!failed[0].enabled);

        repository.summary = LoadState::Ready(summary("main", RepositoryHead::Unborn));
        repository.pull_request = PullRequestLoadState::Loading;
        assert!(
            repository_controls(&repository)
                .iter()
                .any(|control| control.kind == RepositoryControlKind::PullRequest
                    && !control.enabled)
        );

        repository.pull_request = PullRequestLoadState::NoPullRequest;
        assert!(
            repository_controls(&repository)
                .iter()
                .any(|control| control.kind == RepositoryControlKind::CreatePullRequestAi)
        );

        repository.pull_request = PullRequestLoadState::Unavailable("login required".to_owned());
        assert!(
            repository_controls(&repository)
                .iter()
                .any(|control| control.tooltip.contains("login required"))
        );

        repository.pull_request = PullRequestLoadState::Found(Box::new(PullRequestInfo {
            url: ValidatedExternalUrl::try_from("https://github.com/muxy/repo/pull/42".to_owned())
                .unwrap(),
            number: 42,
            state: PullRequestState::Open,
            is_draft: false,
            base_branch: "main".to_owned(),
            mergeable: PullRequestMergeable::Mergeable,
            merge_state: PullRequestMergeState::Clean,
            checks: PullRequestChecks {
                status: PullRequestChecksStatus::Success,
                passing: 1,
                failing: 0,
                pending: 0,
                total: 1,
            },
            is_cross_repository: false,
            head_oid: "a".repeat(40),
            head_branch: "topic".to_owned(),
        }));
        let found = repository_controls(&repository);
        assert!(found.iter().any(|control| {
            control.kind == RepositoryControlKind::PullRequest && control.label == "#42"
        }));

        repository.key = None;
        assert!(repository_controls(&repository).is_empty());
    }
}
