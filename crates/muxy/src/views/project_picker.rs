use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyBinding, Render, Subscription,
};
use muxy_api::picker::path_service::{self, DirectoryItem};
use muxy_api::picker::search::SearchService;
use muxy_api::picker::session::{InputMode, LoadState, Session};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverAction, CommandPopoverConfig, CommandPopoverDensity,
    CommandPopoverEvent, CommandPopoverItem, CommandPopoverLeading, CommandPopoverPresentation,
    CommandPopoverRow, CommandPopoverStatus, CommandPopoverTab,
};
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};
use std::time::Duration;

const RELOAD_DELAY: Duration = Duration::from_millis(100);
const LOADING_MESSAGE_DELAY: Duration = Duration::from_millis(500);
const SEARCH_RESULT_LIMIT: usize = 50;
const DIRECTORY_CACHE_LIMIT: usize = 64;

pub fn key_bindings() -> Vec<KeyBinding> {
    Vec::new()
}

pub enum PickerEvent {
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

pub struct ProjectPicker {
    session: Session,
    search: SearchService,
    picker: Entity<CommandPopover>,
    generation: usize,
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
    pub fn new(
        search: SearchService,
        search_root: String,
        project_paths: Vec<String>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            CommandPopover::new(
                CommandPopoverConfig {
                    id: "project-picker".into(),
                    presentation: CommandPopoverPresentation::Modal,
                    density: CommandPopoverDensity::Comfortable,
                    tabs: vec![CommandPopoverTab::new("projects", "Projects")],
                    placeholder: "Search folders or enter a path…".into(),
                    footer_actions: vec![
                        CommandPopoverAction::new("confirm-path", "Open")
                            .icon(CommandPopoverLeading::Icon(Icon::Plus)),
                        CommandPopoverAction::new("back", "Back"),
                        CommandPopoverAction::new("finder", "Choose with Finder"),
                        CommandPopoverAction::new("location", "Edit Search Location"),
                    ],
                    footer_hints: Vec::new(),
                    width: Some(640.0),
                    height: Some(460.0),
                    max_height: None,
                    completion_on_tab: true,
                    confirm_on_click: true,
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
                    CommandPopoverEvent::QueryChanged { query, .. } => {
                        project_picker.session.set_input(query.as_ref());
                        project_picker.reload(cx);
                    }
                    CommandPopoverEvent::Confirmed(selection) => {
                        project_picker.activate(selection.id.as_ref(), cx)
                    }
                    CommandPopoverEvent::SelectionChanged(selection) => {
                        project_picker.select(selection.id.as_ref())
                    }
                    CommandPopoverEvent::SecondaryConfirmed(_)
                    | CommandPopoverEvent::Submitted { secondary: true } => {
                        project_picker.confirm(true, cx)
                    }
                    CommandPopoverEvent::Submitted { secondary: false } => {
                        project_picker.confirm(false, cx)
                    }
                    CommandPopoverEvent::CompletionRequested => {
                        project_picker.complete_highlighted(cx)
                    }
                    CommandPopoverEvent::NavigateBackRequested => project_picker.go_back(cx),
                    CommandPopoverEvent::FooterAction(action) => match action.as_ref() {
                        "confirm-path" => project_picker.confirm(true, cx),
                        "back" => project_picker.go_back(cx),
                        "finder" => project_picker.choose_finder(cx),
                        "location" => project_picker.edit_search_location(cx),
                        _ => {}
                    },
                    CommandPopoverEvent::Dismissed => cx.emit(PickerEvent::Dismiss),
                    _ => {}
                },
            );
        let mut project_picker = Self {
            session: Session::new(&search_root, project_paths),
            search,
            picker,
            generation: 0,
            directory_cache: std::collections::HashMap::new(),
            directory_cache_order: Vec::new(),
            _subscriptions: vec![subscription],
        };
        let warm_root = project_picker.session.search_root_path.clone();
        let warm_search = project_picker.search.clone();
        cx.background_executor()
            .spawn(async move { warm_search.prepare(&warm_root) })
            .detach();
        project_picker.reload(cx);
        project_picker
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let mode = self.session.input_mode();
        let query = self.session.search_query().to_owned();
        let root = self.session.search_root_path.clone();
        let paths = self.session.project_paths.clone();
        let path_state = self.session.path_state();
        let search = self.search.clone();

        if mode == InputMode::FolderSearch && query.is_empty() {
            self.session.apply_search_snapshot(Default::default());
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
                    let snapshot =
                        cx.background_executor()
                            .spawn(async move {
                                search.search(&query, &root, &paths, SEARCH_RESULT_LIMIT)
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
            InputMode::Path => {
                let directory = path_state.directory_path.clone();
                cx.spawn(async move |picker, cx| {
                    cx.background_executor().timer(RELOAD_DELAY).await;
                    if picker.read_with(cx, |picker, _| picker.generation).ok() != Some(generation)
                    {
                        return;
                    }
                    let listed = directory.clone();
                    let contents = cx
                        .background_executor()
                        .spawn(async move { path_service::directory_contents(&listed) })
                        .await;
                    let _ = picker.update(cx, |picker, cx| {
                        if picker.generation != generation {
                            return;
                        }
                        let path_state = picker.session.path_state();
                        match contents {
                            Ok(items) => {
                                picker.cache_items(&directory, items.clone());
                                picker.session.apply_directory_snapshot(
                                    path_state.directory_items(items),
                                    false,
                                );
                            }
                            Err(_) => picker.session.apply_directory_snapshot(
                                path_state.directory_read_failure_items(),
                                true,
                            ),
                        }
                        picker.sync_picker(cx);
                    });
                })
                .detach();
            }
        }
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
                            CommandPopoverRow::new(format!("search-{index}"), result.name.clone());
                        row.subtitle = Some(result.display_path.clone().into());
                        row.leading = Some(CommandPopoverLeading::Icon(Icon::Folder));
                        CommandPopoverItem::Row(row)
                    })
                    .collect(),
                InputMode::Path => self
                    .session
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let mut row = CommandPopoverRow::new(
                            format!("path-{index}"),
                            if item.is_parent() {
                                "Parent Directory".to_owned()
                            } else {
                                item.name().to_owned()
                            },
                        );
                        row.subtitle = item.is_symlink().then(|| "Symbolic link".into());
                        row.leading = Some(CommandPopoverLeading::Icon(if item.is_parent() {
                            Icon::ChevronLeft
                        } else {
                            Icon::Folder
                        }));
                        CommandPopoverItem::Row(row)
                    })
                    .collect(),
            }
        };
        if !loading && self.session.shows_unavailable_state() && !items.is_empty() {
            let label = if matches!(self.session.load_state, LoadState::Failed) {
                "Could not read this folder"
            } else if self.session.input_mode() == InputMode::Path {
                "Folder is empty"
            } else {
                "No matching folders"
            };
            let mut row = CommandPopoverRow::new("path-unavailable", label);
            row.disabled = true;
            items.push(CommandPopoverItem::Row(row));
        }
        let status = match self.session.load_state {
            LoadState::Loading {
                shows_message: true,
            } => CommandPopoverStatus::Loading("Loading folders…".into()),
            LoadState::Loading {
                shows_message: false,
            } => CommandPopoverStatus::Loading("".into()),
            LoadState::Failed if !items.is_empty() => CommandPopoverStatus::Ready,
            LoadState::Failed => CommandPopoverStatus::Error("Could not read this folder".into()),
            LoadState::Loaded if self.session.shows_unavailable_state() && !items.is_empty() => {
                CommandPopoverStatus::Ready
            }
            LoadState::Loaded if self.session.shows_unavailable_state() => {
                CommandPopoverStatus::Empty("No matching folders".into())
            }
            LoadState::Loaded => CommandPopoverStatus::Ready,
        };
        let ghost = self.session.ghost_text();
        let actions = vec![
            CommandPopoverAction::new("confirm-path", self.session.top_right_action_title())
                .icon(CommandPopoverLeading::Icon(Icon::Plus))
                .disabled(self.session.confirmation_path().is_none()),
            CommandPopoverAction::new("back", "Back"),
            CommandPopoverAction::new("finder", "Choose with Finder"),
            CommandPopoverAction::new("location", "Edit Search Location"),
        ];
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
            picker
                .input()
                .update(cx, |input, cx| input.set_ghost(ghost, cx));
        });
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

    fn select(&mut self, id: &str) {
        let Some(index) = id
            .split_once('-')
            .and_then(|(_, index)| index.parse::<usize>().ok())
        else {
            return;
        };
        self.session.select_row(index);
    }

    fn apply_input(&mut self, cx: &mut Context<Self>) {
        let text = self.session.input.clone();
        self.picker
            .read(cx)
            .input()
            .update(cx, |input, cx| input.set_text(text, cx));
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
            && self.session.typed_path_state() == path_service::TypedPathState::Missing;
        cx.emit(PickerEvent::Confirm {
            path,
            create_if_missing,
        });
    }

    fn choose_finder(&self, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::ChooseFinder {
            directory: self.session.path_state().directory_path,
        });
    }

    fn edit_search_location(&self, cx: &mut Context<Self>) {
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
        while self.directory_cache_order.len() > DIRECTORY_CACHE_LIMIT {
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
