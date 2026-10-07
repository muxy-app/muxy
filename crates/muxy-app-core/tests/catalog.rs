use muxy_app_core::{AppState, ProjectId, ServerId, store};
use muxy_protocol::{CatalogPage, ProjectPatch, ServerIdentity, ServerPath, SessionId};
use std::fs;
use std::path::Path;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn snapshot(state: &AppState) -> CatalogPage {
    CatalogPage {
        server: ServerIdentity::from_u128(1),
        home: state.home().id,
        revision: 1,
        projects: state
            .projects()
            .iter()
            .map(muxy_app_core::Project::descriptor)
            .collect(),
        next: None,
        legacy_home: None,
    }
}
fn acknowledge(state: &mut AppState) -> Result {
    while let Some(intent) = state.project_intents(ServerId::local()).first().cloned() {
        state.complete_project_intent(ServerId::local(), intent.operation)?;
    }
    Ok(())
}

#[test]
fn shared_metadata_refresh_keeps_layout_and_overlays_only_pending_fields() -> Result {
    let mut state = AppState::bootstrap()?;
    let project = state.add_project(ServerId::local(), std::env::temp_dir())?;
    state.open_terminal_tab(project)?;
    acknowledge(&mut state)?;
    let mut catalog = snapshot(&state);
    state.apply_catalog(ServerId::local(), &catalog)?;
    let window = state.window().clone();
    let tabs = state.current_project().tabs.clone();
    state.rename_project(project, "Offline name")?;
    let pending = state.project_intents(ServerId::local()).to_vec();
    let descriptor = catalog
        .projects
        .iter_mut()
        .find(|entry| entry.id == project)
        .ok_or("project")?;
    descriptor.name = "Other client name".into();
    ProjectPatch::Color("#123456".into()).apply(descriptor);
    catalog.revision += 1;
    state.apply_catalog(ServerId::local(), &catalog)?;
    assert_eq!(state.current_project().name, "Offline name");
    assert_eq!(state.current_project().color.as_str(), "#123456");
    assert_eq!(state.current_project().tabs, tabs);
    assert_eq!(state.window(), &window);
    assert_eq!(state.project_intents(ServerId::local()), pending);
    acknowledge(&mut state)?;
    state.apply_catalog(ServerId::local(), &catalog)?;
    assert_eq!(state.current_project().name, "Other client name");
    Ok(())
}

#[test]
fn server_home_remap_preserves_panes_focus_and_cleanup_order() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let pane = state.home().tabs[0].panes[0].id;
    let first = SessionId::new(1).ok_or("session")?;
    let second = SessionId::new(2).ok_or("session")?;
    state.set_pane_session(pane, Some(first))?;
    state.queue_discard(ServerId::local(), second);
    state.queue_discard(ServerId::local(), first);
    let mut catalog = snapshot(&state);
    let old_home = catalog.home;
    catalog.home = ProjectId::new();
    catalog.legacy_home = Some(old_home);
    catalog.projects[0].id = catalog.home;
    state.apply_catalog(ServerId::local(), &catalog)?;
    assert_eq!(state.home().id, catalog.home);
    assert_eq!(state.window().current_project, catalog.home);
    assert_eq!(state.window().active_pane, Some(pane));
    assert_eq!(state.pending_discards(ServerId::local()), [second, first]);
    assert_eq!(state.home().tabs[0].panes[0].id, pane);
    assert!(!state.window().selected_tab.contains_key(&old_home));
    Ok(())
}

#[test]
fn migration_preserves_original_file_and_pending_creation_directory_and_token() -> Result {
    let directory =
        std::env::temp_dir().join(format!("muxy-client-migration-{}", ProjectId::new()));
    fs::create_dir(&directory)?;
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let pane = state.home().tabs[0].panes[0].id;
    let mut legacy = serde_json::to_value(&state)?;
    legacy["version"] = 1.into();
    let original = serde_json::to_vec(&legacy)?;
    fs::write(directory.join("state.json"), &original)?;
    let path = directory.join("desktop-state.json");
    let mut migrated = store::load(&path)?;
    assert_eq!(migrated.version(), 3);
    let initial = migrated.prepare_creation(pane, Path::new("/tmp"));
    store::save(&path, &migrated)?;
    let mut restored = store::load(&path)?;
    assert_eq!(
        restored.prepare_creation(pane, Path::new("/different")),
        initial
    );
    assert_eq!(
        restored.home().tabs[0].panes[0].id.creation_token(),
        pane.creation_token()
    );
    assert_eq!(fs::read(directory.join("state.json"))?, original);
    restored.close_pane(pane)?;
    store::save(&path, &restored)?;
    assert_eq!(
        store::load(&path)?.pending_cancellations(ServerId::local()),
        [pane.creation_token()]
    );
    fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn foreign_session_membership_is_never_inferred_from_directory() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(1).ok_or("session")?;
    state.set_pane_session(pane, Some(session))?;
    let plan = muxy_app_core::restore::plan(
        &state,
        ServerId::local(),
        &[muxy_protocol::SessionInfo {
            sandbox: None,
            id: session,
            project: ProjectId::new(),
            directory: ServerPath(b"/tmp".into()),
        }],
    );
    assert!(plan.attach.is_empty());
    assert!(plan.create.is_empty());
    assert_eq!(plan.close, [(pane, session)]);
    Ok(())
}
