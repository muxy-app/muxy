use super::git::{open_pull_request, registered_current_project};
use super::*;
use crate::ai::PROVIDERS;
use crate::model::ai::Availability;
use crate::repository_actions::Action;
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
            "The working tree is clean.",
            "Create PR requires uncommitted changes, even on a feature branch"
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
fn confirmation_does_not_start_work_and_cancel_is_side_effect_free(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
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
fn one_confirmation_starts_background_work_and_cannot_start_twice(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let other = state
        .projects()
        .iter()
        .find(|candidate| candidate.id != project && !candidate.home)
        .unwrap()
        .id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::Commit, window, cx);
            model.open_ai_action(Action::CreatePullRequest, window, cx);
        });
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Commit and Push");
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    view.update(cx, |model, cx| {
        assert!(model.overlay.is_none());
        assert!(model.ai.confirmation.is_none());
        assert!(model.ai.running(project));
        assert!(describe(model.ai_availability(Action::CreatePullRequest)).contains("Wait"));
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::ExtensionClient(_)))
                .count(),
            1
        );
        model.state.select_project(other).unwrap();
        model.sync_git(cx);
        assert!(
            model.ai.running(project),
            "switching projects must not cancel confirmed work"
        );
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.ai.running(project));
        assert_eq!(model.error.as_deref(), Some("Couldn't commit and push"));
    });
}

#[gpui::test]
fn confirming_a_stale_branch_does_not_start_work(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
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

#[gpui::test]
fn provider_menu_allows_uninstalled_choices_and_project_prompt_save_reset(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let settings_path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_provider_menu(Action::Commit, window, cx);
            model.select_ai_provider(2, window, cx);
            assert!(model.overlay.is_none());
            assert_eq!(model.settings.ai.providers["commit"], "codex");
            assert!(describe(model.ai_availability(Action::Commit)).contains("not installed"));
            model.set_ai_prompt(Action::CreatePullRequest, "Global instructions", cx);
            model.open_ai_provider_menu(Action::CreatePullRequest, window, cx);
            model.edit_project_pr_prompt(window, cx);
            let Some(Overlay::AiProvider(menu)) = &model.overlay else {
                panic!("provider menu");
            };
            let input = menu.prompt.as_ref().unwrap().clone();
            assert_eq!(input.read(cx).text(), "Global instructions");
            input.update(cx, |input, cx| input.set_text("Project instructions", cx));
            model.save_project_pr_prompt(false, cx);
            assert!(model.overlay.is_none());
            assert_eq!(
                model.configured_ai_prompt(Action::CreatePullRequest, Some(project)),
                "Project instructions"
            );
            assert_eq!(
                model.configured_ai_prompt(Action::Commit, Some(project)),
                Action::Commit.default_prompt()
            );
            assert_eq!(
                muxy_app_core::settings::Settings::load(&settings_path)
                    .unwrap()
                    .ai
                    .project_pr_prompts[&project.to_string()],
                "Project instructions"
            );
            model.open_ai_provider_menu(Action::CreatePullRequest, window, cx);
            model.edit_project_pr_prompt(window, cx);
            model.save_project_pr_prompt(true, cx);
            assert_eq!(
                model.configured_ai_prompt(Action::CreatePullRequest, Some(project)),
                "Global instructions"
            );
            assert!(
                muxy_app_core::settings::Settings::load(&settings_path)
                    .unwrap()
                    .ai
                    .project_pr_prompts
                    .is_empty()
            );
        });
    });
}

#[gpui::test]
fn native_pr_confirmation_starts_only_the_pr_action(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::CreatePullRequest, window, cx);
        });
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Create Pull Request");
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.ai.confirmation.is_none());
        assert_eq!(
            model.ai.running_action(project),
            Some(Action::CreatePullRequest)
        );
    });
    assert_eq!(
        requests
            .try_iter()
            .filter(|(_, work)| matches!(work, Work::ExtensionClient(_)))
            .count(),
        1
    );
    cx.run_until_parked();
}

#[gpui::test]
fn switching_projects_cancels_an_unconfirmed_action(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let other = state
        .projects()
        .iter()
        .find(|candidate| candidate.id != project && !candidate.home)
        .unwrap()
        .id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.open_ai_action(Action::Commit, window, cx);
        });
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.state.select_project(other).unwrap();
        model.sync_git(cx);
        assert!(model.ai.confirmation.is_none());
    });
    cx.run_until_parked();
    // The test platform retains the prompt, but its cancelled task cannot act on a late answer.
    cx.simulate_prompt_answer("Commit and Push");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(!model.ai.running(project)));
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::ExtensionClient(_)))
    );
}

#[gpui::test]
fn provider_menu_keyboard_selects_auto_and_editor_cancel_preserves_prompt(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.ai.installed = vec![PROVIDERS[0]];
            load(model, project, feature_changes(), cx);
            model.set_ai_provider(Action::Commit, "codex", cx);
            model.open_ai_provider_menu(Action::Commit, window, cx);
        });
    });
    cx.simulate_keystrokes("down enter");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(!model.settings.ai.providers.contains_key("commit"));
    });
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.open_ai_provider_menu(Action::CreatePullRequest, window, cx);
            model.edit_project_pr_prompt(window, cx);
            let Some(Overlay::AiProvider(menu)) = &model.overlay else {
                panic!("provider menu");
            };
            menu.prompt
                .as_ref()
                .unwrap()
                .update(cx, |input, cx| input.set_text("Unsaved instructions", cx));
        });
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |model, _| {
        let Some(Overlay::AiProvider(menu)) = &model.overlay else {
            panic!("return to provider menu");
        };
        assert!(menu.prompt.is_none());
        assert!(model.settings.ai.project_pr_prompts.is_empty());
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
}

#[gpui::test]
fn project_prompt_is_shared_by_worktrees_but_not_other_projects(cx: &mut TestAppContext) {
    let (mut state, project) = registered_current_project();
    let mut child = state.project(project).unwrap().descriptor();
    child.id = ProjectId::new();
    child.parent_id = Some(project);
    child.kind = Some(muxy_protocol::ProjectKind::Worktree);
    let worktree = child.id;
    let mut projects: Vec<_> = state
        .projects()
        .iter()
        .map(muxy_app_core::Project::descriptor)
        .collect();
    projects.push(child);
    state
        .apply_catalog(&muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(1),
            home: state.home().id,
            revision: state.catalog_revision() + 1,
            projects,
            next: None,
            legacy_home: None,
        })
        .unwrap();
    let other = state
        .projects()
        .iter()
        .find(|candidate| candidate.id != project && candidate.id != worktree && !candidate.home)
        .unwrap()
        .id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model
            .settings
            .ai
            .prompts
            .insert("create_pr".into(), "Global".into());
        model
            .settings
            .ai
            .project_pr_prompts
            .insert(project.to_string(), "Project".into());
        assert_eq!(
            model.configured_ai_prompt(Action::CreatePullRequest, Some(worktree)),
            "Project"
        );
        assert_eq!(
            model.configured_ai_prompt(Action::CreatePullRequest, Some(other)),
            "Global"
        );
        assert_eq!(
            model.configured_ai_prompt(Action::Commit, Some(worktree)),
            Action::Commit.default_prompt()
        );
    });
}
