mod targets;

use super::overlays::Overlay;
use crate::model::{AppModel, PendingRemote, RemoteIntent};
use crate::picker::path_service::{self, DirectoryItem, TypedPathState};
use crate::picker::remote::RemoteFolders;
use crate::picker::search::{SearchService, Snapshot};
use crate::picker::session::{InputMode, LoadState, Session};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription,
};
use muxy_app_core::{ProjectStatus, ServerId, settings::Keymap};
use muxy_core::shortcuts::{ShortcutId, ShortcutSettings};
use muxy_ui::command_palette::Registry;
use muxy_ui::icon::Icon;
use muxy_ui::l10n::tr_key;
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent as ListEvent, PickerFooterStyle, PickerItem,
    PickerLeading, PickerRow, PickerStatus,
};
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use targets::Target;

const RELOAD_DELAY: Duration = Duration::from_millis(100);
const LOADING_MESSAGE_DELAY: Duration = Duration::from_millis(500);
const SEARCH_RESULT_LIMIT: usize = 50;
const DIRECTORY_CACHE_LIMIT: usize = 64;
const DIRECTORY_CACHE_ITEMS: usize = 50_000;

pub(crate) enum PickerEvent {
    ConfirmTarget {
        target: Target,
        home_terminal: bool,
    },
    Confirm {
        path: String,
        create_if_missing: bool,
        home_terminal: bool,
    },
    Dismiss,
}

pub(crate) struct ProjectPicker {
    session: Session,
    search: SearchService,
    targets: Registry<Target>,
    shortcuts: [Option<String>; 2],
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
            tr!("Search folders or enter a path…").to_string(),
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
        let placeholder = tr!("Enter a path on %@…", &folders.name).to_string();
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
                    confirm_on_click: false,
                    footer_style: PickerFooterStyle::Compact,
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
                    ListEvent::Confirmed(_) | ListEvent::Submitted { secondary: false } => {
                        project_picker.confirm(false, cx);
                    }
                    ListEvent::RowClicked { row, .. } => {
                        project_picker.activate(row.as_ref(), cx);
                    }
                    ListEvent::SelectionChanged(selection) => {
                        project_picker.select(selection.id.as_ref(), cx);
                    }
                    ListEvent::SecondaryConfirmed(_) | ListEvent::Submitted { secondary: true } => {
                        project_picker.confirm(true, cx);
                    }
                    ListEvent::CompletionRequested => {
                        project_picker.complete_highlighted(cx);
                    }
                    ListEvent::NavigateBackRequested => project_picker.go_back(cx),
                    ListEvent::FooterAction(action) => match action.as_ref() {
                        "open-terminal" => project_picker.confirm(true, cx),
                        "confirm-path" => project_picker.confirm(false, cx),
                        _ => {}
                    },
                    ListEvent::Dismissed => cx.emit(PickerEvent::Dismiss),
                    _ => {}
                },
            );
        let mut project_picker = Self {
            session,
            search,
            targets: Registry::default(),
            shortcuts: [None, None],
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

    fn set_shortcuts(&mut self, keymap: &Keymap) {
        self.shortcuts = [
            ShortcutId::PopoverConfirm,
            ShortcutId::PopoverSecondaryConfirm,
        ]
        .map(|id| confirmation_shortcut(keymap, id));
    }

    fn sync_targets(&mut self, cx: &mut Context<Self>) -> bool {
        if self.session.input_mode() != InputMode::FolderSearch {
            return false;
        }
        let items = self.targets.items(self.session.search_query());
        let has_project = items.iter().any(|item| {
            matches!(item, PickerItem::Row(row) if matches!(self.targets.action(&row.id), Some(Target::Project(_))))
        });
        if !has_project {
            return false;
        }
        self.session.apply_search_snapshot(Snapshot::default());
        self.sync_picker(cx);
        true
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.listing_error = None;
        self.generation = self.generation.wrapping_add(1);
        if self.sync_targets(cx) {
            return;
        }
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
                            let mut snapshot = search.search(
                                &query,
                                &root,
                                &paths,
                                SEARCH_RESULT_LIMIT,
                                &cancelled,
                            );
                            snapshot
                                .results
                                .retain(|result| Path::new(&result.path).is_dir());
                            snapshot
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
                                tr!("Parent Directory").to_string()
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
            _ => tr!("Could not read this folder").to_string(),
        };
        if !loading && self.session.shows_unavailable_state() && !items.is_empty() {
            let label = if matches!(self.session.load_state, LoadState::Failed) {
                unreadable.clone()
            } else if self.session.input_mode() == InputMode::Path {
                if self.session.path_state().leaf_filter.is_empty() {
                    tr!("Folder is empty").to_string()
                } else {
                    tr!("No matching folders").to_string()
                }
            } else {
                tr!("No matching folders").to_string()
            };
            let mut row = PickerRow::new("path-unavailable", label);
            row.disabled = true;
            items.push(PickerItem::Row(row));
        }
        let targets = if self.session.input_mode() == InputMode::FolderSearch {
            self.targets.items(self.session.search_query())
        } else {
            Vec::new()
        };
        let has_targets = !targets.is_empty();
        items.splice(..0, targets);
        let status = match self.session.load_state {
            _ if has_targets => PickerStatus::Ready,
            LoadState::Loading {
                shows_message: true,
            } => PickerStatus::Loading(tr!("Loading folders…")),
            LoadState::Loading {
                shows_message: false,
            } => PickerStatus::Loading("".into()),
            LoadState::Failed if !items.is_empty() => PickerStatus::Ready,
            LoadState::Failed => PickerStatus::Error(unreadable.clone().into()),
            LoadState::Loaded if self.session.shows_unavailable_state() && !items.is_empty() => {
                PickerStatus::Ready
            }
            LoadState::Loaded if self.session.shows_unavailable_state() => {
                PickerStatus::Empty(tr!("No matching folders"))
            }
            LoadState::Loaded => PickerStatus::Ready,
        };
        let ghost = self.session.ghost_text();
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            self.sync_selection(picker, cx);
            picker.set_can_navigate_back(self.session.input_mode() == InputMode::Path, cx);
            picker
                .input()
                .update(cx, |input, cx| input.set_ghost(ghost, cx));
        });
    }

    fn sync_selection(&self, picker: &mut Picker, cx: &mut Context<Picker>) {
        if picker
            .selected_row_id()
            .and_then(|id| self.targets.action(id))
            .is_none()
            && let Some(index) = self.session.highlighted_index
        {
            let prefix = match self.session.input_mode() {
                InputMode::FolderSearch => "search",
                InputMode::Path => "path",
            };
            let _ = picker.select_row(&format!("{prefix}-{index}"), cx);
        }
        let target = picker
            .selected_row_id()
            .and_then(|id| self.targets.action(id))
            .copied();
        picker.set_footer_actions(
            self.footer_actions(
                target,
                target.is_none() && self.session.confirmation_path().is_none(),
            ),
            cx,
        );
    }

    fn footer_actions(&self, target: Option<Target>, disabled: bool) -> Vec<PickerAction> {
        let title = match target {
            Some(Target::Project(_)) => tr_key!("Open Project"),
            Some(Target::Server(_)) => tr_key!("Browse Folders…"),
            None if self
                .session
                .confirmation_path()
                .is_some_and(|path| self.missing_remote_folder(&path).is_some()) =>
            {
                tr_key!("Create & Add Project")
            }
            None => self.session.top_right_action_title(),
        };
        [
            PickerAction::new("confirm-path", muxy_ui::l10n::translate(title)),
            PickerAction::new("open-terminal", tr!("Terminal in Home")),
        ]
        .into_iter()
        .zip(&self.shortcuts)
        .map(|(mut action, shortcut)| {
            action.shortcut = shortcut.clone().map(Into::into);
            action.disabled(disabled)
        })
        .collect()
    }

    fn activate(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(target) = self.targets.action(id) {
            cx.emit(PickerEvent::ConfirmTarget {
                target: *target,
                home_terminal: false,
            });
            return;
        }
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
        if let Some(target) = self.targets.action(id).copied() {
            let actions = self.footer_actions(Some(target), false);
            self.picker
                .update(cx, |picker, cx| picker.set_footer_actions(actions, cx));
            return;
        }
        let Some(index) = id
            .split_once('-')
            .and_then(|(_, index)| index.parse::<usize>().ok())
        else {
            return;
        };
        self.session.select_row(index);
        let ghost = self.session.ghost_text();
        let actions = self.footer_actions(None, self.session.confirmation_path().is_none());
        self.picker.update(cx, |picker, cx| {
            picker.set_footer_actions(actions, cx);
            picker
                .input()
                .update(cx, |input, cx| input.set_ghost(ghost, cx));
        });
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
        if self
            .picker
            .read(cx)
            .selected_row_id()
            .and_then(|id| self.targets.action(id))
            .is_some()
        {
            return;
        }
        self.session.complete_highlighted();
        self.apply_input(cx);
    }

    fn confirm(&self, home_terminal: bool, cx: &mut Context<Self>) {
        if let Some(target) = self
            .picker
            .read(cx)
            .selected_row_id()
            .and_then(|id| self.targets.action(id))
        {
            cx.emit(PickerEvent::ConfirmTarget {
                target: *target,
                home_terminal,
            });
            return;
        }
        let Some(path) = self.session.confirmation_path() else {
            return;
        };
        let create_if_missing = !home_terminal
            && self.session.input_mode() == InputMode::Path
            && (self.remote.is_some()
                || self.session.typed_path_state() == TypedPathState::Missing);
        cx.emit(PickerEvent::Confirm {
            path,
            create_if_missing,
            home_terminal,
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
        Some(tr!("There is no folder named %@ in %@", name, &parent).to_string())
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
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Projects(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        let root = self
            .settings
            .projects
            .search_root
            .as_ref()
            .unwrap_or(&self.state.home().directory)
            .to_string_lossy()
            .into_owned();
        let paths = self.project_paths(ServerId::local());
        let targets = targets::registry(self);
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
        picker.update(cx, |picker, cx| {
            picker.targets = targets;
            picker.picker.update(cx, |picker, cx| {
                picker.set_placeholder(tr!("Search projects, folders or enter a path…"), cx);
            });
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
            self.pending_remote = Some(PendingRemote::new(server, RemoteIntent::AddProject));
            if ready && self.extensions.client(server).is_none() {
                self.extension_client(server, cx);
            } else {
                self.connect_remote_server(server, cx);
            }
            cx.notify();
            return;
        };
        self.pending_remote = None;
        let folders = RemoteFolders::through(client, name, project, &home);
        let paths = self.project_paths(server);
        let picker = cx
            .new(|cx| ProjectPicker::remote(folders, paths, self.theme.clone(), self.metrics, cx));
        self.show_project_picker(server, picker, cx);
    }

    /// The folders of the projects the sidebar lists on `server`, besides
    /// another computer's Home, which is where its picker starts.
    fn project_paths(&self, server: ServerId) -> Vec<String> {
        self.state
            .projects()
            .iter()
            .filter(|project| {
                project.server_id == server
                    && self.project_listed(project)
                    && !project.is_remote_home()
            })
            .map(|project| project.directory.to_string_lossy().into_owned())
            .collect()
    }

    pub(crate) fn show_project_picker(
        &mut self,
        server: ServerId,
        picker: Entity<ProjectPicker>,
        cx: &mut Context<Self>,
    ) {
        picker.update(cx, |picker, cx| {
            picker.set_shortcuts(&self.settings.keymap);
            picker.reload(cx);
        });
        self.focus_later(picker.focus_handle(cx), cx);
        self.overlay_subscription =
            Some(
                cx.subscribe(&picker, move |model, picker, event, cx| match event {
                    PickerEvent::ConfirmTarget {
                        target,
                        home_terminal,
                    } => {
                        model.confirm_project_target(picker, *target, *home_terminal, cx);
                    }
                    PickerEvent::Confirm {
                        path,
                        create_if_missing,
                        home_terminal,
                    } if !server.is_local() => {
                        model.confirm_remote_project_path(
                            server,
                            picker,
                            path,
                            *create_if_missing,
                            *home_terminal,
                            cx,
                        );
                    }
                    PickerEvent::Confirm {
                        path,
                        home_terminal: true,
                        ..
                    } => {
                        model.open_home_terminal(
                            ServerId::local(),
                            Path::new(&path_service::standardize(path)),
                            cx,
                        );
                    }
                    PickerEvent::Confirm {
                        path,
                        create_if_missing,
                        ..
                    } => {
                        model.confirm_project_path(path, *create_if_missing, cx);
                    }
                    PickerEvent::Dismiss => model.dismiss_overlay(cx),
                }),
            );
        self.overlay = Some(Overlay::Projects(picker));
        cx.notify();
    }

    fn confirm_project_target(
        &mut self,
        picker: Entity<ProjectPicker>,
        target: Target,
        home_terminal: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        match target {
            Target::Project(id) if home_terminal => {
                let Some(project) = self.state.project(id) else {
                    return;
                };
                if project.status() == ProjectStatus::Missing {
                    self.project_picker_error(
                        tr!("Choose an existing folder for the terminal.").to_string(),
                        cx,
                    );
                    return;
                }
                let server = project.server_id;
                let path = project.directory.clone();
                if server.is_local() {
                    self.open_home_terminal(server, &path, cx);
                    return;
                }
                let files = match self.remote_files(server, cx) {
                    Ok(files) => files,
                    Err(error) => {
                        self.project_picker_error(error, cx);
                        return;
                    }
                };
                let generation = picker.read(cx).generation;
                let connection = self.generation(server);
                picker.read(cx).picker.clone().update(cx, |picker, cx| {
                    picker.set_status(PickerStatus::Loading(tr!("Checking folder…")), cx);
                });
                self.close_prompt = Some(cx.spawn(async move |model, cx| {
                    let available = cx
                        .background_executor()
                        .spawn(async move { files.folder_available(id, false) })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        if !current_picker(model, &picker, generation, cx) {
                            return;
                        }
                        let current = model.generation(server) == connection
                            && model.server_ready(server)
                            && model.state.project(id).is_some_and(|project| {
                                project.server_id == server && project.directory == path
                            });
                        match available {
                            Some(true) if current => model.open_home_terminal(server, &path, cx),
                            Some(false) => model.project_picker_error(
                                tr!("Choose an existing folder for the terminal.").to_string(),
                                cx,
                            ),
                            _ => model.project_picker_error(
                                tr!("Could not read this folder").to_string(),
                                cx,
                            ),
                        }
                    });
                }));
            }
            Target::Project(id) => self.show_project(id, cx),
            Target::Server(server) => {
                self.dismiss_overlay(cx);
                if home_terminal {
                    self.new_remote_home_tab(server, cx);
                } else {
                    self.open_remote_project_picker(server, cx);
                }
            }
        }
    }

    fn confirm_remote_project_path(
        &mut self,
        server: ServerId,
        picker: Entity<ProjectPicker>,
        path: &str,
        allow_create: bool,
        home_terminal: bool,
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
            picker.set_status(PickerStatus::Loading(tr!("Checking folder…")), cx);
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
            if current(cx)
                && allow_create
                && !home_terminal
                && result == Ok(Some(TypedPathState::Missing))
            {
                let message = tr!(
                    "Muxy will create \"%@\" on %@ and add it as a project.",
                    &path,
                    &folders.name
                )
                .to_string();
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
                model.finish_confirmation(cx);
                if !current_picker(model, &picker, generation, cx) {
                    return;
                }
                match result {
                    Ok(Some(TypedPathState::Directory)) if home_terminal => {
                        model.open_home_terminal(
                            server,
                            Path::new(&path_service::standardize(&path)),
                            cx,
                        );
                    }
                    Ok(Some(TypedPathState::Directory)) => {
                        model.open_remote_project_path(server, &path, cx);
                    }
                    Ok(Some(TypedPathState::Missing | TypedPathState::NotDirectory))
                        if home_terminal =>
                    {
                        model.project_picker_error(
                            tr!("Choose an existing folder for the terminal.").to_string(),
                            cx,
                        );
                    }
                    Ok(Some(TypedPathState::Missing)) => model.project_picker_error(
                        tr!("Choose an existing folder, or use Create & Add Project.").to_string(),
                        cx,
                    ),
                    Ok(Some(TypedPathState::NotDirectory)) => model.project_picker_error(
                        tr!("This path is a file. Choose a folder for the project.").to_string(),
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
            let message = match self.server_name(server) {
                Some(name) => tr!("Choose a folder inside the home folder on %@.", name),
                None => tr!("Choose a folder inside the home folder on the server."),
            };
            self.project_picker_error(message.to_string(), cx);
        } else if let Some(id) = existing {
            self.join_active_workspace(id, cx);
            self.select_project(id, cx);
            self.dismiss_overlay(cx);
        } else if self
            .add_project_on(server, PathBuf::from(path), true, cx)
            .is_some()
        {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn open_home_terminal(&mut self, server: ServerId, path: &Path, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if server.is_local() && !path.is_dir() {
            self.project_picker_error(
                tr!("Choose an existing folder for the terminal.").to_string(),
                cx,
            );
            return;
        }
        if self.new_home_tab_at(server, path, cx) {
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
            let message = tr!(
                "Muxy will create \"%@\" and add it as a project.",
                path.display().to_string()
            )
            .to_string();
            self.close_prompt = Some(cx.spawn(async move |model, cx| {
                let response = create_prompt(window, message, cx).await;
                if response.as_ref().is_ok_and(|confirmed| *confirmed) {
                    let created_path = path.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move { std::fs::create_dir_all(created_path) })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        match result {
                            Ok(()) => model.open_project_path(path, true, cx),
                            Err(error) => model.project_picker_error(
                                tr!("Could not create folder: %@", error.to_string()).to_string(),
                                cx,
                            ),
                        }
                    });
                } else {
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        if let Err(error) = response {
                            model.project_picker_error(error, cx);
                        }
                        cx.notify();
                    });
                }
            }));
        } else {
            self.open_project_path(path, true, cx);
        }
    }

    pub(crate) fn open_folders_from(
        folders: async_channel::Receiver<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(urls) = folders.recv().await {
                if this
                    .update(cx, |model, cx| {
                        if urls.is_empty() {
                            model.raise_window(cx);
                        }
                        model
                            .pending_folders
                            .extend(urls.iter().filter_map(|url| folder_from_url(url)));
                        if model.can_open_local_folders() {
                            model.resume_folders(ServerId::local(), cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn resume_folders(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() {
            for folder in std::mem::take(&mut self.pending_folders) {
                self.open_folder(folder, cx);
            }
        }
    }

    fn open_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let canonical = folder.canonicalize().ok();
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && canonical.is_some()
                    && project.directory.canonicalize().ok() == canonical
            })
            .map(|project| project.id);
        match existing {
            Some(id) => self.show_project(id, cx),
            None => self.open_project_path(folder, false, cx),
        }
    }

    fn open_project_path(&mut self, path: PathBuf, initial_terminal: bool, cx: &mut Context<Self>) {
        if !path.is_dir() {
            let message = tr!("Choose an existing folder, or use Create & Add Project.");
            self.project_picker_error(message.to_string(), cx);
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
            self.show_project(id, cx);
        } else if self
            .add_project_on(ServerId::local(), path, initial_terminal, cx)
            .is_some()
        {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn show_project(&mut self, id: muxy_app_core::ProjectId, cx: &mut Context<Self>) {
        self.refresh_project_statuses(cx);
        self.join_active_workspace(id, cx);
        self.select_project(id, cx);
        self.dismiss_overlay(cx);
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
}

fn confirmation_shortcut(keymap: &Keymap, id: ShortcutId) -> Option<String> {
    let keys = keymap.keys(id.name(), Some("Picker"));
    let preferred = if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-shift-enter"
    };
    keys.iter()
        .find(|key| key.as_str() == preferred)
        .or_else(|| keys.first())
        .map(|key| gpui::Keystroke::parse(key).map_or_else(|_| key.clone(), |key| key.to_string()))
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

fn folder_from_url(url: &str) -> Option<PathBuf> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    let path = url.to_file_path().ok()?;
    path.is_dir().then(|| path.components().collect())
}

#[cfg(not(test))]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            muxy_ui::dialog::confirm(
                window,
                cx,
                &tr!("Create Project Folder?"),
                &message,
                &tr!("Create & Add"),
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

    #[test]
    fn confirmation_shortcut_hints_follow_active_aliases_and_overrides() {
        let keymap = Keymap::default();
        let secondary = ShortcutId::PopoverSecondaryConfirm;
        let preferred = if cfg!(target_os = "macos") {
            "cmd-enter"
        } else {
            "ctrl-shift-enter"
        };
        let displayed = |key| Some(gpui::Keystroke::parse(key).expect("keystroke").to_string());
        assert_eq!(
            confirmation_shortcut(&keymap, secondary),
            displayed(preferred)
        );
        let remapped = keymap
            .with_binding(
                secondary.name(),
                Some("ctrl-alt-enter".parse().expect("chord")),
            )
            .expect("remapped secondary confirmation");
        assert_eq!(
            confirmation_shortcut(&remapped, secondary),
            displayed("ctrl-alt-enter")
        );
        let claimed = keymap
            .with_binding(
                ShortcutId::PopoverConfirm.name(),
                Some(preferred.parse().expect("chord")),
            )
            .expect("reassigned alias");
        assert_eq!(
            confirmation_shortcut(&claimed, secondary),
            displayed("alt-enter")
        );
        assert_eq!(
            confirmation_shortcut(&claimed, ShortcutId::PopoverConfirm),
            displayed(preferred)
        );
    }
}
