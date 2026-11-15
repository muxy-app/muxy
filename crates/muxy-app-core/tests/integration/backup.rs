#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::collections::BTreeMap;

use muxy_app_core::{
    AppState, Direction, ServerId, backup,
    settings::{InterfaceSize, Settings},
};
use serde_json::json;

#[test]
fn one_x_settings_keep_unrelated_preferences_and_skip_unsupported_values() {
    let mut original = Settings::default();
    original.window.default_size = [1400.0, 900.0];
    let source = json!({
        "muxy.theme.dark": "Dracula",
        "muxy.showExtensionIconRail": true,
        "muxy.general.autoCopyTerminalSelection": true,
        "muxy.appLayout": "agentsFocused",
        "muxy.activeSidebar": "workspace-nav",
        "muxy.quickTerminal.width": 900,
        "muxy.quickTerminal.height": 0,
        "muxy.projectPicker.defaultDirectory": "/projects",
        "editor.richInputImageStrategy": "inlinePath",
        "muxy.richInput.floating": false,
        "muxy.richInput.clearAfterSending": true,
        "muxy.ai.repositoryActions.createPullRequest.prompt": "My instructions",
        "muxy.recording.autoSend": true,
        "muxy.localization": "language-packs:ko",
        "muxy.browser.homePage": "https://example.com",
        "mobile.approvedDevices": [{"token":"never-copy"}]
    });
    let (settings, report) =
        backup::import_settings(&serde_json::to_vec(&source).unwrap(), &original).unwrap();
    assert_eq!(settings.window.default_size, original.window.default_size);
    assert_eq!(settings.quick_terminal.width, 900);
    assert_eq!(
        settings.quick_terminal.height,
        original.quick_terminal.height
    );
    assert_eq!(
        settings.appearance.layout,
        muxy_app_core::settings::AppLayout::AgentsFocused
    );
    assert_eq!(settings.appearance.extension_sidebar, "workspace-nav");
    assert!(settings.appearance.rail_expanded);
    assert!(settings.composer.pinned);
    assert_eq!(settings.appearance.language, "language-packs:ko");
    assert_eq!(settings.ai.prompts["create_pr"], "My instructions");
    assert_eq!(report.skipped.len(), 4);
    let source = backup::settings_source(&settings).unwrap();
    assert!(!source.contains("never-copy"));
    assert!(!source.contains("example.com"));
    let loaded: Settings = toml::from_str(&source).unwrap();
    assert_eq!(settings, loaded);
}

#[test]
fn shortcuts_and_commands_migrate_without_running_commands() {
    let source = json!({
        "shortcuts.app": {
            "newTab": {"key":"n", "modifiers":1_048_576},
            "newBrowserTab": {"key":"b", "modifiers":1_048_576}
        },
        "shortcuts.customCommands": {
            "prefixCombo":{"key":"", "modifiers":0},
            "shortcuts":[{"id":"46E29D7B-7E44-4231-89B6-8810484E4FD7", "name":"Build", "command":"cargo build", "combo":{"key":"b", "modifiers":786_432}}]
        }
    });
    let (settings, report) =
        backup::import_settings(&serde_json::to_vec(&source).unwrap(), &Settings::default())
            .unwrap();
    assert_eq!(
        settings.keymap.binding("new_tab").unwrap().as_str(),
        if cfg!(target_os = "macos") {
            "cmd-n"
        } else {
            "ctrl-n"
        }
    );
    assert_eq!(settings.commands[0].command, "cargo build");
    assert!(
        settings
            .keymap
            .binding(&settings.commands[0].shortcut_id())
            .is_some()
    );
    assert_eq!(
        settings.keymap.binding("new_browser_tab").unwrap().as_str(),
        if cfg!(target_os = "macos") {
            "cmd-b"
        } else {
            "ctrl-b"
        }
    );
    assert!(report.skipped.is_empty());
}

#[test]
fn portable_restore_clears_sessions_and_queues_projects_before_catalog_reconciliation() {
    let directory = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let project = state
        .add_project(ServerId::local(), directory.path().into())
        .unwrap();
    state.open_terminal_tab(project).unwrap();
    let pane = state.project(project).unwrap().tabs[0].panes[0].id;
    state.split_pane(pane, Direction::Right).unwrap();
    state
        .set_pane_session(pane, muxy_protocol::SessionId::new(42))
        .unwrap();
    let portable = state.configuration_backup();
    assert!(portable.session_references(ServerId::local()).is_empty());
    assert!(portable.project_intents(ServerId::local()).is_empty());
    let current = AppState::bootstrap().unwrap();
    let mut restored = portable.restore_configuration(&current).unwrap();
    assert_ne!(restored.project(project).unwrap().tabs[0].panes[0].id, pane);
    let creates = restored
        .project_intents(ServerId::local())
        .iter()
        .filter(|intent| matches!(intent.mutation, muxy_protocol::ProjectMutation::Create(_)))
        .count();
    assert_eq!(creates, 1);
    assert_eq!(restored.project(project).unwrap().tabs[0].panes.len(), 2);
    restored
        .apply_catalog(
            ServerId::local(),
            &muxy_protocol::CatalogPage {
                server: muxy_protocol::ServerIdentity::new(),
                revision: 0,
                home: current.home().id,
                projects: vec![current.home().descriptor()],
                next: None,
                legacy_home: None,
            },
        )
        .unwrap();
    assert!(restored.project(project).is_some());
}

#[test]
fn legacy_projects_restore_groups_and_split_terminal_layouts_and_skip_remote_data() {
    let folder = tempfile::tempdir().unwrap();
    let project = "01000000-0000-0000-0000-000000000001";
    let remote = "01000000-0000-0000-0000-000000000002";
    let tab = "02000000-0000-0000-0000-000000000001";
    let child = "02000000-0000-0000-0000-000000000002";
    let projects = json!([
        {"id":project,"name":"Example","path":folder.path(),"sortOrder":0,"pullRequestPrompt":"Project instructions"},
        {"id":remote,"name":"Remote","path":"/remote","sortOrder":1,"remoteDeviceID":remote}
    ]);
    let area = |id: &str, parent| json!({"type":"tabArea","tabArea":{"tabs":[{"id":id,"parentTabID":parent,"kind":"terminal","paneTitle":"Shell","isPinned":true}]}});
    let workspaces = json!([{"projectID":project,"root":{"type":"split","split":{"direction":"horizontal","ratio":0.6,"first":area(tab, None::<&str>),"second":area(child, Some(tab))}}}]);
    let groups = json!([{"id":"03000000-0000-0000-0000-000000000001","name":"Work","type":"local","projectIDs":[project,remote]}]);
    let mut settings = Settings::default();
    let (state, report) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        Some(&serde_json::to_vec(&workspaces).unwrap()),
        Some(&serde_json::to_vec(&groups).unwrap()),
        &BTreeMap::new(),
        &mut settings,
    )
    .unwrap();
    let project = state.project(project.parse().unwrap()).unwrap();
    assert_eq!(project.tabs.len(), 1);
    assert_eq!(project.tabs[0].panes.len(), 2);
    assert!(project.tabs[0].pinned);
    assert_eq!(state.workspaces()[0].projects.len(), 1);
    assert!(report.attention[0].contains("Remote"));
    assert_eq!(
        settings.ai.project_pr_prompts[&project.id.to_string()],
        "Project instructions"
    );
}

#[test]
fn matching_project_directories_keep_local_identity_and_remap_project_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let mut current = AppState::bootstrap().unwrap();
    let current_id = current
        .add_project(ServerId::local(), directory.path().into())
        .unwrap();
    let mut backup = AppState::bootstrap().unwrap();
    let backup_id = backup
        .add_project(ServerId::local(), directory.path().into())
        .unwrap();
    backup.open_terminal_tab(backup_id).unwrap();
    let tab = backup.open_terminal_tab(backup_id).unwrap();
    let state = backup.restore_configuration(&current).unwrap();
    assert_eq!(state.projects().len(), 2);
    assert!(state.project(backup_id).is_none());
    assert_eq!(state.project(current_id).unwrap().tabs.len(), 2);
    assert_ne!(state.window().selected_tab[&current_id], tab);
    assert_eq!(
        state.window().selected_tab[&current_id],
        state.project(current_id).unwrap().tabs[1].id
    );
    let mut settings = Settings::default();
    settings.appearance.hidden_worktrees.insert(backup_id);
    settings.appearance.project_recent = vec![backup_id, backup.home().id];
    settings
        .ai
        .project_pr_prompts
        .insert(backup_id.to_string(), "Instructions".into());
    let source = backup::remap_settings(
        &backup::settings_source(&settings).unwrap(),
        &backup,
        &state,
    )
    .unwrap();
    let settings: Settings = toml::from_str(&source).unwrap();
    assert!(settings.appearance.hidden_worktrees.contains(&current_id));
    assert_eq!(
        settings.appearance.project_recent,
        [current_id, current.home().id]
    );
    assert_eq!(
        settings.ai.project_pr_prompts[&current_id.to_string()],
        "Instructions"
    );
}

#[test]
fn one_x_worktrees_keep_parent_relationships_and_terminal_layouts() {
    let folder = tempfile::tempdir().unwrap();
    let checkout = tempfile::tempdir().unwrap();
    std::fs::write(
        checkout.path().join(".git"),
        "gitdir: /repository/.git/worktrees/feature",
    )
    .unwrap();
    let project = "01000000-0000-0000-0000-000000000001";
    let worktree = "01000000-0000-0000-0000-000000000002";
    let missing = "01000000-0000-0000-0000-000000000003";
    let projects = json!([{ "id":project,"name":"Project","path":folder.path(),"sortOrder":0 }]);
    let worktrees = json!([
        { "id":worktree,"name":"Feature","path":checkout.path(),"isPrimary":false },
        { "id":missing,"name":"Deleted","path":"/missing-muxy-worktree","isPrimary":false }
    ]);
    let workspaces = json!([{ "projectID":project,"worktreeID":worktree,"root":{"type":"tabArea","tabArea":{"tabs":[{"kind":"terminal","id":"02000000-0000-0000-0000-000000000001","paneTitle":"Shell"}]}} }]);
    let assets = BTreeMap::from([(
        format!("worktrees/{project}.json"),
        serde_json::to_vec(&worktrees).unwrap(),
    )]);
    let (state, report) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        Some(&serde_json::to_vec(&workspaces).unwrap()),
        None,
        &assets,
        &mut Settings::default(),
    )
    .unwrap();
    let worktree = state.project(worktree.parse().unwrap()).unwrap();
    assert_eq!(worktree.parent_id, Some(project.parse().unwrap()));
    assert_eq!(worktree.tabs.len(), 1);
    assert!(state.project(missing.parse().unwrap()).is_none());
    assert!(report.attention.iter().any(|item| item.contains("Deleted")));
}

#[test]
fn one_x_import_keeps_every_usable_project_when_some_details_are_stale() {
    let folders: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let symbol = "01000000-0000-0000-0000-000000000001";
    let emoji = "01000000-0000-0000-0000-000000000002";
    let outdated = "01000000-0000-0000-0000-000000000003";
    let projects = json!([
        {"id":symbol,"name":"Symbol","path":folders[0].path(),"sortOrder":0,"icon":"chart.pie.fill","iconColor":"indigo"},
        {"id":emoji,"name":"Emoji","path":folders[1].path(),"sortOrder":1,"icon":"🚀","iconColor":"#E5484D"},
        {"id":outdated,"name":"Stale","path":folders[2].path(),"sortOrder":2,"icon":"not a symbol!","logo":"stale.png"},
        {"id":"01000000-0000-0000-0000-000000000004","name":"Missing","path":"/missing-muxy-project","sortOrder":3},
        {"id":symbol,"name":"Duplicate","path":folders[0].path(),"sortOrder":4}
    ]);
    let home_tab = json!({"id":"02000000-0000-0000-0000-000000000001","paneTitle":"◐ Shell","customTitle":"🚀 日本語","isPinned":false});
    let home_child = json!({"id":"02000000-0000-0000-0000-000000000002","parentTabID":"02000000-0000-0000-0000-000000000001","paneTitle":"👩🏽‍💻","isPinned":false});
    let workspaces = json!([
        {"projectID":"00000000-0000-0000-0000-000000000001","root":{"type":"split","split":{"direction":"horizontal","ratio":0.95,"first":{"type":"tabArea","tabArea":{"tabs":[home_tab]}},"second":{"type":"tabArea","tabArea":{"tabs":[home_child]}}}}},
        {"projectID":symbol,"root":{"type":"unknown"}}
    ]);
    let assets = BTreeMap::from([("logos/stale.png".to_owned(), b"not a PNG".to_vec())]);
    let (state, report) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        Some(&serde_json::to_vec(&workspaces).unwrap()),
        None,
        &assets,
        &mut Settings::default(),
    )
    .unwrap();
    assert_eq!(state.projects().len(), 4);
    let symbol = state.project(symbol.parse().unwrap()).unwrap();
    assert_eq!(symbol.icon.as_deref(), Some("sf:chart.pie.fill"));
    let emoji = state.project(emoji.parse().unwrap()).unwrap();
    assert_eq!(emoji.icon.as_deref(), Some("🚀"));
    assert_eq!(emoji.color.as_str(), "#e5484d");
    let outdated = state.project(outdated.parse().unwrap()).unwrap();
    assert!(outdated.icon.is_none() && outdated.logo.is_none());
    assert_eq!(state.home().tabs.len(), 1);
    assert_eq!(state.home().tabs[0].panes.len(), 2);
    assert_eq!(state.home().tabs[0].custom_title.as_deref(), Some("日本語"));
    assert_eq!(state.home().tabs[0].panes[0].title, "Shell");
    assert_eq!(state.home().tabs[0].panes[1].title, "Terminal");
    for item in [
        "Icon of Stale",
        "Logo of Stale",
        "Project Missing (folder not found)",
        "Layout (",
    ] {
        assert!(
            report.attention.iter().any(|entry| entry.starts_with(item)),
            "{item}"
        );
    }
}

#[test]
fn merging_one_x_keeps_existing_projects_tabs_workspaces_and_preferences() {
    let shared = tempfile::tempdir().unwrap();
    let added = tempfile::tempdir().unwrap();
    let mut current = AppState::bootstrap().unwrap();
    let mine = current
        .add_project(ServerId::local(), shared.path().into())
        .unwrap();
    current.rename_project(mine, "Mine").unwrap();
    current.open_terminal_tab(mine).unwrap();
    let work = current.create_workspace("Work").unwrap();
    current.set_workspace_member(work, mine, true).unwrap();
    let old_shared = "ff000000-0000-0000-0000-000000000001";
    let old_added = "01000000-0000-0000-0000-000000000002";
    let projects = json!([
        {"id":old_shared,"name":"Shared","path":shared.path(),"sortOrder":0,"pullRequestPrompt":"Theirs"},
        {"id":old_added,"name":"Added","path":added.path(),"sortOrder":1}
    ]);
    let area = |id: &str| json!({"type":"tabArea","tabArea":{"tabs":[{"id":id,"paneTitle":"Shell","isPinned":false}]}});
    let workspaces = json!([
        {"projectID":old_shared,"root":{"type":"split","split":{"direction":"vertical","ratio":0.5,"first":area("02000000-0000-0000-0000-000000000001"),"second":area("02000000-0000-0000-0000-000000000002")}}},
        {"projectID":"00000000-0000-0000-0000-000000000001","root":area("02000000-0000-0000-0000-000000000003")}
    ]);
    let groups = json!([{"id":"03000000-0000-0000-0000-000000000001","name":"work","type":"local","projectIDs":[old_shared,old_added]}]);
    let mut settings = Settings::default();
    settings
        .ai
        .project_pr_prompts
        .insert(mine.to_string(), "Mine".into());
    let (imported, _) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        Some(&serde_json::to_vec(&workspaces).unwrap()),
        Some(&serde_json::to_vec(&groups).unwrap()),
        &BTreeMap::new(),
        &mut settings,
    )
    .unwrap();
    let merged = imported.merge_configuration(&current).unwrap();
    let project = merged.project(mine).unwrap();
    assert_eq!(project.name, "Mine");
    assert_eq!(project.tabs, current.project(mine).unwrap().tabs);
    assert!(merged.project(old_shared.parse().unwrap()).is_none());
    let added: muxy_app_core::ProjectId = old_added.parse().unwrap();
    assert_eq!(merged.projects().last().unwrap().id, added);
    assert_eq!(merged.home().tabs.len(), 1);
    assert_eq!(
        merged.window().current_project,
        current.window().current_project
    );
    assert_eq!(merged.workspaces().len(), 1);
    assert_eq!(
        merged.workspaces()[0].projects,
        [mine, added].into_iter().collect()
    );
    assert_eq!(
        merged.project_intents(ServerId::local()).len(),
        current.project_intents(ServerId::local()).len() + 1
    );
    let source = backup::remap_settings(
        &backup::settings_source(&settings).unwrap(),
        &imported,
        &merged,
    )
    .unwrap();
    let settings: Settings = toml::from_str(&source).unwrap();
    assert_eq!(settings.ai.project_pr_prompts[&mine.to_string()], "Mine");
}

#[test]
fn one_x_shell_command_becomes_the_default_shell_unless_one_is_chosen() {
    let shell = if cfg!(windows) {
        "C:/Windows/System32/cmd.exe"
    } else {
        "/bin/sh"
    };
    let mut report = backup::ImportReport::default();
    let server = backup::import_shell(&format!("command = \"{shell}\"\n"), None, &mut report)
        .unwrap()
        .unwrap();
    assert!(server.contains(&format!("default_shell = \"{shell}\"")));
    let chosen = "default_shell = \"/bin/zsh\"\n";
    assert!(
        backup::import_shell("command = /bin/sh\n", Some(chosen), &mut report)
            .unwrap()
            .is_none()
    );
    assert!(
        backup::import_shell("command = fish -l\n", None, &mut report)
            .unwrap()
            .is_none()
    );
    assert!(report.attention.iter().any(|item| item.contains("fish -l")));
}

#[test]
fn the_interface_size_migrates_from_muxy_1() {
    for (old, size) in [
        ("regular", InterfaceSize::Regular),
        ("large", InterfaceSize::Large),
        ("extraLarge", InterfaceSize::ExtraLarge),
        ("huge", InterfaceSize::Huge),
    ] {
        let source = json!({ "muxy.ui.scale": old });
        let (settings, report) =
            backup::import_settings(&serde_json::to_vec(&source).unwrap(), &Settings::default())
                .unwrap();
        assert_eq!(settings.appearance.interface_size, size);
        assert!(report.skipped.is_empty());
    }
}

#[test]
fn one_x_close_tab_and_close_pane_defaults_take_the_two_x_defaults() {
    let source = json!({
        "shortcuts.app": {
            "closeTab": {"key":"w", "modifiers":1_048_576},
            "closePane": {"key":"w", "modifiers":1_179_648}
        }
    });
    let (settings, report) =
        backup::import_settings(&serde_json::to_vec(&source).unwrap(), &Settings::default())
            .unwrap();
    assert_eq!(settings.keymap, Settings::default().keymap);
    assert_eq!(report.imported, 2);
    assert!(report.attention.is_empty());
}

#[test]
fn restoring_never_replaces_a_remote_project_that_shares_a_local_projects_id() {
    let directory = tempfile::tempdir().unwrap();
    let (remote, remote_home, api) = (
        ServerId::new(),
        muxy_app_core::ProjectId::new(),
        muxy_app_core::ProjectId::new(),
    );
    let descriptor = |id, home, name: &str| muxy_protocol::ProjectDescriptor {
        id,
        home,
        name: name.into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: muxy_protocol::ServerPath::from(format!("/home/dev/{name}")),
        kind: None,
        parent_id: None,
    };
    let mut current = AppState::bootstrap().unwrap();
    current
        .apply_catalog(
            remote,
            &muxy_protocol::CatalogPage {
                server: muxy_protocol::ServerIdentity::from_u128(2),
                home: remote_home,
                revision: 7,
                projects: vec![
                    descriptor(remote_home, true, "Home"),
                    descriptor(api, false, "api"),
                ],
                next: None,
                legacy_home: None,
            },
        )
        .unwrap();
    current.open_terminal_tab(api).unwrap();

    let mut exported = AppState::bootstrap().unwrap();
    let local = exported
        .add_project(ServerId::local(), directory.path().into())
        .unwrap();
    let source = serde_json::to_string(&exported.configuration_backup())
        .unwrap()
        .replace(&local.to_string(), &api.to_string());
    let imported: AppState = serde_json::from_str(&source).unwrap();

    for restored in [
        imported.restore_configuration(&current).unwrap(),
        imported.merge_configuration(&current).unwrap(),
    ] {
        let kept = restored.project(api).unwrap();
        assert_eq!(kept.server_id, remote);
        assert_eq!(kept.tabs.len(), 1);
        let copies = restored
            .projects()
            .iter()
            .filter(|project| project.server_id.is_local() && !project.home)
            .filter(|project| project.id != api)
            .count();
        assert_eq!(copies, 1);
    }
}

#[test]
fn one_x_projects_share_logos_and_stop_at_the_logo_budget() {
    let folder = tempfile::tempdir().unwrap();
    let mut logo = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    logo.extend(64_u32.to_be_bytes());
    logo.extend(64_u32.to_be_bytes());
    logo.resize(300 * 1024, 0);
    let projects: Vec<_> = (0..80)
        .map(|index| {
            let path = folder.path().join(format!("p{index}"));
            std::fs::create_dir(&path).unwrap();
            json!({
                "id": format!("01000000-0000-0000-0000-{:012}", index + 10),
                "name": format!("P{index}"),
                "path": path,
                "sortOrder": index,
                "logo": "shared.png",
            })
        })
        .collect();
    let assets = BTreeMap::from([("logos/shared.png".to_owned(), logo)]);
    let (state, report) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        None,
        None,
        &assets,
        &mut Settings::default(),
    )
    .unwrap();
    let logos: Vec<_> = state
        .projects()
        .iter()
        .filter_map(|project| project.logo.clone())
        .collect();
    assert_eq!(logos.len(), 54);
    assert!(
        logos
            .iter()
            .all(|logo| std::sync::Arc::ptr_eq(logo, &logos[0]))
    );
    assert_eq!(state.projects().len(), 81);
    assert_eq!(
        report
            .attention
            .iter()
            .filter(|note| note.contains("too many logos"))
            .count(),
        26
    );
}

#[test]
fn one_x_layouts_with_many_tabs_import_in_linear_time() {
    let folder = tempfile::tempdir().unwrap();
    let project = "01000000-0000-0000-0000-000000000001";
    let projects = json!([{"id":project,"name":"Example","path":folder.path(),"sortOrder":0}]);
    let tabs: Vec<_> = (0..20_000)
        .map(|index| json!({"id":index,"kind":"terminal"}))
        .collect();
    let order: Vec<_> = (0..20_000).rev().collect();
    let workspaces = json!([{
        "projectID": project,
        "topLevelTabOrder": order,
        "root": {"type":"tabArea","tabArea":{"tabs":tabs}},
    }]);
    let started = std::time::Instant::now();
    let (state, _) = backup::import_projects(
        &serde_json::to_vec(&projects).unwrap(),
        Some(&serde_json::to_vec(&workspaces).unwrap()),
        None,
        &BTreeMap::new(),
        &mut Settings::default(),
    )
    .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    let project = state.project(project.parse().unwrap()).unwrap();
    assert_eq!(project.tabs.len(), 20_000);
}

#[test]
fn one_x_commands_are_checked_before_anything_is_copied() {
    let invalid: Vec<_> = (0..5_000)
        .map(|_| json!({"name":"Broken","command":"true"}))
        .collect();
    let source = json!({
        "muxy.ai.repositoryActions.createPullRequest.prompt": "x".repeat(1 << 20),
        "shortcuts.customCommands": {
            "prefixCombo": {"key":"", "modifiers":0},
            "shortcuts": invalid,
        }
    });
    let started = std::time::Instant::now();
    let (_, report) =
        backup::import_settings(&serde_json::to_vec(&source).unwrap(), &Settings::default())
            .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    let broken = report
        .attention
        .iter()
        .filter(|note| note.as_str() == "command Broken")
        .count();
    assert_eq!(broken, 1024);
    assert!(
        report
            .attention
            .iter()
            .any(|note| note.starts_with("3976 more custom commands"))
    );
}
