use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use muxy_app_core::{AppError, AppState, ProjectId, ProjectStatus, ServerId, restore, store};
use muxy_protocol::{
    CatalogPage, ProjectDescriptor, ServerIdentity, ServerPath, SessionId, SessionInfo,
};
use serde_json::json;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const LOCAL: ServerId = ServerId::local();

fn session(id: u64) -> TestResult<SessionId> {
    Ok(SessionId::new(id).ok_or("session IDs are nonzero")?)
}

fn acknowledge(state: &mut AppState, server: ServerId) -> TestResult {
    while let Some(intent) = state.project_intents(server).first().cloned() {
        state.complete_project_intent(server, intent.operation)?;
    }
    Ok(())
}

/// The local server's catalog once it has accepted every pending edit.
fn local_catalog(state: &mut AppState) -> TestResult<CatalogPage> {
    acknowledge(state, LOCAL)?;
    Ok(CatalogPage {
        server: ServerIdentity::from_u128(1),
        home: state.home().id,
        revision: state.catalog_revision(LOCAL) + 1,
        projects: state
            .projects()
            .iter()
            .filter(|project| project.server_id == LOCAL)
            .map(muxy_app_core::Project::descriptor)
            .collect(),
        next: None,
        legacy_home: None,
    })
}

/// A remote server's catalog: its Home and `projects`, under `/home/dev`.
fn remote_catalog(identity: u128, home: ProjectId, projects: &[(ProjectId, &str)]) -> CatalogPage {
    let descriptor = |id, home, name: &str, directory: String| ProjectDescriptor {
        id,
        home,
        name: name.into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: ServerPath(directory.into_bytes()),
        kind: None,
        parent_id: None,
    };
    CatalogPage {
        server: ServerIdentity::from_u128(identity),
        home,
        revision: 7,
        projects: std::iter::once(descriptor(home, true, "Home", "/home/dev".into()))
            .chain(
                projects
                    .iter()
                    .map(|(id, name)| descriptor(*id, false, name, format!("/home/dev/{name}"))),
            )
            .collect(),
        next: None,
        legacy_home: None,
    }
}

struct TwoServers {
    state: AppState,
    remote: ServerId,
    remote_home: ProjectId,
    api: ProjectId,
}

/// The local server with one project, and a remote one with its Home and `api`.
fn two_servers() -> TestResult<TwoServers> {
    let mut state = AppState::bootstrap()?;
    state.add_project(LOCAL, std::env::temp_dir())?;
    let page = local_catalog(&mut state)?;
    state.apply_catalog(LOCAL, &page)?;
    let (remote, remote_home, api) = (ServerId::new(), ProjectId::new(), ProjectId::new());
    state.apply_catalog(remote, &remote_catalog(2, remote_home, &[(api, "api")]))?;
    Ok(TwoServers {
        state,
        remote,
        remote_home,
        api,
    })
}

fn servers_in_order(state: &AppState) -> Vec<(ServerId, bool)> {
    state
        .projects()
        .iter()
        .map(|project| (project.server_id, project.home))
        .collect()
}

#[test]
fn catalogs_from_two_servers_keep_each_others_projects_and_homes() -> TestResult {
    for local_first in [true, false] {
        let mut state = AppState::bootstrap()?;
        let local_project = state.add_project(LOCAL, std::env::temp_dir())?;
        let (remote, remote_home, api) = (ServerId::new(), ProjectId::new(), ProjectId::new());
        let remote_page = remote_catalog(2, remote_home, &[(api, "api")]);
        if local_first {
            let page = local_catalog(&mut state)?;
            state.apply_catalog(LOCAL, &page)?;
            state.apply_catalog(remote, &remote_page)?;
        } else {
            state.apply_catalog(remote, &remote_page)?;
            let page = local_catalog(&mut state)?;
            state.apply_catalog(LOCAL, &page)?;
        }
        assert_eq!(
            servers_in_order(&state),
            [
                (LOCAL, true),
                (LOCAL, false),
                (remote, true),
                (remote, false)
            ]
        );
        assert_eq!(
            state.server_home(remote).map(|home| home.id),
            Some(remote_home)
        );
        let api_project = state.project(api).ok_or("remote project")?;
        assert_eq!(api_project.server_id, remote);
        assert_eq!(api_project.directory, Path::new("/home/dev/api"));
        assert_eq!(state.catalog_revision(remote), 7);
        assert_eq!(state.catalog_revision(LOCAL), 1);

        let page = local_catalog(&mut state)?;
        state.apply_catalog(LOCAL, &page)?;
        assert!(state.project(api).is_some());
        state.apply_catalog(remote, &remote_catalog(2, remote_home, &[]))?;
        assert!(state.project(api).is_none());
        assert!(state.project(local_project).is_some());
        assert_eq!(state.home().server_id, LOCAL);
    }
    Ok(())
}

#[test]
fn equal_session_ids_on_two_servers_never_mix() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        remote_home,
        ..
    } = two_servers()?;
    let one = session(1)?;
    state.open_terminal_tab(state.home().id)?;
    let local_pane = state.home().tabs[0].panes[0].id;
    state.set_pane_session(local_pane, Some(one))?;
    state.open_terminal_tab(remote_home)?;
    let remote_pane = state.project(remote_home).ok_or("remote Home")?.tabs[0].panes[0].id;
    state.set_pane_session(remote_pane, Some(one))?;
    let quick = state.ensure_quick_terminal();
    assert_eq!(state.pane_server(local_pane), Some(LOCAL));
    assert_eq!(state.pane_server(remote_pane), Some(remote));
    assert_eq!(state.pane_server(quick), Some(LOCAL));
    assert_eq!(state.project_server(remote_home), Some(remote));
    assert_eq!(
        state.session_references_by_server(),
        BTreeMap::from([(LOCAL, vec![one]), (remote, vec![one])])
    );

    state.close_session_pane(remote_pane)?;
    assert_eq!(state.pending_discards(remote), [one]);
    assert!(state.pending_discards(LOCAL).is_empty());
    assert_eq!(state.session_references(LOCAL), [one]);
    assert_eq!(
        state.session_references_by_server(),
        BTreeMap::from([(LOCAL, vec![one]), (remote, Vec::new())])
    );

    state.queue_discard(LOCAL, one);
    state.prepare_closes(LOCAL);
    state.prepare_closes(remote);
    let local_close = state.close_operation(LOCAL, one).ok_or("local close")?;
    let remote_close = state.close_operation(remote, one).ok_or("remote close")?;
    assert_ne!(local_close, remote_close);
    state.complete_discard(remote, one);
    assert!(state.pending_discards(remote).is_empty());
    assert_eq!(state.pending_discards(LOCAL), [one]);
    assert_eq!(state.close_operation(LOCAL, one), Some(local_close));

    state.open_terminal_tab(remote_home)?;
    let remote_pane = state.project(remote_home).ok_or("remote Home")?.tabs[0].panes[0].id;
    state.set_pane_session(remote_pane, Some(one))?;
    state.close_session_panes(LOCAL, one)?;
    assert!(state.home().tabs.is_empty());
    assert_eq!(state.session_references(remote), [one]);

    let creating = state.split_pane(remote_pane, muxy_app_core::Direction::Right)?;
    state.prepare_creation(creating, Path::new("/home/dev"));
    state.close_pane(creating)?;
    assert_eq!(
        state.pending_cancellations(remote),
        [creating.creation_token()]
    );
    assert!(state.pending_cancellations(LOCAL).is_empty());
    state.complete_cancellation(remote, creating.creation_token());
    assert!(state.pending_cancellations(remote).is_empty());

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    store::save(&path, &state)?;
    assert_eq!(store::load(&path)?, state);
    Ok(())
}

#[test]
fn a_changed_identity_refuses_only_that_servers_catalog() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        remote_home,
        api,
    } = two_servers()?;
    let before = state.clone();
    let replaced = remote_catalog(3, remote_home, &[]);
    let error = state.apply_catalog(remote, &replaced).err();
    assert!(matches!(error, Some(AppError::ServerChanged(server)) if server == remote));
    assert_eq!(state, before);
    assert!(state.project(api).is_some());

    let page = local_catalog(&mut state)?;
    state.apply_catalog(LOCAL, &page)?;
    assert_eq!(state.catalog_revision(LOCAL), 2);

    state.forget_server(remote)?;
    state.apply_catalog(remote, &replaced)?;
    assert_eq!(
        state.server_home(remote).map(|home| home.id),
        Some(remote_home)
    );
    assert!(state.project(api).is_none());
    Ok(())
}

#[test]
fn the_same_server_under_two_entries_is_refused() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        remote_home,
        ..
    } = two_servers()?;
    let before = state.clone();
    let twin = ServerId::new();
    let error = state
        .apply_catalog(twin, &remote_catalog(2, remote_home, &[]))
        .err();
    assert!(matches!(
        error,
        Some(AppError::DuplicateServer { server, existing }) if server == twin && existing == remote
    ));
    let error = state
        .apply_catalog(twin, &remote_catalog(1, ProjectId::new(), &[]))
        .err();
    assert!(matches!(
        error,
        Some(AppError::DuplicateServer { existing, .. }) if existing == LOCAL
    ));
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn forgetting_a_server_removes_only_what_belongs_to_it() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        api,
        ..
    } = two_servers()?;
    let local_project = state.projects()[1].id;
    let workspace = state.create_workspace("Work")?;
    state.set_workspace_member(workspace, local_project, true)?;
    state.set_workspace_member(workspace, api, true)?;
    state.open_terminal_tab(api)?;
    let pane = state.project(api).ok_or("api")?.tabs[0].panes[0].id;
    state.set_startup_command(pane, "make")?;
    state.prepare_creation(pane, Path::new("/home/dev/api"));
    state.queue_discard(remote, session(4)?);
    state.rename_project(api, "API")?;
    assert_eq!(state.project_intents(remote).len(), 1);
    assert_eq!(state.window().current_project, api);

    let error = state.forget_server(LOCAL).err();
    assert!(matches!(error, Some(AppError::InvalidState(_))));
    state.forget_server(remote)?;
    assert_eq!(servers_in_order(&state), [(LOCAL, true), (LOCAL, false)]);
    assert_eq!(
        state.workspace(workspace).ok_or("workspace")?.projects,
        [local_project].into()
    );
    assert_eq!(state.window().current_project, state.home().id);
    assert!(!state.window().selected_tab.contains_key(&api));
    assert_eq!(state.startup_command(pane), None);
    assert!(state.pending_discards(remote).is_empty());
    assert!(state.project_intents(remote).is_empty());
    assert_eq!(state.catalog_revision(remote), 0);

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    store::save(&path, &state)?;
    assert_eq!(store::load(&path)?, state);
    Ok(())
}

#[test]
fn projects_of_unlisted_servers_survive_load_and_save() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        remote_home,
        api,
    } = two_servers()?;
    state.open_terminal_tab(api)?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    store::save(&path, &state)?;
    let loaded = store::load(&path)?;
    assert_eq!(loaded, state);
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    assert_eq!(saved["version"], 3);
    assert_eq!(saved["servers"][remote.to_string()]["catalog_revision"], 7);

    let mut misplaced = saved.clone();
    let projects = misplaced["projects"].as_array_mut().ok_or("projects")?;
    projects.swap(2, 3);
    std::fs::write(&path, serde_json::to_vec(&misplaced)?)?;
    let repaired = store::load(&path)?;
    assert_eq!(
        repaired.server_home(remote).map(|home| home.id),
        Some(remote_home)
    );
    assert_eq!(repaired.projects()[2].id, remote_home);
    Ok(())
}

#[test]
fn remote_projects_never_touch_this_computers_disk() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        api,
        ..
    } = two_servers()?;
    let missing = Path::new("/muxy-test-missing/app");
    assert!(state.add_project(LOCAL, missing.into()).is_err());
    assert!(state.add_project(remote, "relative/app".into()).is_err());
    assert!(
        state
            .add_project(ServerId::new(), "/srv/app".into())
            .is_err()
    );
    let before = state.project_intents(LOCAL).len();

    let project = state.add_project(remote, missing.into())?;
    state.refresh_project_statuses();
    let added = state.project(project).ok_or("remote project")?;
    assert_eq!(added.server_id, remote);
    assert_eq!(added.status(), ProjectStatus::Available);
    assert_eq!(
        state.project(api).map(muxy_app_core::Project::status),
        Some(ProjectStatus::Available)
    );
    assert_eq!(state.project_intents(remote).len(), 1);
    assert_eq!(state.project_intents(LOCAL).len(), before);
    assert!(state.project_creation_pending(project));
    state.remove_project(project)?;
    assert!(state.project_creation_pending(project));
    Ok(())
}

#[test]
fn restoring_and_ending_all_stay_on_one_server() -> TestResult {
    let TwoServers {
        mut state,
        remote,
        remote_home,
        ..
    } = two_servers()?;
    let one = session(1)?;
    let home = state.home().id;
    state.open_terminal_tab(home)?;
    let local_pane = state.home().tabs[0].panes[0].id;
    state.set_pane_session(local_pane, Some(one))?;
    state.open_terminal_tab(remote_home)?;
    let remote_pane = state.project(remote_home).ok_or("remote Home")?.tabs[0].panes[0].id;
    state.set_pane_session(remote_pane, Some(one))?;
    let fresh = state.split_pane(remote_pane, muxy_app_core::Direction::Down)?;
    let live = SessionInfo {
        id: one,
        project: home,
        directory: ServerPath(b"/".into()),
    };

    let local_plan = restore::plan(&state, LOCAL, std::slice::from_ref(&live));
    assert_eq!(local_plan.attach, [(local_pane, one)]);
    assert!(local_plan.create.is_empty() && local_plan.close.is_empty());
    let remote_plan = restore::plan(&state, remote, &[live]);
    assert_eq!(remote_plan.close, [(remote_pane, one)]);
    assert_eq!(remote_plan.create, [fresh]);
    assert!(remote_plan.attach.is_empty());

    state.ensure_quick_terminal();
    state.clear_terminal_panes(LOCAL)?;
    assert!(state.home().tabs.is_empty());
    assert!(state.quick_terminal().is_none());
    assert_eq!(
        state.project(remote_home).ok_or("remote Home")?.tabs[0]
            .panes
            .len(),
        2
    );
    Ok(())
}

#[test]
fn backups_leave_remote_projects_out_and_restoring_keeps_them() -> TestResult {
    let TwoServers {
        state: mut current,
        remote,
        api,
        ..
    } = two_servers()?;
    let folder = tempfile::tempdir()?;
    let local_project = current.add_project(LOCAL, folder.path().into())?;
    acknowledge(&mut current, LOCAL)?;
    let workspace = current.create_workspace("Work")?;
    current.set_workspace_member(workspace, local_project, true)?;
    current.set_workspace_member(workspace, api, true)?;
    current.open_terminal_tab(api)?;
    let pane = current.project(api).ok_or("api")?.tabs[0].panes[0].id;
    current.set_pane_session(pane, Some(session(9)?))?;
    current.queue_discard(remote, session(8)?);
    current.rename_project(api, "API")?;

    let backup = current.configuration_backup();
    assert!(
        backup
            .projects()
            .iter()
            .all(|project| project.server_id == LOCAL)
    );
    assert_eq!(
        backup.workspace(workspace).ok_or("workspace")?.projects,
        [local_project].into()
    );
    assert_eq!(serde_json::to_value(&backup)?["servers"], json!({}));

    let restored = current.clone().restore_configuration(&current)?;
    assert_eq!(restored.project(api), current.project(api));
    assert_eq!(restored.pending_discards(remote), [session(8)?]);
    assert_eq!(restored.catalog_revision(remote), 7);
    assert_eq!(
        restored.project_intents(remote),
        current.project_intents(remote)
    );
    assert_eq!(restored.project_intents(remote).len(), 1);
    assert_eq!(
        restored.workspace(workspace).ok_or("workspace")?.projects,
        [local_project, api].into()
    );
    Ok(())
}
