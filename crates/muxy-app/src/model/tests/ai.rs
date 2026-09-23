use super::git::{open_pull_request, registered_current_project};
use super::*;
use crate::ai::{Cancellation, PROVIDERS};
use crate::model::ai::Availability;
use crate::repository_actions::Action;
use crate::views::git::AiStep;
use crate::views::overlays::Overlay;
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

fn describe(availability: Availability) -> String {
    match availability {
        Availability::Available(provider) => format!("available with {}", provider.id),
        Availability::Disabled(reason) => reason,
        Availability::Hidden => "hidden".into(),
    }
}

#[gpui::test]
fn ai_actions_say_why_they_cannot_run(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        load(model, project, feature_changes(), cx);
        assert_eq!(
            describe(model.ai_availability(Action::Commit)),
            "Install a supported AI provider CLI or choose one in Settings → AI."
        );
        model.ai.installed = vec![PROVIDERS[0]];
        assert_eq!(
            describe(model.ai_availability(Action::Commit)),
            "available with claude"
        );
        model
            .settings
            .ai
            .providers
            .insert("commit".into(), "codex".into());
        assert_eq!(
            describe(model.ai_availability(Action::Commit)),
            "Codex CLI is not installed. Choose another provider or install its CLI."
        );
        model.settings.ai.providers.clear();

        load(
            model,
            project,
            GitSummary {
                conflicted: 1,
                ..feature_changes()
            },
            cx,
        );
        assert_eq!(
            describe(model.ai_availability(Action::Commit)),
            "Resolve merge conflicts first"
        );

        load(
            model,
            project,
            GitSummary {
                changed: 0,
                ..feature_changes()
            },
            cx,
        );
        assert_eq!(
            describe(model.ai_availability(Action::Commit)),
            "The working tree is clean."
        );
        assert_eq!(
            describe(model.ai_availability(Action::CreatePullRequest)),
            "available with claude",
            "committed work on a feature branch can become a pull request"
        );

        load(
            model,
            project,
            GitSummary {
                branch: Some("main".into()),
                changed: 0,
                ..feature_changes()
            },
            cx,
        );
        assert_eq!(
            describe(model.ai_availability(Action::CreatePullRequest)),
            "The working tree is clean."
        );

        model.git.projects.entry(project).or_default().pull_request = Some(open_pull_request());
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::PullRequest(GitPullRequestAction::Info),
            },
            Ok(GitReply::PullRequest(Some(Box::new(open_pull_request())))),
            cx,
        );
        assert_eq!(
            describe(model.ai_availability(Action::CreatePullRequest)),
            "hidden"
        );
    });
}

#[gpui::test]
fn failed_preparation_closes_the_sheet_and_explains_it(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            requests.try_iter().for_each(drop);
            model.open_ai_action(Action::Commit, window, cx);
            assert!(matches!(model.overlay, Some(Overlay::AiAction(_))));
            assert!(model.ai.running(project));
            assert!(model.git.projects[&project].summary.is_some());
        });
    });
    requests.try_iter().for_each(drop);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(!model.ai.running(project));
        assert_eq!(
            model.error.as_deref(),
            Some("Couldn't start Commit and Push")
        );
    });
}

#[gpui::test]
fn closing_the_sheet_or_switching_projects_cancels_unapplied_work(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let other = state
        .projects()
        .iter()
        .find(|candidate| candidate.id != project && !candidate.home)
        .expect("second project")
        .id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::Commit, window, cx);
            model.dismiss_overlay(cx);
            assert!(!model.ai.running(project));

            model.open_ai_action(Action::Commit, window, cx);
            assert!(model.ai.running(project));
            model
                .state
                .select_project(other)
                .expect("select other project");
            model.sync_git(cx);
            assert!(model.overlay.is_none());
            assert!(!model.ai.running(project));
        });
    });
    requests.try_iter().for_each(drop);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            model.error.is_none(),
            "late results of cancelled work are ignored"
        );
    });
}

#[gpui::test]
fn opening_another_overlay_stops_the_draft(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let drafting = Cancellation::default();
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::Commit, window, cx);
            let Some(Overlay::AiAction(sheet)) = &mut model.overlay else {
                panic!("AI sheet");
            };
            sheet.step = AiStep::Drafting;
            sheet.drafting = Some(drafting.clone());
        });
    });

    cx.simulate_keystrokes("cmd-shift-p");

    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Commands { .. })));
    });
    assert!(drafting.is_cancelled());
    requests.try_iter().for_each(drop);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.ai.running(project));
        assert!(
            model.error.is_none(),
            "the replaced sheet's late result is ignored"
        );
    });
}
