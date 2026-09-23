use super::projects::two_projects;
use super::*;
use muxy_protocol::{
    ErrorCode, GitAction, GitBaseSwitch, GitChecks, GitMergeMethod, GitPullRequest,
    GitPullRequestAction, GitReply, GitRequest, GitSummary, ProjectMutation, ServerPath,
};

pub(super) fn open_pull_request() -> GitPullRequest {
    GitPullRequest {
        number: 12,
        url: "https://github.com/example/repo/pull/12".into(),
        title: "Feature".into(),
        author: "author".into(),
        head_branch: "feature".into(),
        head_oid: "abc".into(),
        base_branch: "main".into(),
        state: "OPEN".into(),
        draft: false,
        updated_at: None,
        mergeable: Some(true),
        merge_state: "CLEAN".into(),
        cross_repository: false,
        checks: GitChecks::default(),
    }
}

pub(super) fn registered_current_project() -> (AppState, ProjectId) {
    let (mut state, _, project, _, _) = two_projects();
    while let Some(intent) = state.project_intents().first().cloned() {
        state
            .complete_project_intent(intent.operation)
            .expect("projects registered");
    }
    (state, project)
}

fn merge_request(project: ProjectId) -> GitRequest {
    GitRequest {
        project,
        action: GitAction::PullRequest(GitPullRequestAction::Merge {
            number: 12,
            method: GitMergeMethod::Squash,
            delete_branch: false,
            expected_head: Some("abc".into()),
        }),
    }
}

#[gpui::test]
fn merged_pull_request_updates_the_base_branch_then_reports_it(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git.projects.entry(project).or_default().pull_request = Some(open_pull_request());
        let merge = merge_request(project);
        model.git_request(project, merge.action.clone(), cx);
        requests.try_iter().for_each(drop);

        model.dismiss_overlay(cx);
        model.receive_git(&merge, Ok(GitReply::Done), cx);
        assert!(model.notice.is_none(), "the merge is reported with the base update");
        assert!(requests.try_iter().any(|(_, work)| {
            matches!(work, Work::Git(GitRequest { project: target, action: GitAction::SwitchToBase(base) }) if target == project && base == "main")
        }));
        model.receive_git(
            &GitRequest { project, action: GitAction::SwitchToBase("main".into()) },
            Ok(GitReply::BaseSwitch(GitBaseSwitch::Updated)),
            cx,
        );
        assert_eq!(model.notice.as_deref(), Some("Merged PR #12 into main"));
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(work.iter().any(|work| {
            matches!(work, Work::Git(GitRequest { project: target, action: GitAction::Summary }) if *target == project)
        }), "{work:?}");
    });
}

#[gpui::test]
fn merged_pull_request_leaves_worktrees_whose_base_is_checked_out_elsewhere(
    cx: &mut TestAppContext,
) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git.projects.entry(project).or_default().pull_request = Some(open_pull_request());
        let merge = merge_request(project);
        model.git_request(project, merge.action.clone(), cx);
        model.receive_git(&merge, Ok(GitReply::Done), cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::SwitchToBase("main".into()),
            },
            Ok(GitReply::BaseSwitch(GitBaseSwitch::CheckedOutElsewhere(
                ServerPath(b"/repo".to_vec()),
            ))),
            cx,
        );
        assert_eq!(model.notice.as_deref(), Some("Merged PR #12 into main"));
        assert!(model.error.is_none());
    });
}

#[gpui::test]
fn failed_base_update_reports_that_the_merge_succeeded(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git.projects.entry(project).or_default().pull_request = Some(open_pull_request());
        let merge = merge_request(project);
        model.git_request(project, merge.action.clone(), cx);
        model.receive_git(&merge, Ok(GitReply::Done), cx);
        requests.try_iter().for_each(drop);
        model.dismiss_overlay(cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::SwitchToBase("main".into()),
            },
            Err(muxy_client::ClientError::Invalid(ErrorCode::BadRequest)),
            cx,
        );
        assert_eq!(
            model.error.as_deref(),
            Some("Merged PR #12, but couldn't update main")
        );
        assert!(requests.try_iter().any(|(_, work)| {
            matches!(work, Work::Git(GitRequest { project: target, action: GitAction::Summary }) if target == project)
        }));
    });
}

#[gpui::test]
fn newly_added_project_starts_git_after_server_registration(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        assert!(model.add_project(std::env::temp_dir(), cx));
        let project = model.state.current_project().id;
        let create = model
            .state
            .project_intents()
            .iter()
            .find(|intent| matches!(&intent.mutation, ProjectMutation::Create(record) if record.id == project))
            .expect("pending project create")
            .operation;
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Git(request) if request.project == project)),
            "Git must wait until the server knows this project"
        );

        model.receive_project_mutation(create, Ok(1), cx);
        assert!(requests.try_iter().any(|(_, work)| {
            matches!(work, Work::Git(request) if request.project == project && request.action == GitAction::Watch)
        }));
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Watch,
            },
            Ok(GitReply::Done),
            cx,
        );
        assert!(requests.try_iter().any(|(_, work)| {
            matches!(work, Work::Git(request) if request.project == project && request.action == GitAction::Summary)
        }));
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Summary,
            },
            Ok(GitReply::Summary(Some(GitSummary {
                branch: Some("feature".into()),
                ..GitSummary::default()
            }))),
            cx,
        );
        assert_eq!(
            model.git.projects[&project]
                .summary
                .as_ref()
                .and_then(|summary| summary.branch.as_deref()),
            Some("feature")
        );
    });
}

#[gpui::test]
fn git_results_stay_with_the_requested_project_after_switching(cx: &mut TestAppContext) {
    let (state, _, _, _, _) = two_projects();
    let first = state.current_project().id;
    let second = state
        .projects()
        .iter()
        .find(|p| p.id != first && !p.home)
        .expect("second")
        .id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git_request(first, GitAction::Summary, cx);
        model.select_project(second, cx);
        model.receive_git(
            &GitRequest {
                project: first,
                action: GitAction::Summary,
            },
            Ok(GitReply::Summary(Some(GitSummary {
                branch: Some("first-branch".into()),
                ..GitSummary::default()
            }))),
            cx,
        );
        assert_eq!(model.state.current_project().id, second);
        assert_eq!(
            model.git.projects[&first]
                .summary
                .as_ref()
                .expect("summary")
                .branch
                .as_deref(),
            Some("first-branch")
        );
        assert!(
            model
                .git
                .projects
                .get(&second)
                .is_none_or(|r| r.summary.is_none())
        );
    });
}

#[gpui::test]
fn git_refreshes_coalesce_and_disconnected_mutations_are_not_queued(cx: &mut TestAppContext) {
    let (state, _, _, _, _) = two_projects();
    let project = state.current_project().id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        for _ in 0..4 {
            model.git_request(project, GitAction::Summary, cx);
        }
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::Git(_)))
                .count(),
            1
        );
        model.disconnect(cx);
        requests.try_iter().for_each(drop);
        model.git_request(project, GitAction::DeleteBranch("branch".into()), cx);
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Git(_)))
        );
    });
}

#[gpui::test]
fn an_accepted_git_action_waits_for_background_refresh(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git_request(project, GitAction::Summary, cx);
        model.git_request(project, GitAction::DeleteBranch("confirmed".into()), cx);
        assert_eq!(requests.try_iter().filter(|(_, work)| matches!(work, Work::Git(_))).count(), 1);
        model.receive_git(&GitRequest { project, action: GitAction::Summary }, Ok(GitReply::Summary(None)), cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Git(request) if request.action == GitAction::DeleteBranch("confirmed".into()))));
    });
}

#[gpui::test]
fn stale_git_mutations_and_inspections_do_not_change_a_new_form(cx: &mut TestAppContext) {
    let (state, first, second, _, _) = two_projects();
    let (boot, _) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git_request(first, GitAction::CreateBranch("old".into()), cx);
        model.select_project(second, cx);
        model.open_git_form(second, false, cx);
        model.receive_git(
            &GitRequest {
                project: first,
                action: GitAction::CreateBranch("old".into()),
            },
            Ok(GitReply::Done),
            cx,
        );
        assert!(matches!(model.overlay, Some(Overlay::GitForm(_))));
        model.git_request(first, GitAction::InspectRemoval, cx);
        model.open_git_form(second, false, cx);
        let expected = muxy_protocol::WorktreeRemoval {
            directory: ServerPath(b"/unused".to_vec()),
            device: 1,
            inode: 1,
            dirty: false,
            status: vec![],
            head: None,
            branch: None,
        };
        model.receive_git(
            &GitRequest {
                project: first,
                action: GitAction::InspectRemoval,
            },
            Ok(GitReply::Removal(expected)),
            cx,
        );
        assert!(matches!(model.overlay, Some(Overlay::GitForm(_))));
        assert!(model.close_prompt.is_none());
    });
}

#[gpui::test]
fn git_popovers_open_from_their_controls_and_follow_their_anchors(cx: &mut TestAppContext) {
    let (state, _, _, _, _) = two_projects();
    let project = state.current_project().id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Summary,
            },
            Ok(GitReply::Summary(Some(GitSummary {
                branch: Some("feature".into()),
                changed: 2,
                ..GitSummary::default()
            }))),
            cx,
        );
    });
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    let changes = cx.debug_bounds("git-changes-status").expect("changes");
    cx.simulate_click(changes.center(), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            model.git.changes_anchor.get().is_some(),
            "changes anchor was measured"
        );
        assert!(
            matches!(model.overlay, Some(Overlay::Git(_))),
            "changes picker is open"
        );
    });
    let panel = cx.debug_bounds("git-picker").expect("changes popover");
    assert!(panel.bottom() <= changes.top());
    cx.simulate_resize(size(px(700.0), px(500.0)));
    cx.run_until_parked();
    let panel = cx.debug_bounds("git-picker").expect("resized popover");
    let changes = cx
        .debug_bounds("git-changes-status")
        .expect("resized changes");
    assert!(panel.bottom() <= changes.top());
    assert!(panel.left() >= px(0.0) && panel.right() <= px(700.0));
    assert!(panel.left() <= changes.right() && panel.right() >= changes.left());
}

#[gpui::test]
fn git_form_fits_within_small_windows(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, _) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.open_git_form(project, true, cx));
    for viewport in [size(px(1200.0), px(900.0)), size(px(600.0), px(400.0))] {
        cx.simulate_resize(viewport);
        cx.run_until_parked();
        let form = cx.debug_bounds("git-form").expect("form");
        assert!(form.top() >= px(0.0) && form.bottom() <= viewport.height);
        assert!(form.left() >= px(0.0) && form.right() <= viewport.width);
    }
}

#[gpui::test]
fn distinct_reads_are_retained_and_cached_controls_stay_usable(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        requests.try_iter().for_each(drop);
        model.git_request(project, GitAction::Branches, cx);
        model.git_request(project, GitAction::Changes, cx);
        assert!(!model.git.projects[&project].busy());
        requests.try_iter().for_each(drop);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Branches,
            },
            Ok(GitReply::Branches(vec![])),
            cx,
        );
        assert!(requests.try_iter().any(
            |(_, work)| matches!(work, Work::Git(request) if request.action == GitAction::Changes)
        ));
        assert!(!model.git.projects[&project].busy());
    });
}

#[gpui::test]
fn background_git_errors_do_not_raise_alerts_or_retry_on_idle(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git_request(project, GitAction::Summary, cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Summary,
            },
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert!(model.error.is_none());
    });
    requests.try_iter().for_each(drop);
    cx.executor().advance_clock(Duration::from_secs(30));
    cx.run_until_parked();
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Git(_)))
    );
}

#[gpui::test]
fn filesystem_invalidations_refresh_only_the_active_project(cx: &mut TestAppContext) {
    let (mut state, _, _, _, _) = two_projects();
    while let Some(intent) = state.project_intents().first().cloned() {
        state
            .complete_project_intent(intent.operation)
            .expect("projects registered");
    }
    let project = state.current_project().id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git.reset_context();
        model.sync_git(cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Watch,
            },
            Ok(GitReply::Done),
            cx,
        );
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Summary,
            },
            Ok(GitReply::Summary(Some(GitSummary {
                branch: Some("main".into()),
                ..GitSummary::default()
            }))),
            cx,
        );
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Branches,
            },
            Ok(GitReply::Branches(vec![])),
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
        requests.try_iter().for_each(drop);
        model.git_invalidated(ProjectId::new(), cx);
        assert!(requests.try_iter().next().is_none());
        model.git_invalidated(project, cx);
        assert!(requests.try_iter().any(
            |(_, work)| matches!(work, Work::Git(request) if request.action == GitAction::Summary)
        ));
        assert!(!model.git.projects[&project].busy());
    });
}

#[gpui::test]
fn successful_refresh_recovers_a_rejected_branch_switch(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        let request = GitRequest {
            project,
            action: GitAction::SwitchBranch("busy-branch".into()),
        };
        model.git_request(project, request.action.clone(), cx);
        model.receive_git(
            &request,
            Err(muxy_client::ClientError::Invalid(ErrorCode::BadRequest)),
            cx,
        );
        assert!(
            model.git.projects[&project]
                .load_error(&GitAction::Branches)
                .is_some()
        );
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Summary,
            },
            Ok(GitReply::Summary(Some(GitSummary {
                branch: Some("main".into()),
                ..GitSummary::default()
            }))),
            cx,
        );
        model.git_request(project, GitAction::Branches, cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Branches,
            },
            Ok(GitReply::Branches(vec![muxy_protocol::GitBranch {
                name: "main".into(),
                current: true,
                checked_out: true,
                default: true,
            }])),
            cx,
        );
        let repository = &model.git.projects[&project];
        assert!(repository.load_error(&GitAction::Branches).is_none());
        assert!(repository.has_loaded(&GitAction::Branches));
        assert!(!repository.busy());
        assert_eq!(repository.branches[0].name, "main");
    });
}

fn click_form(cx: &mut VisualTestContext, selector: &str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector.to_owned().leak())
        .expect("control");
    let position = gpui::point(bounds.center().x, bounds.bottom() - px(10.0));
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn branch_form_validates_inline_and_submits_once(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git.projects.entry(project).or_default().disconnect();
        model.open_git_form(project, false, cx);
    });
    cx.run_until_parked();
    requests.try_iter().for_each(drop);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.debug_bounds("git-form-error").is_some());
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Git(_)))
    );
    cx.simulate_input("feature-ui");
    cx.simulate_keystrokes("enter enter");
    cx.run_until_parked();
    let creations: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Git(GitRequest {
                action: GitAction::CreateBranch(branch),
                ..
            }) => Some(branch),
            _ => None,
        })
        .collect();
    assert_eq!(creations, ["feature-ui"]);
}

#[gpui::test]
fn worktree_form_derives_location_and_preserves_a_manual_location(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git.projects.entry(project).or_default().disconnect();
        model.open_git_form(project, true, cx);
        model.receive_git(
            &GitRequest {
                project,
                action: GitAction::Branches,
            },
            Ok(GitReply::Branches(vec![muxy_protocol::GitBranch {
                name: "main".into(),
                current: true,
                checked_out: true,
                default: true,
            }])),
            cx,
        );
    });
    cx.run_until_parked();
    cx.simulate_input("feature/ui");
    cx.run_until_parked();
    click_form(cx, "git-field-Location");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("/tmp/custom-worktree");
    click_form(cx, "git-field-Branch name");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("feature/changed");
    requests.try_iter().for_each(drop);
    click_form(cx, "git-submit");
    let action = requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::Git(GitRequest {
                action: GitAction::Worktree(intent),
                ..
            }) => Some(intent.action),
            _ => None,
        })
        .expect("creation");
    let muxy_protocol::WorktreeAction::Create {
        directory,
        branch,
        base,
        ..
    } = action
    else {
        panic!("creation action")
    };
    assert_eq!(directory.0, b"/tmp/custom-worktree");
    assert_eq!(branch, "feature/changed");
    assert_eq!(base.as_deref(), Some("main"));
}

#[gpui::test]
fn worktree_branch_picker_keeps_the_form_and_cancel_restores_input(cx: &mut TestAppContext) {
    let (state, project, _, _, _) = two_projects();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| model.open_git_form(project, true, cx));
    cx.run_until_parked();
    cx.simulate_input("feature-ui");
    click_form(cx, "git-field-Base branch");
    assert!(cx.debug_bounds("git-form").is_some());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::GitForm(_))));
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    view.update(cx, |model, cx| model.open_git_form(project, true, cx));
    click_form(cx, "git-cancel");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
}

#[gpui::test]
fn existing_branch_choices_update_only_automatic_worktree_locations(cx: &mut TestAppContext) {
    for manual in [false, true] {
        let (state, project, _, _, _) = two_projects();
        let (boot, requests) = stub_boot(state);
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.git.projects.entry(project).or_default().disconnect();
            model.open_git_form(project, true, cx);
            model.receive_git(
                &GitRequest {
                    project,
                    action: GitAction::Branches,
                },
                Ok(GitReply::Branches(vec![
                    muxy_protocol::GitBranch {
                        name: "main".into(),
                        current: true,
                        checked_out: true,
                        default: true,
                    },
                    muxy_protocol::GitBranch {
                        name: "feature/existing".into(),
                        current: false,
                        checked_out: false,
                        default: false,
                    },
                ])),
                cx,
            );
        });
        cx.run_until_parked();
        click_form(cx, "settings-segment-worktree-branch-mode-existing");
        if manual {
            click_form(cx, "git-field-Location");
            cx.simulate_keystrokes("cmd-a");
            cx.simulate_input("/tmp/keep-my-location");
        }
        click_form(cx, "git-field-Branch");
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        requests.try_iter().for_each(drop);
        click_form(cx, "git-submit");
        let action = requests
            .try_iter()
            .find_map(|(_, work)| match work {
                Work::Git(GitRequest {
                    action: GitAction::Worktree(intent),
                    ..
                }) => Some(intent.action),
                _ => None,
            })
            .expect("worktree creation");
        let muxy_protocol::WorktreeAction::Create {
            directory,
            branch,
            base,
            ..
        } = action
        else {
            panic!("creation action")
        };
        assert_eq!(branch, "feature/existing");
        assert!(base.is_none());
        if manual {
            assert_eq!(directory.0, b"/tmp/keep-my-location");
        } else {
            assert!(directory.0.ends_with(b"-feature-existing"));
        }
    }
}

#[gpui::test]
fn pull_request_toasts_report_success_only_for_the_active_project(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.git_request(
            project,
            GitAction::PullRequest(GitPullRequestAction::Close { number: 12 }),
            cx,
        );
        for (action, expected) in [
            (GitPullRequestAction::Close { number: 12 }, "Closed PR #12"),
            (
                GitPullRequestAction::UpdateBranch {
                    number: 12,
                    expected_head: "abc".into(),
                },
                "Updated branch for PR #12",
            ),
        ] {
            model.receive_git(
                &GitRequest {
                    project,
                    action: GitAction::PullRequest(action),
                },
                Ok(GitReply::Done),
                cx,
            );
            assert_eq!(model.notice.as_deref(), Some(expected));
        }
        model.notice = None;
        model.receive_git(
            &merge_request(project),
            Err(muxy_client::ClientError::Invalid(ErrorCode::BadRequest)),
            cx,
        );
        assert!(model.notice.is_none());
        assert_eq!(model.error.as_deref(), Some("Couldn't merge PR #12"));
        model.set_banner_error(None);
        model
            .state
            .select_project(model.state.home().id)
            .expect("select home");
        model.receive_git(&merge_request(project), Ok(GitReply::Done), cx);
        assert!(model.notice.is_none());
    });
}
