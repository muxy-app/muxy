use super::overlays::Overlay;
use crate::model::AppModel;
use crate::picker::path_service::{self, DirectoryItem, TypedPathState};
use crate::picker::remote::RemoteFolders;
use crate::picker::search::{SearchService, Snapshot};
use crate::picker::session::{InputMode, LoadState, Session};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription,
};
use muxy_app_core::ServerId;
use muxy_ui::icon::Icon;
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent as ListEvent, PickerItem, PickerLeading,
    PickerRow, PickerStatus,
};
use muxy_ui::theme::{Metrics, Theme};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

const RELOAD_DELAY: Duration = Duration::from_millis(100);
const LOADING_MESSAGE_DELAY: Duration = Duration::from_millis(500);
const SEARCH_RESULT_LIMIT: usize = 50;
const DIRECTORY_CACHE_LIMIT: usize = 64;
const DIRECTORY_CACHE_ITEMS: usize = 50_000;

pub(crate) enum PickerEvent {
    Confirm {
        path: String,
        create_if_missing: bool,
    },
    ChooseFinder {
        directory: String,
    },
    EditSearchLocation {
        directory: String,
    },
    Dismiss,
}

pub(crate) struct ProjectPicker {
    session: Session,
    search: SearchService,
    /// Another computer's folders, instead of this one's.
    remote: Option<RemoteFolders>,
    /// Why the folder couldn't be listed, when the server said.
    listing_error: Option<String>,
    picker: Entity<Picker>,
    generation: usize,
    cancelled: Arc<AtomicBool>,
    directory_cache: std::collections::HashMap<String, Vec<DirectoryItem>>,
    directory_cache_order: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PickerEvent> for ProjectPicker {}

impl Focusable for ProjectPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}

impl ProjectPicker {
    pub(crate) fn new(
        search: SearchService,
        search_root: &str,
        project_paths: Vec<String>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let session = Session::new(search_root, project_paths);
        let picker = Self::build(
            session,
            search,
            None,
            "Search folders or enter a path…".into(),
            theme,
            metrics,
            cx,
        );
        let warm_root = picker.session.search_root_path.clone();
        let warm_search = picker.search.clone();
        let cancelled = picker.cancelled.clone();
        cx.background_executor()
            .spawn(async move { warm_search.prepare(&warm_root, &cancelled) })
            .detach();
        picker
    }

    pub(crate) fn remote(
        folders: RemoteFolders,
        project_paths: Vec<String>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let session = Session::remote(&folders.home, project_paths);
        let placeholder = format!("Enter a path on {}…", folders.name);
        Self::build(
            session,
            SearchService::new(),
            Some(folders),
            placeholder,
            theme,
            metrics,
            cx,
        )
    }

    fn build(
        session: Session,
        search: SearchService,
        remote: Option<RemoteFolders>,
        placeholder: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig {
                    completion_on_tab: true,
                    ..PickerConfig::new("project-picker", placeholder)
                },
                theme,
                metrics,
                cx,
            )
        });
        let subscription =
            cx.subscribe(
                &picker,
                |project_picker: &mut Self, _, event, cx| match event {
                    ListEvent::QueryChanged { query, .. } => {
                        project_picker.session.set_input(query.as_ref());
                        project_picker.reload(cx);
                    }
                    ListEvent::Confirmed(selection) => {
                        project_picker.activate(selection.id.as_ref(), cx);
                    }
                    ListEvent::SelectionChanged(selection) => {
                        project_picker.select(selection.id.as_ref(), cx);
                    }
                    ListEvent::SecondaryConfirmed(_) | ListEvent::Submitted { secondary: true } => {
                        project_picker.confirm(true, cx);
                    }
                    ListEvent::Submitted { secondary: false } => {
                        project_picker.confirm(false, cx);
                    }
                    ListEvent::CompletionRequested => {
                        project_picker.complete_highlighted(cx);
                    }
                    ListEvent::NavigateBackRequested => project_picker.go_back(cx),
                    ListEvent::FooterAction(action) => match action.as_ref() {
                        "confirm-path" => project_picker.confirm(true, cx),
                        "finder" => project_picker.choose_finder(cx),
                        "location" => project_picker.edit_search_location(cx),
                        _ => {}
                    },
                    ListEvent::Dismissed => cx.emit(PickerEvent::Dismiss),
                    _ => {}
                },
            );
        let mut project_picker = Self {
            session,
            search,
            remote,
            listing_error: None,
            picker,
            generation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            directory_cache: std::collections::HashMap::new(),
            directory_cache_order: Vec::new(),
            _subscriptions: vec![subscription],
        };
        project_picker.picker.update(cx, |picker, cx| {
            picker.set_query(project_picker.session.input.clone(), cx);
        });
        project_picker.reload(cx);
        project_picker
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.listing_error = None;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let mode = self.session.input_mode();
        let query = self.session.search_query().to_owned();
        let root = self.session.search_root_path.clone();
        let paths = self.session.project_paths.clone();
        let path_state = self.session.path_state();
        let search = self.search.clone();
        let cancelled = self.cancelled.clone();

        if mode == InputMode::FolderSearch && (query.is_empty() || self.remote.is_some()) {
            self.session.apply_search_snapshot(Snapshot::default());
            self.sync_picker(cx);
            return;
        }

        if mode == InputMode::Path
            && let Some(items) = self
                .directory_cache
                .get(&path_state.directory_path)
                .cloned()
        {
            self.session
                .apply_directory_snapshot(path_state.directory_items(items), false);
            self.sync_picker(cx);
            return;
        }

        self.sync_picker(cx);
        cx.spawn(async move |picker, cx| {
            cx.background_executor().timer(LOADING_MESSAGE_DELAY).await;
            let _ = picker.update(cx, |picker, cx| {
                if picker.generation == generation {
                    picker.session.show_loading_message();
                    picker.sync_picker(cx);
                }
            });
        })
        .detach();

        match mode {
            InputMode::FolderSearch => {
                cx.spawn(async move |picker, cx| {
                    cx.background_executor().timer(RELOAD_DELAY).await;
                    if picker.read_with(cx, |picker, _| picker.generation).ok() != Some(generation)
                    {
                        return;
                    }
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move {
                            search.search(&query, &root, &paths, SEARCH_RESULT_LIMIT, &cancelled)
                        })
                        .await;
                    let _ = picker.update(cx, |picker, cx| {
                        if picker.generation == generation {
                            picker.session.apply_search_snapshot(snapshot);
                            picker.sync_picker(cx);
                        }
                    });
                })
                .detach();
            }
            InputMode::Path => self.list_directory(path_state.directory_path, generation, cx),
        }
    }

    /// Lists a folder here, or through another computer's server.
    fn list_directory(&self, directory: String, generation: usize, cx: &mut Context<Self>) {
        let remote = self.remote.clone();
        cx.spawn(async move |picker, cx| {
            cx.background_executor().timer(RELOAD_DELAY).await;
            if picker.read_with(cx, |picker, _| picker.generation).ok() != Some(generation) {
                return;
            }
            let listed = directory.clone();
            let contents = cx
                .background_executor()
                .spawn(async move {
                    match remote {
                        Some(remote) => remote.list(&listed),
                        None => path_service::directory_contents(&listed)
                            .map_err(|error| error.to_string()),
                    }
                })
                .await;
            let _ = picker.update(cx, |picker, cx| {
                if picker.generation != generation {
                    return;
                }
                let path_state = picker.session.path_state();
                picker.listing_error = contents.as_ref().err().cloned();
                match contents {
                    Ok(items) => {
                        picker.cache_items(&directory, items.clone());
                        picker
                            .session
                            .apply_directory_snapshot(path_state.directory_items(items), false);
                    }
                    Err(_) => picker
                        .session
                        .apply_directory_snapshot(path_state.directory_read_failure_items(), true),
                }
                picker.sync_picker(cx);
            });
        })
        .detach();
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let loading = matches!(self.session.load_state, LoadState::Loading { .. });
        let mut items = if loading {
            Vec::new()
        } else {
            match self.session.input_mode() {
                InputMode::FolderSearch => self
                    .session
                    .search_results
                    .iter()
                    .enumerate()
                    .map(|(index, result)| {
                        let mut row =
                            PickerRow::new(format!("search-{index}"), result.name.clone());
                        row.detail = Some(result.display_path.clone().into());
                        row.leading = Some(PickerLeading::Icon(Icon::Folder));
                        PickerItem::Row(row)
                    })
                    .collect(),
                InputMode::Path => self
                    .session
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let mut row = PickerRow::new(
                            format!("path-{index}"),
                            if item.is_parent() {
                                "Parent Directory".to_owned()
                            } else {
                                item.name().to_owned()
                            },
                        );
                        row.leading = Some(PickerLeading::Icon(if item.is_parent() {
                            Icon::ChevronLeft
                        } else {
                            Icon::Folder
                        }));
                        PickerItem::Row(row)
                    })
                    .collect(),
            }
        };
        let unreadable = match (&self.remote, &self.listing_error) {
            (Some(_), Some(error)) => error.clone(),
            _ => "Could not read this folder".to_owned(),
        };
        if !loading && self.session.shows_unavailable_state() && !items.is_empty() {
            let label = if matches!(self.session.load_state, LoadState::Failed) {
                unreadable.clone()
            } else if self.session.input_mode() == InputMode::Path {
                if self.session.path_state().leaf_filter.is_empty() {
                    "Folder is empty".to_owned()
                } else {
                    "No matching folders".to_owned()
                }
            } else {
                "No matching folders".to_owned()
            };
            let mut row = PickerRow::new("path-unavailable", label);
            row.disabled = true;
            items.push(PickerItem::Row(row));
        }
        let status = match self.session.load_state {
            LoadState::Loading {
                shows_message: true,
            } => PickerStatus::Loading("Loading folders…".into()),
            LoadState::Loading {
                shows_message: false,
            } => PickerStatus::Loading("".into()),
            LoadState::Failed if !items.is_empty() => PickerStatus::Ready,
            LoadState::Failed => PickerStatus::Error(unreadable.clone().into()),
            LoadState::Loaded if self.session.shows_unavailable_state() && !items.is_empty() => {
                PickerStatus::Ready
            }
            LoadState::Loaded if self.session.shows_unavailable_state() => {
                PickerStatus::Empty("No matching folders".into())
            }
            LoadState::Loaded => PickerStatus::Ready,
        };
        let ghost = self.session.ghost_text();
        let actions = self.footer_actions();
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            if let Some(index) = self.session.highlighted_index {
                let prefix = match self.session.input_mode() {
                    InputMode::FolderSearch => "search",
                    InputMode::Path => "path",
                };
                let _ = picker.select_row(&format!("{prefix}-{index}"), cx);
            }
            picker.set_footer_actions(actions, cx);
            picker.set_can_navigate_back(self.session.input_mode() == InputMode::Path, cx);
            picker
                .input()
                .update(cx, |input, cx| input.set_ghost(ghost, cx));
        });
    }

    /// Another computer has no Finder or search location.
    fn footer_actions(&self) -> Vec<PickerAction> {
        let title = if self
            .session
            .confirmation_path()
            .is_some_and(|path| self.missing_remote_folder(&path).is_some())
        {
            "Create & Add Project"
        } else {
            self.session.top_right_action_title()
        };
        let mut actions = vec![
            PickerAction::new("confirm-path", title)
                .icon(PickerLeading::Icon(Icon::Plus))
                .disabled(self.session.confirmation_path().is_none()),
        ];
        if self.remote.is_none() {
            actions.push(PickerAction::new("finder", "Finder…"));
            actions.push(PickerAction::new("location", "Search Location…"));
        }
        actions
    }

    fn activate(&mut self, id: &str, cx: &mut Context<Self>) {
        let index = id
            .split_once('-')
            .and_then(|(_, index)| index.parse::<usize>().ok());
        let Some(index) = index else {
            return;
        };
        self.session.select_row(index);
        match self.session.input_mode() {
            InputMode::FolderSearch => self.confirm(false, cx),
            InputMode::Path => {
                let Some(item) = self.session.rows.get(index).cloned() else {
                    return;
                };
                self.session.activate(&item);
                self.apply_input(cx);
            }
        }
    }

    fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = id
            .split_once('-')
            .and_then(|(_, index)| index.parse::<usize>().ok())
        else {
            return;
        };
        self.session.select_row(index);
        let ghost = self.session.ghost_text();
        self.picker
            .read(cx)
            .input()
            .update(cx, |input, cx| input.set_ghost(ghost, cx));
    }

    fn apply_input(&mut self, cx: &mut Context<Self>) {
        let text = self.session.input.clone();
        self.picker
            .update(cx, |picker, cx| picker.set_query(text, cx));
        self.reload(cx);
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        self.session.go_back();
        self.apply_input(cx);
    }

    fn complete_highlighted(&mut self, cx: &mut Context<Self>) {
        self.session.complete_highlighted();
        self.apply_input(cx);
    }

    fn confirm(&self, allow_create: bool, cx: &mut Context<Self>) {
        let Some(path) = self.session.confirmation_path() else {
            return;
        };
        let create_if_missing = allow_create
            && self.session.input_mode() == InputMode::Path
            && (self.remote.is_some()
                || self.session.typed_path_state() == TypedPathState::Missing);
        cx.emit(PickerEvent::Confirm {
            path,
            create_if_missing,
        });
    }

    /// Why `path` on another computer can't be added, when the listing of
    /// its parent already shows it isn't there. Otherwise the server checks.
    fn missing_remote_folder(&self, path: &str) -> Option<String> {
        self.remote.as_ref()?;
        let parent = path_service::parent_path(path);
        let name = path_service::last_component(path);
        let listed = self.directory_cache.get(&parent)?;
        if name.is_empty() || listed.iter().any(|item| item.name() == name) {
            return None;
        }
        let parent = self.session.path_service.abbreviated_display_path(&parent);
        Some(format!("There is no folder named {name} in {parent}"))
    }

    fn choose_finder(&self, cx: &mut Context<Self>) {
        if self.remote.is_some() {
            return;
        }
        cx.emit(PickerEvent::ChooseFinder {
            directory: self.session.path_state().directory_path,
        });
    }

    fn edit_search_location(&self, cx: &mut Context<Self>) {
        if self.remote.is_some() {
            return;
        }
        cx.emit(PickerEvent::EditSearchLocation {
            directory: self.session.search_root_path.clone(),
        });
    }

    fn cache_items(&mut self, directory: &str, items: Vec<DirectoryItem>) {
        if self
            .directory_cache
            .insert(directory.to_owned(), items)
            .is_none()
        {
            self.directory_cache_order.push(directory.to_owned());
        }
        while self.directory_cache_order.len() > DIRECTORY_CACHE_LIMIT
            || self.directory_cache.values().map(Vec::len).sum::<usize>() > DIRECTORY_CACHE_ITEMS
        {
            let evicted = self.directory_cache_order.remove(0);
            self.directory_cache.remove(&evicted);
        }
    }
}

impl Render for ProjectPicker {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}

impl Drop for ProjectPicker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl AppModel {
    pub(crate) fn open_project_picker(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let root = self
            .settings
            .projects
            .search_root
            .as_ref()
            .unwrap_or(&self.state.home().directory)
            .to_string_lossy()
            .into_owned();
        let paths = self.project_paths(ServerId::local());
        let picker = cx.new(|cx| {
            ProjectPicker::new(
                self.picker_search.clone(),
                &root,
                paths,
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        picker.focus_handle(cx).focus(window);
        self.show_project_picker(ServerId::local(), picker, cx);
    }

    /// Opens the same picker on another computer's folders, from its Home.
    /// A server that isn't ready yet connects first, asking for its password
    /// if needed, and the picker opens once it is.
    pub(crate) fn open_remote_project_picker(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let Some(name) = self.server_name(server).map(str::to_owned) else {
            return;
        };
        let home: Option<(muxy_app_core::ProjectId, String)> = self
            .state
            .server_home(server)
            .map(|home| (home.id, home.directory.to_string_lossy().into_owned()));
        let ready = self.server_ready(server);
        let client = self.extensions.client(server);
        let (Some((project, home)), true, true, Some(client)) =
            (home, ready, self.confirmed(server), client)
        else {
            self.pending_remote_picker = Some((server, std::time::Instant::now()));
            if ready && self.extensions.client(server).is_none() {
                self.extension_client(server, cx);
            } else {
                self.connect_remote_server(server, cx);
            }
            cx.notify();
            return;
        };
        self.pending_remote_picker = None;
        let folders = RemoteFolders::through(client, name, project, &home);
        let paths = self.project_paths(server);
        let picker = cx
            .new(|cx| ProjectPicker::remote(folders, paths, self.theme.clone(), self.metrics, cx));
        self.show_project_picker(server, picker, cx);
    }

    /// Opens the remote picker that waited for `server` to be ready. It is
    /// dropped once the user moved on: something else is open, or the wait
    /// was long.
    pub(crate) fn resume_remote_picker(&mut self, server: ServerId, cx: &mut Context<Self>) {
        const PATIENCE: Duration = Duration::from_secs(60);
        let Some((pending, asked)) = self.pending_remote_picker else {
            return;
        };
        if pending != server {
            return;
        }
        if self.overlay.is_some() || asked.elapsed() > PATIENCE {
            self.pending_remote_picker = None;
            return;
        }
        self.open_remote_project_picker(server, cx);
    }

    /// The folders of the projects the sidebar lists on `server`.
    fn project_paths(&self, server: ServerId) -> Vec<String> {
        self.state
            .projects()
            .iter()
            .filter(|project| project.server_id == server && self.project_listed(project))
            .map(|project| project.directory.to_string_lossy().into_owned())
            .collect()
    }

    pub(crate) fn show_project_picker(
        &mut self,
        server: ServerId,
        picker: Entity<ProjectPicker>,
        cx: &mut Context<Self>,
    ) {
        self.focus_later(picker.focus_handle(cx), cx);
        self.overlay_subscription =
            Some(
                cx.subscribe(&picker, move |model, picker, event, cx| match event {
                    PickerEvent::Confirm {
                        path,
                        create_if_missing,
                    } if !server.is_local() => {
                        model.confirm_remote_project_path(
                            server,
                            picker,
                            path,
                            *create_if_missing,
                            cx,
                        );
                    }
                    PickerEvent::Confirm {
                        path,
                        create_if_missing,
                    } => model.confirm_project_path(path, *create_if_missing, cx),
                    PickerEvent::ChooseFinder { directory } => {
                        model.choose_project_folder(directory.clone(), false, cx);
                    }
                    PickerEvent::EditSearchLocation { directory } => {
                        model.choose_project_folder(directory.clone(), true, cx);
                    }
                    PickerEvent::Dismiss => model.dismiss_overlay(cx),
                }),
            );
        self.overlay = Some(Overlay::Projects(picker));
        cx.notify();
    }

    fn confirm_remote_project_path(
        &mut self,
        server: ServerId,
        picker: Entity<ProjectPicker>,
        path: &str,
        allow_create: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let Some(folders) = picker.read(cx).remote.clone() else {
            return;
        };
        let generation = picker.read(cx).generation;
        let path = path.to_owned();
        let checked_path = path.clone();
        let checking = folders.clone();
        let job = self.extensions.reserve_job();
        let window = self.window;
        picker.read(cx).picker.clone().update(cx, |picker, cx| {
            picker.set_status(PickerStatus::Loading("Checking folder…".into()), cx);
        });
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let mut result = cx
                .background_executor()
                .spawn(async move { checking.check(&checked_path, job).map(Some) })
                .await;
            let current = |cx: &gpui::AsyncApp| {
                model
                    .read_with(cx, |model, cx| {
                        current_picker(model, &picker, generation, cx)
                    })
                    .unwrap_or(false)
            };
            if current(cx) && allow_create && result == Ok(Some(TypedPathState::Missing)) {
                let message = format!(
                    "Muxy will create \"{path}\" on {} and add it as a project.",
                    folders.name
                );
                result = match create_prompt(window, message, cx).await {
                    Ok(true) if current(cx) => {
                        let creating_path = path.clone();
                        cx.background_executor()
                            .spawn(async move {
                                folders
                                    .create(&creating_path, job)
                                    .map(|()| Some(TypedPathState::Directory))
                            })
                            .await
                    }
                    Ok(_) => Ok(None),
                    Err(error) => Err(error),
                };
            }
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                if !current_picker(model, &picker, generation, cx) {
                    return;
                }
                match result {
                    Ok(Some(TypedPathState::Directory)) => {
                        model.open_remote_project_path(server, &path, cx);
                    }
                    Ok(Some(TypedPathState::Missing)) => model.project_picker_error(
                        "Choose an existing folder, or use Create & Add Project.".into(),
                        cx,
                    ),
                    Ok(Some(TypedPathState::NotDirectory)) => model.project_picker_error(
                        "This path is a file. Choose a folder for the project.".into(),
                        cx,
                    ),
                    Ok(None) => picker.update(cx, |picker, cx| picker.sync_picker(cx)),
                    Err(error) => model.project_picker_error(error, cx),
                }
                cx.notify();
            });
        }));
    }

    /// Adds a folder on another computer; its server checks that it exists.
    fn open_remote_project_path(&mut self, server: ServerId, path: &str, cx: &mut Context<Self>) {
        let path = path_service::standardize(path);
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id == server
                    && path_service::standardize(&project.directory.to_string_lossy()) == path
            })
            .map(|project| project.id);
        if self
            .state
            .server_home(server)
            .is_some_and(|home| Some(home.id) == existing)
        {
            let name = self.server_name(server).unwrap_or("the server").to_owned();
            self.project_picker_error(
                format!("Choose a folder inside the home folder on {name}."),
                cx,
            );
        } else if let Some(id) = existing {
            self.join_active_workspace(id, cx);
            self.select_project(id, cx);
            self.dismiss_overlay(cx);
        } else if self
            .add_project_on(server, PathBuf::from(path), cx)
            .is_some()
        {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn confirm_project_path(
        &mut self,
        path: &str,
        create_if_missing: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let path = PathBuf::from(path_service::standardize(path));
        if !path.exists() && create_if_missing {
            let window = self.window;
            let message = format!(
                "Muxy will create \"{}\" and add it as a project.",
                path.display()
            );
            self.close_prompt = Some(cx.spawn(async move |model, cx| {
                let response = create_prompt(window, message, cx).await;
                if response.as_ref().is_ok_and(|confirmed| *confirmed) {
                    let created_path = path.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move { std::fs::create_dir_all(created_path) })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.close_prompt = None;
                        match result {
                            Ok(()) => model.open_project_path(path, cx),
                            Err(error) => model.project_picker_error(
                                format!("Could not create folder: {error}"),
                                cx,
                            ),
                        }
                    });
                } else {
                    let _ = model.update(cx, |model, cx| {
                        model.close_prompt = None;
                        if let Err(error) = response {
                            model.project_picker_error(error, cx);
                        }
                        cx.notify();
                    });
                }
            }));
        } else {
            self.open_project_path(path, cx);
        }
    }

    fn open_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.project_picker_error(
                "Choose an existing folder, or use Create & Add Project.".into(),
                cx,
            );
            return;
        }
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && path_service::standardize(&project.directory.to_string_lossy())
                        == path.to_string_lossy()
            })
            .map(|project| project.id);
        if let Some(id) = existing {
            self.refresh_project_statuses(cx);
            self.join_active_workspace(id, cx);
            self.select_project(id, cx);
            self.dismiss_overlay(cx);
        } else if self.add_project(path, cx).is_some() {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn project_picker_error(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(Overlay::Projects(picker)) = &self.overlay {
            picker.update(cx, |picker, cx| {
                picker.picker.update(cx, |picker, cx| {
                    picker.set_status(PickerStatus::Error(error.into()), cx);
                });
            });
        } else {
            self.fail(error, cx);
        }
    }

    fn choose_project_folder(
        &mut self,
        directory: String,
        search_location: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = folder_prompt(window, directory.into(), search_location, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                match response {
                    Ok(Some(path)) if search_location => model.save_project_search_root(path, cx),
                    Ok(Some(path)) => model.open_project_path(path, cx),
                    Ok(None) => {}
                    Err(error) => model.fail(error, cx),
                }
                cx.notify();
            });
        }));
    }
}

fn current_picker(
    model: &AppModel,
    picker: &Entity<ProjectPicker>,
    generation: usize,
    cx: &App,
) -> bool {
    matches!(&model.overlay, Some(Overlay::Projects(current)) if current == picker)
        && picker.read(cx).generation == generation
}

#[cfg(not(test))]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm(
                window,
                "Create Project Folder?",
                &message,
                "Create & Add",
                None,
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(matches!(
        receiver.recv().await,
        Ok(muxy_ui::dialog::ConfirmationResponse::Confirmed { .. })
    ))
}

#[cfg(test)]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let response = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                "Create Project Folder?",
                Some(&message),
                &["Create & Add", "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(response.await == Ok(0))
}

#[cfg(not(test))]
async fn folder_prompt(
    window: gpui::AnyWindowHandle,
    directory: PathBuf,
    search_location: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, _, _| {
            muxy_ui::dialog::choose_folder(
                if search_location {
                    "Select where Muxy searches for project folders"
                } else {
                    "Select a project folder"
                },
                &directory,
                move |path| {
                    let _ = sender.try_send(path);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
async fn folder_prompt(
    _: gpui::AnyWindowHandle,
    _: PathBuf,
    _: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let response = cx.update(|cx| {
        cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        })
    });
    response
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?
        .map(|paths| paths.and_then(|paths| paths.into_iter().next()))
        .map_err(|error| error.to_string())
}

pub(crate) fn display_path(path: &Path) -> String {
    if let Some(home) = std::env::home_dir()
        && let Ok(relative) = path.strip_prefix(home)
    {
        if relative.as_os_str().is_empty() {
            "~".into()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        path.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn remote_navigation_reloads_and_never_completes_stale_rows(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.bind_keys(muxy_ui::text_input::key_bindings());
            cx.bind_keys(muxy_ui::picker::key_bindings());
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            let folders = RemoteFolders::new(
                "box".into(),
                "/home/remote",
                |path| {
                    Ok(match path {
                        "/home/remote" => vec![DirectoryItem::Directory("code".into())],
                        "/home/remote/code" => vec![DirectoryItem::Directory("api".into())],
                        "/home/remote/code/api" => Vec::new(),
                        _ => return Err(format!("Unexpected listing: {path}")),
                    })
                },
                |_, _| Ok(TypedPathState::Directory),
                |_, _| unreachable!(),
            );
            let picker = ProjectPicker::remote(
                folders,
                vec![],
                Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            );
            picker.focus_handle(cx).focus(window);
            picker
        });
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(125));
            cx.run_until_parked();
        };
        settle(cx);
        view.read_with(cx, |picker, cx| {
            assert!(picker.session.input.is_empty());
            assert!(picker.picker.read(cx).query().is_empty());
            assert_eq!(picker.session.path_state().directory_path, "/home/remote");
        });
        cx.simulate_input("~/");
        settle(cx);
        cx.simulate_keystrokes("tab tab");
        view.read_with(cx, |picker, cx| {
            assert_eq!(picker.session.input, "~/code/");
            assert_eq!(picker.picker.read(cx).query(), "~/code/");
            assert!(picker.session.rows.is_empty());
            assert!(picker.session.ghost_text().is_empty());
        });
        settle(cx);
        view.read_with(cx, |picker, _| {
            assert_eq!(picker.session.ghost_text(), "api/");
        });
        cx.simulate_keystrokes("tab");
        settle(cx);
        view.read_with(cx, |picker, _| {
            assert_eq!(picker.session.input, "~/code/api/");
            assert_eq!(picker.session.rows, [DirectoryItem::Parent]);
        });
        cx.simulate_keystrokes("tab");
        settle(cx);
        view.read_with(cx, |picker, _| assert_eq!(picker.session.input, "~/code/"));
        cx.simulate_keystrokes("alt-backspace");
        settle(cx);
        view.read_with(cx, |picker, _| assert_eq!(picker.session.input, "~/"));
        for input in ["~", "~/", "/home/remote/"] {
            cx.simulate_keystrokes("cmd-a");
            cx.simulate_input(input);
            settle(cx);
            view.read_with(cx, |picker, _| {
                assert_eq!(picker.session.path_state().directory_path, "/home/remote");
                assert_eq!(picker.session.rows[1].name(), "code");
            });
        }
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input("code/a");
        settle(cx);
        cx.simulate_keystrokes("tab");
        settle(cx);
        view.read_with(cx, |picker, _| {
            assert_eq!(picker.session.input, "~/code/api/");
        });
    }

    #[gpui::test]
    fn path_rows_complete_and_navigate_with_the_legacy_keys(cx: &mut TestAppContext) {
        let root = std::env::temp_dir().join(format!(
            "muxy-picker-ui-{}",
            muxy_app_core::ProjectId::new()
        ));
        std::fs::create_dir_all(root.join("Alpha/Child")).expect("mkdir");
        cx.update(|cx| {
            cx.bind_keys(muxy_ui::text_input::key_bindings());
            cx.bind_keys(muxy_ui::picker::key_bindings());
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            let picker = ProjectPicker::new(
                SearchService::new(),
                &root.to_string_lossy(),
                Vec::new(),
                Theme::from_scheme(&muxy_ui::theme::ColorScheme::default()),
                Metrics::new(1.0),
                cx,
            );
            picker.focus_handle(cx).focus(window);
            picker
        });
        cx.run_until_parked();
        view.read_with(cx, |picker, _| {
            assert!(picker.session.input.is_empty());
            assert!(picker.session.search_results.is_empty());
        });
        cx.simulate_input(&format!("{}/", root.display()));
        cx.executor().advance_clock(Duration::from_millis(125));
        cx.run_until_parked();
        view.read_with(cx, |picker, _| {
            assert!(picker.session.rows[0].is_parent());
            assert_eq!(picker.session.ghost_text(), "Alpha/");
        });
        let panel = cx
            .debug_bounds("project-picker")
            .expect("compact project picker");
        assert_eq!(panel.size.width, gpui::px(480.0));
        let row = cx.debug_bounds("picker-row-path-1").expect("directory row");
        assert_eq!(row.size.height, gpui::px(32.0));
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |picker, _| picker.session.input.clone()),
            format!("{}/Alpha/", root.display())
        );
        let back = cx.debug_bounds("picker-back").expect("back button");
        cx.simulate_click(back.center(), gpui::Modifiers::default());
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |picker, _| picker.session.input.clone()),
            format!("{}/", root.display())
        );
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        cx.simulate_keystrokes("alt-backspace");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |picker, _| picker.session.input.clone()),
            format!("{}/", root.display())
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |picker, _| picker.session.input.clone()),
            format!("{}/Alpha/", root.display())
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
