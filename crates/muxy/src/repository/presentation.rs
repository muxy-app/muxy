use super::{LoadState, PullRequestLoadState, RepositoryState};
use muxy_api::repository::{
    PullRequestChecksStatus, PullRequestMergeState, PullRequestMergeable, PullRequestState,
    RepositorySummary,
};
use muxy_core::repository_ai::{RepositoryAiAction, RepositoryAiPreferences};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryControlKind {
    Branch,
    Changes,
    CommitAi,
    CreatePullRequestAi,
    PullRequest,
    RepositoryUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryControlTone {
    Default,
    Clean,
    Dirty,
    Danger,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryControl {
    pub kind: RepositoryControlKind,
    pub label: String,
    pub tooltip: String,
    pub enabled: bool,
    pub tone: RepositoryControlTone,
}

pub(crate) fn repository_controls(
    state: &RepositoryState,
    preferences: &RepositoryAiPreferences,
    mutation_busy: bool,
) -> Vec<RepositoryControl> {
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
        Some(summary) => ready_controls(summary, state, preferences, mutation_busy),
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
        PullRequestLoadState::NoPullRequest => {
            let (enabled, tooltip) = ai_action_enabled(
                state,
                preferences,
                summary,
                mutation_busy,
                RepositoryAiAction::CreatePullRequest,
            );
            control(
                RepositoryControlKind::CreatePullRequestAi,
                if state.ai
                    == super::RepositoryAiRunState::Running(RepositoryAiAction::CreatePullRequest)
                {
                    "Creating PR…"
                } else {
                    "Create PR"
                },
                tooltip,
                enabled,
            )
        }
        PullRequestLoadState::Unavailable(error) => control_with_tone(
            RepositoryControlKind::PullRequest,
            "Retry PR",
            error,
            !mutation_busy,
            RepositoryControlTone::Danger,
        ),
        PullRequestLoadState::Found { info, .. } => control_with_tone(
            RepositoryControlKind::PullRequest,
            format!("#{}", info.number),
            format!("Pull request #{}", info.number),
            !mutation_busy,
            pull_request_tone(info),
        ),
    });
    controls
}

fn pull_request_tone(info: &muxy_api::repository::PullRequestInfo) -> RepositoryControlTone {
    if info.state == PullRequestState::Closed
        || info.mergeable == PullRequestMergeable::Conflicting
        || info.merge_state == PullRequestMergeState::Dirty
        || info.checks.status == PullRequestChecksStatus::Failure
    {
        RepositoryControlTone::Danger
    } else if info.state == PullRequestState::Open
        && (info.is_draft
            || matches!(
                info.merge_state,
                PullRequestMergeState::Behind
                    | PullRequestMergeState::Blocked
                    | PullRequestMergeState::Draft
                    | PullRequestMergeState::Unstable
            )
            || info.checks.status == PullRequestChecksStatus::Pending)
    {
        RepositoryControlTone::Dirty
    } else {
        RepositoryControlTone::Clean
    }
}

fn ready_controls(
    summary: &RepositorySummary,
    state: &RepositoryState,
    preferences: &RepositoryAiPreferences,
    mutation_busy: bool,
) -> Vec<RepositoryControl> {
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
            !mutation_busy,
        ),
        control_with_tone(
            RepositoryControlKind::Changes,
            changes_label,
            changes_tooltip,
            !mutation_busy,
            if summary.conflicted_count > 0 {
                RepositoryControlTone::Danger
            } else if summary.is_dirty() {
                RepositoryControlTone::Dirty
            } else {
                RepositoryControlTone::Clean
            },
        ),
        {
            let (enabled, tooltip) = ai_action_enabled(
                state,
                preferences,
                Some(summary),
                mutation_busy,
                RepositoryAiAction::Commit,
            );
            control(
                RepositoryControlKind::CommitAi,
                if state.ai == super::RepositoryAiRunState::Running(RepositoryAiAction::Commit) {
                    "Committing…"
                } else {
                    "Commit"
                },
                tooltip,
                enabled,
            )
        },
    ]
}

fn ai_action_enabled(
    state: &RepositoryState,
    preferences: &RepositoryAiPreferences,
    summary: Option<&RepositorySummary>,
    mutation_busy: bool,
    action: RepositoryAiAction,
) -> (bool, String) {
    if state.ai == super::RepositoryAiRunState::Running(action) {
        return (true, "Cancel the running repository AI workflow".to_owned());
    }
    if mutation_busy || !matches!(state.ai, super::RepositoryAiRunState::Idle) {
        return (false, "Another repository operation is running".to_owned());
    }
    let Some(summary) = summary else {
        return (false, "Repository is loading".to_owned());
    };
    if summary.is_detached || summary.branch.is_empty() {
        return (false, "Repository AI actions require a branch".to_owned());
    }
    if !summary.is_dirty() {
        return (false, "There are no changes to commit".to_owned());
    }
    match &state.providers {
        LoadState::Ready(inventory) => match inventory.resolve_action(preferences, action) {
            Ok(provider) => (
                true,
                format!(
                    "Use {} for this repository action",
                    provider.descriptor.display_name
                ),
            ),
            Err(error) => (false, error.to_string()),
        },
        LoadState::Error(error) => (false, error.clone()),
        LoadState::Idle | LoadState::Loading => {
            (false, "Finding installed AI providers".to_owned())
        }
    }
}

fn control(
    kind: RepositoryControlKind,
    label: impl Into<String>,
    tooltip: impl Into<String>,
    enabled: bool,
) -> RepositoryControl {
    control_with_tone(
        kind,
        label,
        tooltip,
        enabled,
        RepositoryControlTone::Default,
    )
}

fn control_with_tone(
    kind: RepositoryControlKind,
    label: impl Into<String>,
    tooltip: impl Into<String>,
    enabled: bool,
    tone: RepositoryControlTone,
) -> RepositoryControl {
    RepositoryControl {
        kind,
        label: label.into(),
        tooltip: tooltip.into(),
        enabled,
        tone,
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

    fn control_of(
        controls: &[RepositoryControl],
        kind: RepositoryControlKind,
    ) -> &RepositoryControl {
        controls
            .iter()
            .find(|control| control.kind == kind)
            .unwrap()
    }

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

    fn controls(state: &RepositoryState) -> Vec<RepositoryControl> {
        repository_controls(state, &RepositoryAiPreferences::default(), false)
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
        let loading = controls(&repository);
        assert_eq!(loading[0].kind, RepositoryControlKind::Branch);
        assert_eq!(loading[0].label, "Branch");

        repository.summary = LoadState::Ready(summary(
            "main",
            RepositoryHead::Commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
        ));
        let clean = controls(&repository);
        assert_eq!(clean[0].label, "main");
        assert_eq!(clean[1].label, "Clean");
        assert_eq!(clean[1].tone, RepositoryControlTone::Clean);
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
        let dirty = controls(&repository);
        assert_eq!(dirty[1].label, "4 Changes");
        assert_eq!(dirty[1].tone, RepositoryControlTone::Dirty);
        assert!(dirty[0].tooltip.contains("2 ahead"));
        assert!(dirty[0].tooltip.contains("3 behind"));

        let LoadState::Ready(current_summary) = &mut repository.summary else {
            panic!("summary")
        };
        current_summary.conflicted_count = 1;
        assert_eq!(controls(&repository)[1].tone, RepositoryControlTone::Danger);

        let LoadState::Ready(current_summary) = &mut repository.summary else {
            panic!("summary")
        };
        current_summary.is_detached = true;
        let detached = controls(&repository);
        assert_eq!(detached[0].label, "Detached aaaaaaa");

        repository.summary = LoadState::Ready(summary("topic", RepositoryHead::Unborn));
        assert_eq!(controls(&repository)[0].label, "topic");
    }

    #[test]
    fn repository_presentation_models_errors_and_every_pull_request_state() {
        let mut repository = state();
        repository.summary = LoadState::Error("failed".to_owned());
        let failed = controls(&repository);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].kind, RepositoryControlKind::RepositoryUnavailable);
        assert!(!failed[0].enabled);

        repository.summary = LoadState::Ready(summary("main", RepositoryHead::Unborn));
        repository.pull_request = PullRequestLoadState::Loading;
        assert!(
            controls(&repository)
                .iter()
                .any(|control| control.kind == RepositoryControlKind::PullRequest
                    && !control.enabled)
        );

        repository.pull_request = PullRequestLoadState::NoPullRequest;
        assert!(
            controls(&repository)
                .iter()
                .any(|control| control.kind == RepositoryControlKind::CreatePullRequestAi)
        );

        repository.pull_request = PullRequestLoadState::Unavailable("login required".to_owned());
        let unavailable = controls(&repository);
        let unavailable = unavailable
            .iter()
            .find(|control| control.kind == RepositoryControlKind::PullRequest)
            .unwrap();
        assert_eq!(unavailable.label, "Retry PR");
        assert!(unavailable.enabled);
        assert_eq!(unavailable.tone, RepositoryControlTone::Danger);

        repository.pull_request = PullRequestLoadState::Found {
            info: Box::new(PullRequestInfo {
                url: ValidatedExternalUrl::try_from(
                    "https://github.com/muxy/repo/pull/42".to_owned(),
                )
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
            }),
            resolved: None,
        };
        let found = controls(&repository);
        let found = found
            .iter()
            .find(|control| control.kind == RepositoryControlKind::PullRequest)
            .unwrap();
        assert_eq!(found.label, "#42");
        assert_eq!(found.tone, RepositoryControlTone::Clean);

        repository.key = None;
        assert!(controls(&repository).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn repository_ai_availability_tracks_truthful_repository_provider_and_run_state() {
        use muxy_api::execution_environment::ExecutionEnvironment;
        use std::ffi::OsString;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        let executable = bin.join("codex");
        fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let environment =
            ExecutionEnvironment::fallback([(OsString::from("PATH"), OsString::new())]);
        let inventory =
            muxy_api::repository::ProviderInventory::discover(&environment, temp.path(), false);
        let mut repository = state();
        repository.summary =
            LoadState::Ready(summary("topic", RepositoryHead::Commit("a".repeat(40))));
        repository.pull_request = PullRequestLoadState::NoPullRequest;
        repository.providers = LoadState::Ready(inventory);
        let mut preferences = RepositoryAiPreferences::default();
        preferences.commit.provider = "codex".to_owned();
        preferences.create_pull_request.provider = "codex".to_owned();

        let clean = repository_controls(&repository, &preferences, false);
        assert!(!control_of(&clean, RepositoryControlKind::CommitAi).enabled);
        assert!(!control_of(&clean, RepositoryControlKind::CreatePullRequestAi).enabled);

        let LoadState::Ready(summary) = &mut repository.summary else {
            panic!("summary")
        };
        summary.changed_count = 1;
        summary.unstaged_count = 1;
        let available = repository_controls(&repository, &preferences, false);
        assert!(control_of(&available, RepositoryControlKind::CommitAi).enabled);
        assert!(control_of(&available, RepositoryControlKind::CreatePullRequestAi).enabled);

        preferences.commit.provider = "claude".to_owned();
        let missing = repository_controls(&repository, &preferences, false);
        let commit = control_of(&missing, RepositoryControlKind::CommitAi);
        assert!(!commit.enabled);
        assert!(commit.tooltip.contains("not installed"));
        assert!(control_of(&missing, RepositoryControlKind::CreatePullRequestAi).enabled);

        preferences.commit.provider = "codex".to_owned();
        let busy = repository_controls(&repository, &preferences, true);
        for kind in [
            RepositoryControlKind::Branch,
            RepositoryControlKind::Changes,
            RepositoryControlKind::CommitAi,
            RepositoryControlKind::CreatePullRequestAi,
        ] {
            assert!(!control_of(&busy, kind).enabled);
        }

        repository.ai =
            crate::repository::RepositoryAiRunState::Running(RepositoryAiAction::Commit);
        let running = repository_controls(&repository, &preferences, true);
        assert_eq!(
            control_of(&running, RepositoryControlKind::CommitAi).label,
            "Committing…"
        );
        assert!(control_of(&running, RepositoryControlKind::CommitAi).enabled);
        assert!(!control_of(&running, RepositoryControlKind::Branch).enabled);

        repository.ai = crate::repository::RepositoryAiRunState::Idle;
        let LoadState::Ready(summary) = &mut repository.summary else {
            panic!("summary")
        };
        summary.is_detached = true;
        let detached = repository_controls(&repository, &preferences, false);
        assert!(!control_of(&detached, RepositoryControlKind::CommitAi).enabled);
        assert!(!control_of(&detached, RepositoryControlKind::CreatePullRequestAi).enabled);

        repository.summary = LoadState::Loading;
        repository.pull_request = PullRequestLoadState::Loading;
        let loading = repository_controls(&repository, &preferences, false);
        assert!(!control_of(&loading, RepositoryControlKind::CommitAi).enabled);
        assert!(
            loading
                .iter()
                .all(|control| control.kind != RepositoryControlKind::CreatePullRequestAi)
        );
    }
}
