use super::git::registered_current_project;
use super::*;
use crate::ai::PROVIDERS;
use crate::repository_actions::Action;
use muxy_protocol::{GitAction, GitPullRequestAction, GitReply, GitRequest, GitSummary};

fn feature_changes() -> GitSummary {
    GitSummary {
        branch: Some("feature".into()),
        head: Some("abc".into()),
        changed: 1,
        ..GitSummary::default()
    }
}

fn load(model: &mut AppModel, project: ProjectId, summary: GitSummary, cx: &mut Context<AppModel>) {
    model.receive_git(
        &GitRequest {
            project,
            action: GitAction::Summary,
        },
        Ok(GitReply::Summary(Some(summary))),
        cx,
    );
    model.receive_git(
        &GitRequest {
            project,
            action: GitAction::Branches,
        },
        Ok(GitReply::Branches(vec![muxy_protocol::GitBranch {
            name: "main".into(),
            current: false,
            checked_out: false,
            default: true,
        }])),
        cx,
    );
    model.receive_git(
        &GitRequest {
            project,
            action: GitAction::PullRequest(GitPullRequestAction::Info),
        },
        Ok(GitReply::PullRequest(None)),
        cx,
    );
}

#[gpui::test]
fn confirmation_does_not_start_work_and_cancel_is_side_effect_free(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.ai.installed = vec![PROVIDERS[0]];
        load(model, project, feature_changes(), cx);
    });
    cx.run_until_parked();
    requests.try_iter().for_each(drop);
    for action in [Action::Commit, Action::CreatePullRequest] {
        cx.update(|window, cx| {
            view.update(cx, |model, cx| model.open_ai_action(action, window, cx));
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        view.read_with(cx, |model, _| {
            assert!(model.overlay.is_none());
            assert!(model.ai.confirmation.is_some());
            assert!(!model.ai.running(project));
        });
        assert!(requests.try_iter().next().is_none());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.ai.confirmation.is_none());
            assert!(!model.ai.running(project));
        });
        assert!(requests.try_iter().next().is_none());
    }
}

#[gpui::test]
fn confirming_a_stale_branch_does_not_start_work(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::Commit, window, cx);
        });
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        load(
            model,
            project,
            GitSummary {
                branch: Some("other".into()),
                ..feature_changes()
            },
            cx,
        );
    });
    requests.try_iter().for_each(drop);
    cx.simulate_prompt_answer("Commit and Push");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.ai.running(project));
        assert!(model.ai.confirmation.is_none());
        assert!(model.overlay.is_none());
        assert!(
            model
                .error
                .as_deref()
                .unwrap()
                .contains("no longer available")
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::ExtensionClient(_)))
    );
}
