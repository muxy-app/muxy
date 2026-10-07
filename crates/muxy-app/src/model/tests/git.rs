use super::projects::two_projects;
use super::*;
use muxy_protocol::{GitAction, GitReply, GitRequest, ServerPath};

pub(super) fn registered_current_project() -> (AppState, ProjectId) {
    let (mut state, _, project, _, _) = two_projects();
    while let Some(intent) = state.project_intents(ServerId::local()).first().cloned() {
        state
            .complete_project_intent(ServerId::local(), intent.operation)
            .expect("projects registered");
    }
    (state, project)
}

#[gpui::test]
fn git_refreshes_coalesce_and_disconnected_mutations_are_not_queued(cx: &mut TestAppContext) {
    let (state, _, _, _, _) = two_projects();
    let project = state.current_project().id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
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
        model.disconnect(ServerId::local(), cx);
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
fn stale_git_mutations_and_inspections_do_not_change_a_new_form(cx: &mut TestAppContext) {
    let (state, first, second, _, _) = two_projects();
    let (boot, _) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
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
#[allow(
    clippy::too_many_lines,
    reason = "Exercise approval, submission, and persistence in one form lifecycle"
)]
fn worktree_form_reviews_hooks_validates_templates_and_saves_the_submitted_location(
    cx: &mut TestAppContext,
) {
    let (state, project) = registered_current_project();
    let (boot, requests) = stub_boot(state);
    let settings_path = boot.state_path.with_file_name("settings.toml");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let hooks = vec![muxy_protocol::WorktreeHook {
        command: "echo setup".into(),
        name: Some("Setup".into()),
        project: true,
    }];
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.git.projects.entry(project).or_default().disconnect();
        model.open_git_form(project, true, cx);
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
                action: GitAction::WorktreeHooks { teardown: false },
            },
            Ok(GitReply::WorktreeHooks(hooks.clone())),
            cx,
        );
    });
    cx.run_until_parked();
    cx.simulate_input("display-name");
    click_form(cx, "settings-field-git-branch");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("feature/custom");
    click_form(cx, "settings-field-git-name");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Renamed worktree");
    click_form(cx, "settings-segment-worktree-location-template");
    click_form(cx, "settings-field-git-template");
    cx.simulate_input("../{branch}/../fixed");
    requests.try_iter().for_each(drop);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Git(_)))
    );
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("../trees/{branch}");
    click_form(cx, "settings-toggle-worktree-setup");
    click_form(cx, "settings-field-git-name");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let request = requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::Git(request) if matches!(request.action, GitAction::Worktree(_)) => Some(request),
            _ => None,
        })
        .expect("create worktree");
    let GitAction::Worktree(intent) = &request.action else {
        panic!("intent")
    };
    let options = intent.options.as_ref().expect("options");
    assert_eq!(options.name.as_deref(), Some("Renamed worktree"));
    assert_eq!(options.hooks.as_ref(), Some(&hooks));
    let muxy_protocol::WorktreeAction::Create {
        project: child,
        directory,
        branch,
        ..
    } = &intent.action
    else {
        panic!("create")
    };
    assert_eq!(
        branch, "feature/custom",
        "editing the name preserves a customized branch"
    );
    assert!(directory.0.ends_with(b"/trees/feature-custom"));
    let record = muxy_protocol::ProjectDescriptor {
        id: *child,
        directory: directory.clone(),
        name: "Renamed worktree".into(),
        home: false,
        icon: None,
        logo: None,
        color: "#ffffff".into(),
        kind: Some(muxy_protocol::ProjectKind::Worktree),
        parent_id: Some(project),
    };
    // Edits while the server works must not replace the location actually submitted.
    click_form(cx, "settings-field-git-template");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("../not-submitted/{branch}");
    view.update(cx, |model, cx| {
        model.receive_git(&request, Ok(GitReply::Project(record)), cx);
    });
    let saved = muxy_app_core::settings::Settings::load(&settings_path).expect("settings");
    assert_eq!(
        saved.worktrees.projects[&project].path_template,
        "../trees/{branch}"
    );
}
