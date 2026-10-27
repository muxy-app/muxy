use muxy_app_core::browser::{BrowserPane, resolve_target};
use muxy_app_core::settings::Settings;
use muxy_app_core::{AppState, PaneContent, ProjectId, ProjectKind, ServerId, restore};
use muxy_protocol::{CatalogPage, ProjectDescriptor, ServerIdentity, ServerPath};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn browser(url: &str) -> BrowserPane {
    BrowserPane::new(Some(url.into()), None)
}

#[test]
fn browser_tabs_open_split_and_survive_a_restart() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let (tab, pane) = state.open_browser(home, browser("https://example.com"), true)?;
    assert_eq!(state.window().active_pane, Some(pane));
    let split = state.split_browser(pane, BrowserPane::default(), true)?;
    assert_eq!(state.window().active_pane, Some(split));
    assert!(state.set_browser_url(split, Some("https://docs.rs".into()))?);
    assert!(!state.set_browser_url(split, Some("https://docs.rs".into()))?);
    assert!(state.set_browser_profile(pane, Some("work".into()))?);

    let saved = serde_json::to_vec(&state)?;
    let mut restored: AppState = serde_json::from_slice(&saved)?;
    assert_eq!(restored, state);
    let plan = restore::plan(&restored, ServerId::local(), &[]);
    assert!(plan.create.is_empty() && plan.attach.is_empty());
    restored.clear_terminal_panes(ServerId::local())?;
    let panes = &restored.home().tabs[0].panes;
    assert_eq!(restored.home().tabs[0].id, tab);
    assert_eq!(
        panes[0].content,
        PaneContent::Browser(BrowserPane::new(
            Some("https://example.com".into()),
            Some("work".into())
        ))
    );
    assert_eq!(
        restored
            .browser(split)
            .and_then(|browser| browser.url.as_deref()),
        Some("https://docs.rs")
    );
    assert!(restored.set_pane_session(pane, None).is_err());
    Ok(())
}

#[test]
fn browser_panes_are_found_by_their_own_or_their_tab_id() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let (browser_tab, first) = state.open_browser(home, browser("https://example.com"), true)?;
    let second = state.split_browser(first, BrowserPane::default(), true)?;
    assert_eq!(state.browser_pane(&first.to_string()), Some(first));
    assert_eq!(state.browser_pane(&browser_tab.to_string()), Some(second));

    let terminal_tab = state.open_terminal_tab(home)?;
    let terminal = state.window().active_pane.ok_or("terminal pane")?;
    let beside = state.split_browser(terminal, BrowserPane::default(), false)?;
    assert_eq!(state.browser_pane(&terminal_tab.to_string()), Some(beside));
    assert_eq!(state.browser_pane(&terminal.to_string()), None);
    assert_eq!(state.browser_pane("nope"), None);
    Ok(())
}

#[test]
fn blank_pages_are_not_saved_as_addresses() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let (_, pane) = state.open_browser(home, browser("about:blank"), true)?;
    assert_eq!(state.browser(pane), Some(&BrowserPane::default()));
    state.set_browser_url(pane, Some("https://example.com".into()))?;
    assert!(state.set_browser_url(pane, Some("about:blank".into()))?);
    assert_eq!(state.browser(pane).and_then(|b| b.url.clone()), None);
    Ok(())
}

#[test]
fn background_tabs_leave_the_view_alone() -> TestResult {
    let (mut state, ids) = projects()?;
    state.select_project(ids.api)?;
    let before = state.window().clone();
    let (tab, _) = state.open_browser(ids.web_feature, browser("https://a.dev"), false)?;
    assert_eq!(state.window().current_project, before.current_project);
    assert_eq!(state.window().active_pane, before.active_pane);
    let web_feature = state.project(ids.web_feature).ok_or("missing worktree")?;
    assert!(web_feature.tabs.iter().any(|candidate| candidate.id == tab));
    Ok(())
}

struct Ids {
    api: ProjectId,
    web: ProjectId,
    web_feature: ProjectId,
    api_feature: ProjectId,
}

/// A remote server with projects `api` and `web`, each with a `feature`
/// worktree.
fn projects() -> TestResult<(AppState, Ids)> {
    let mut state = AppState::bootstrap()?;
    let home = ProjectId::new();
    let ids = Ids {
        api: ProjectId::new(),
        web: ProjectId::new(),
        web_feature: ProjectId::new(),
        api_feature: ProjectId::new(),
    };
    let descriptor =
        |id, name: &str, directory: &str, parent: Option<ProjectId>| ProjectDescriptor {
            id,
            home: id == home,
            name: name.into(),
            icon: None,
            logo: None,
            color: "#808080".into(),
            directory: ServerPath(directory.as_bytes().to_vec()),
            kind: parent.map(|_| ProjectKind::Worktree),
            parent_id: parent,
        };
    state.apply_catalog(
        ServerId::new(),
        &CatalogPage {
            server: ServerIdentity::from_u128(9),
            home,
            revision: 1,
            projects: vec![
                descriptor(home, "Home", "/srv", None),
                descriptor(ids.api, "api", "/srv/api", None),
                descriptor(ids.web, "web", "/srv/web", None),
                descriptor(
                    ids.api_feature,
                    "feature",
                    "/srv/wt/api-feature",
                    Some(ids.api),
                ),
                descriptor(
                    ids.web_feature,
                    "feature",
                    "/srv/wt/web-feature",
                    Some(ids.web),
                ),
            ],
            next: None,
            legacy_home: None,
        },
    )?;
    Ok((state, ids))
}

#[test]
fn targets_resolve_projects_and_worktrees() -> TestResult {
    let (mut state, ids) = projects()?;
    state.select_project(ids.web)?;
    assert_eq!(resolve_target(&state, None, None), Ok(ids.web));
    assert_eq!(resolve_target(&state, Some("api"), None), Ok(ids.api));
    assert_eq!(resolve_target(&state, Some("/srv/api"), None), Ok(ids.api));
    assert_eq!(
        resolve_target(&state, Some(&ids.api.to_string()), None),
        Ok(ids.api)
    );
    assert_eq!(
        resolve_target(&state, None, Some("feature")),
        Ok(ids.web_feature)
    );
    assert_eq!(
        resolve_target(&state, Some("api"), Some("feature")),
        Ok(ids.api_feature)
    );
    assert_eq!(
        resolve_target(&state, None, Some("api-feature")),
        Ok(ids.api_feature)
    );
    assert_eq!(
        resolve_target(&state, Some("nope"), None),
        Err("no project named 'nope'".into())
    );
    state.select_project(state.projects()[0].id)?;
    assert_eq!(
        resolve_target(&state, None, Some("feature")),
        Err(
            "worktree 'feature' is ambiguous across projects; pass --project to disambiguate"
                .into()
        )
    );
    assert_eq!(
        resolve_target(&state, Some("web"), Some("missing")),
        Err("no worktree named 'missing'".into())
    );
    Ok(())
}

#[test]
fn browser_settings_have_safe_defaults_and_check_the_home_page() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("settings.toml");
    std::fs::write(
        &path,
        "[browser]\nsearch_engine = \"duckduckgo\"\nhome_page = \"example.com\"\n",
    )?;
    let settings = Settings::load(&path)?;
    assert_eq!(
        settings.browser.home_url().as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        settings.browser.resolve("a b").as_deref(),
        Some("https://duckduckgo.com/?q=a+b")
    );

    let mut invalid = settings.clone();
    invalid.browser.home_page = "not a page".into();
    assert!(invalid.save_browser(&path).is_err());
    let mut changed = settings;
    changed.browser.open_terminal_links = true;
    changed.save_browser(&path)?;
    assert!(Settings::load(&path)?.browser.open_terminal_links);
    Ok(())
}
