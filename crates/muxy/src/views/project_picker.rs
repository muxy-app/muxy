use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, px,
};
use muxy_api::picker::path_service::{self, DirectoryItem};
use muxy_api::picker::search::SearchService;
use muxy_api::picker::session::{InputMode, Session};
use muxy_api::picker::shortcuts::{self, KeycapPart};
use muxy_ui::components::SymbolGlyph;
use muxy_ui::text_input::{self, InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};
use std::time::Duration;

const MONOSPACE_FONT: &str = "Menlo";
const RELOAD_DELAY: Duration = Duration::from_millis(100);
const LOADING_MESSAGE_DELAY: Duration = Duration::from_millis(500);
const SEARCH_RESULT_LIMIT: usize = 50;
const DIRECTORY_CACHE_LIMIT: usize = 64;
const PANEL_WIDTH: f32 = 640.0;
const PANEL_HEIGHT: f32 = 460.0;
const PANEL_TOP: f32 = 60.0;
const KEY_CONTEXT: &str = "ProjectPicker";

actions!(
    project_picker,
    [
        MoveUp,
        MoveDown,
        OpenHighlighted,
        ConfirmTypedPath,
        GoBack,
        CompleteHighlighted,
        Dismiss,
    ]
);

pub fn key_bindings() -> Vec<KeyBinding> {
    let context = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("enter", OpenHighlighted, context),
        KeyBinding::new("cmd-enter", ConfirmTypedPath, context),
        KeyBinding::new("alt-backspace", GoBack, context),
        KeyBinding::new("tab", CompleteHighlighted, context),
        KeyBinding::new("escape", Dismiss, context),
    ]
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
    input: Entity<TextInput>,
    focus_handle: FocusHandle,
    theme: Theme,
    metrics: Metrics,
    generation: usize,
    action_menu_open: bool,
    focused: bool,
    scroll: gpui::ScrollHandle,
    directory_cache: std::collections::HashMap<String, Vec<DirectoryItem>>,
    directory_cache_order: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PickerEvent> for ProjectPicker {}

impl Focusable for ProjectPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
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
        let style = InputStyle::field(&theme, &metrics);
        let input = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_font_family(MONOSPACE_FONT)
                .with_placeholder("Search folders or enter a path…")
        });
        let subscription = cx.subscribe(
            &input,
            |picker: &mut Self, input: Entity<TextInput>, event, cx| {
                if matches!(event, InputEvent::Changed) {
                    let text = input.read(cx).text().to_owned();
                    picker.session.set_input(text);
                    picker.reload(cx);
                }
            },
        );

        let mut picker = Self {
            session: Session::new(&search_root, project_paths),
            search,
            input,
            focus_handle: cx.focus_handle(),
            theme,
            metrics,
            generation: 0,
            action_menu_open: false,
            focused: false,
            scroll: gpui::ScrollHandle::default(),
            directory_cache: std::collections::HashMap::new(),
            directory_cache_order: Vec::new(),
            _subscriptions: vec![subscription],
        };
        let warm_root = picker.session.search_root_path.clone();
        let warm_search = picker.search.clone();
        cx.background_executor()
            .spawn(async move { warm_search.prepare(&warm_root) })
            .detach();
        picker.reload(cx);
        picker
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let mode = self.session.input_mode();
        let query = self.session.search_query().to_owned();
        let root = self.session.search_root_path.clone();
        let paths = self.session.project_paths.clone();
        let path_state = self.session.path_state();
        let search = self.search.clone();

        if mode == InputMode::FolderSearch && query.is_empty() {
            self.session.apply_search_snapshot(Default::default());
            self.sync_ghost(cx);
            return;
        }

        if mode == InputMode::Path
            && let Some(items) = self
                .directory_cache
                .get(&path_state.directory_path)
                .cloned()
        {
            let rows = path_state.directory_items(items);
            self.session.apply_directory_snapshot(rows, false);
            self.sync_ghost(cx);
            return;
        }

        cx.spawn(async move |picker, cx| {
            cx.background_executor().timer(LOADING_MESSAGE_DELAY).await;
            let _ = picker.update(cx, |picker, cx| {
                if picker.generation == generation {
                    picker.session.show_loading_message();
                    cx.notify();
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
                        if picker.generation != generation {
                            return;
                        }
                        picker.session.apply_search_snapshot(snapshot);
                        picker.sync_ghost(cx);
                        cx.notify();
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
                                let rows = path_state.directory_items(items);
                                picker.session.apply_directory_snapshot(rows, false);
                            }
                            Err(_) => {
                                let rows = path_state.directory_read_failure_items();
                                picker.session.apply_directory_snapshot(rows, true);
                            }
                        }
                        picker.sync_ghost(cx);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
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

    fn sync_ghost(&mut self, cx: &mut Context<Self>) {
        let ghost = self.session.ghost_text();
        self.input
            .update(cx, |input, cx| input.set_ghost(ghost, cx));
    }

    fn apply_input(&mut self, cx: &mut Context<Self>) {
        let text = self.session.input.clone();
        self.input
            .update(cx, |input, cx| input.set_text(text.clone(), cx));
        self.reload(cx);
        cx.notify();
    }

    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.session.move_highlight(-1);
        self.reveal_highlighted();
        self.sync_ghost(cx);
        cx.notify();
    }

    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.session.move_highlight(1);
        self.reveal_highlighted();
        self.sync_ghost(cx);
        cx.notify();
    }

    fn reveal_highlighted(&self) {
        if let Some(index) = self.session.highlighted_index {
            self.scroll.scroll_to_item(index);
        }
    }

    fn open_highlighted(&mut self, _: &OpenHighlighted, _: &mut Window, cx: &mut Context<Self>) {
        match self.session.input_mode() {
            InputMode::FolderSearch => self.confirm(false, cx),
            InputMode::Path => {
                self.session.open_highlighted();
                self.apply_input(cx);
            }
        }
    }

    fn confirm_typed_path(&mut self, _: &ConfirmTypedPath, _: &mut Window, cx: &mut Context<Self>) {
        self.confirm(true, cx);
    }

    fn go_back(&mut self, _: &GoBack, _: &mut Window, cx: &mut Context<Self>) {
        self.session.go_back();
        self.apply_input(cx);
    }

    fn complete_highlighted(
        &mut self,
        _: &CompleteHighlighted,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session.complete_highlighted();
        self.apply_input(cx);
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::Dismiss);
    }

    fn confirm(&mut self, allow_create: bool, cx: &mut Context<Self>) {
        let Some(path) = self.session.confirmation_path() else {
            return;
        };
        let create_if_missing = allow_create
            && self.session.input_mode() == InputMode::Path
            && self.session.typed_path_state()
                == muxy_api::picker::path_service::TypedPathState::Missing;
        cx.emit(PickerEvent::Confirm {
            path,
            create_if_missing,
        });
    }

    fn activate_row(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn path_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing4())
            .px(metrics.spacing6())
            .py(metrics.spacing5())
            .child(SymbolGlyph::new(
                "magnifyingglass",
                metrics.font_body(),
                theme.fg_muted,
            ))
            .child(muxy_ui::text_input::growing_input(&self.input))
            .child(self.action_button(cx))
            .into_any_element()
    }

    fn action_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let enabled = self.session.confirmation_path().is_some();
        let title = self.session.top_right_action_title();

        let confirm = div()
            .id("picker-confirm")
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing2())
            .pl(metrics.spacing3())
            .pr(metrics.spacing4())
            .py(metrics.spacing2())
            .when(enabled, |element| {
                element
                    .cursor_pointer()
                    .on_click(cx.listener(|picker: &mut Self, _, _, cx| {
                        picker.confirm(true, cx);
                    }))
            })
            .when(!enabled, |element| element.opacity(0.4))
            .child(SymbolGlyph::new("plus", metrics.font_footnote(), theme.fg))
            .child(
                div()
                    .text_size(metrics.font_footnote())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.fg)
                    .child(SharedString::from(title)),
            );

        let chevron = div()
            .id("picker-menu")
            .relative()
            .flex()
            .items_center()
            .px(metrics.spacing3())
            .py(metrics.spacing2())
            .cursor_pointer()
            .child(SymbolGlyph::new(
                "chevron.down",
                metrics.font_caption(),
                theme.fg,
            ))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|picker: &mut Self, _, _, cx| {
                    picker.action_menu_open = !picker.action_menu_open;
                    cx.stop_propagation();
                    cx.notify();
                }),
            );

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .rounded(metrics.radius_md())
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .child(confirm)
            .child(
                div()
                    .w(px(1.0))
                    .h(metrics.control_medium())
                    .bg(theme.border),
            )
            .child(chevron)
            .into_any_element()
    }

    fn action_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let needs_fix = !self
            .session
            .path_service
            .location_status(&self.session.search_root_path)
            .is_ready();

        let entry =
            |id: &'static str, symbol: &'static str, label: &'static str, color: gpui::Hsla| {
                div()
                    .id(id)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(metrics.spacing3())
                    .px(metrics.spacing3())
                    .py(metrics.spacing2())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.hover))
                    .child(SymbolGlyph::new(symbol, metrics.font_footnote(), color))
                    .child(
                        div()
                            .text_size(metrics.font_footnote())
                            .text_color(color)
                            .child(SharedString::from(label)),
                    )
            };

        div()
            .occlude()
            .absolute()
            .right(metrics.spacing6())
            .top(metrics.control_large() + metrics.spacing6())
            .flex()
            .flex_col()
            .w(metrics.scaled(200.0))
            .p(metrics.spacing2())
            .rounded(metrics.radius_lg())
            .bg(theme.raised())
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .child(
                entry("choose-finder", "folder", "Choose in Finder", theme.fg).on_click(
                    cx.listener(|picker: &mut Self, _, _, cx| {
                        picker.action_menu_open = false;
                        picker.choose_with_finder(cx);
                    }),
                ),
            )
            .child(
                entry(
                    "edit-search-location",
                    if needs_fix {
                        "exclamationmark.triangle.fill"
                    } else {
                        "gearshape"
                    },
                    if needs_fix {
                        "Fix Search Location"
                    } else {
                        "Edit Search Location"
                    },
                    if needs_fix { theme.warning } else { theme.fg },
                )
                .on_click(cx.listener(|picker: &mut Self, _, _, cx| {
                    picker.action_menu_open = false;
                    picker.edit_search_location(cx);
                })),
            )
            .into_any_element()
    }

    fn choose_with_finder(&mut self, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::ChooseFinder {
            directory: self.session.search_root_path.clone(),
        });
    }

    fn edit_search_location(&mut self, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::EditSearchLocation {
            directory: self.session.search_root_path.clone(),
        });
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.session.load_state.is_loading() {
            return self.loading_content();
        }
        if self.session.shows_unavailable_state() {
            return self.unavailable_content(cx);
        }
        match self.session.input_mode() {
            InputMode::FolderSearch => self.search_rows(cx),
            InputMode::Path => self.directory_rows(cx),
        }
    }

    fn loading_content(&self) -> AnyElement {
        let mut container = div()
            .flex()
            .flex_grow()
            .items_center()
            .justify_center()
            .size_full();
        if self.session.load_state.shows_message() {
            container = container.child(
                div()
                    .text_size(self.metrics.font_body())
                    .text_color(self.theme.fg_muted)
                    .child(SharedString::from("Loading…")),
            );
        }
        container.into_any_element()
    }

    fn unavailable_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;

        let mut container = div().flex().flex_col().flex_grow().min_h(px(0.0));
        if self.session.has_parent_row() {
            container = container.child(self.directory_row(0, &DirectoryItem::Parent, cx));
        }

        container
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .items_center()
                    .justify_center()
                    .gap(metrics.spacing4())
                    .child(
                        div()
                            .text_size(metrics.font_body())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.fg_muted)
                            .child(SharedString::from(self.unavailable_title())),
                    )
                    .child(
                        div()
                            .max_w(metrics.scaled(420.0))
                            .text_size(metrics.font_footnote())
                            .text_color(theme.fg_dim)
                            .text_center()
                            .child(SharedString::from(self.unavailable_description())),
                    ),
            )
            .into_any_element()
    }

    fn unavailable_title(&self) -> String {
        if self.session.input_mode() != InputMode::FolderSearch {
            return "No project folders found".to_owned();
        }
        if self.session.load_state.read_failed() {
            return "Folder search unavailable".to_owned();
        }
        if self.session.search_query().is_empty() {
            "Find a project folder".to_owned()
        } else {
            "No matching folders".to_owned()
        }
    }

    fn unavailable_description(&self) -> String {
        if self.session.input_mode() != InputMode::FolderSearch {
            return "Use the action above to open or create this project, go up, or choose with Finder."
                .to_owned();
        }
        let root = self
            .session
            .path_service
            .abbreviated_display_path(&self.session.search_root_path);
        if self.session.load_state.read_failed() {
            return "Check the folder search location, enter a path, or choose with Finder."
                .to_owned();
        }
        if self.session.search_query().is_empty() {
            return format!("Type a folder name to search inside {root}, or enter a path.");
        }
        if self.session.folder_search_is_truncated {
            return "The folder index reached its safety limit. Refine the search location or enter a path."
                .to_owned();
        }
        format!(
            "No folders in {root} match “{}”. You can still enter a path.",
            self.session.search_query()
        )
    }

    fn directory_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div().flex().flex_col();
        for (index, row) in self.session.rows.iter().enumerate() {
            list = list.child(self.directory_row(index, row, cx));
        }
        div()
            .id("picker-directory-rows")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn directory_row(
        &self,
        index: usize,
        row: &DirectoryItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let highlighted = self.session.highlighted_index == Some(index);
        let symbol = if row.is_parent() {
            "arrow.turn.up.left"
        } else {
            "folder"
        };

        div()
            .id(SharedString::from(format!("picker-row-{index}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing3())
            .px(metrics.spacing5())
            .py(metrics.spacing3())
            .cursor_pointer()
            .when(highlighted, |element| element.bg(theme.surface))
            .when(!highlighted, |element| {
                element.hover(|style| style.bg(theme.hover))
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(metrics.scaled(16.0))
                    .child(SymbolGlyph::new(
                        symbol,
                        metrics.font_body(),
                        theme.fg_muted,
                    )),
            )
            .child(
                div()
                    .font_family(MONOSPACE_FONT)
                    .text_size(metrics.font_body())
                    .text_color(theme.fg)
                    .when(row.is_symlink(), |element| element.italic())
                    .child(SharedString::from(row.name().to_owned())),
            )
            .on_click(cx.listener(move |picker: &mut Self, _, _, cx| {
                picker.activate_row(index, cx);
            }))
            .into_any_element()
    }

    fn search_rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let mut list = div().flex().flex_col();

        for (index, result) in self.session.search_results.iter().enumerate() {
            let highlighted = self.session.highlighted_index == Some(index);
            list = list.child(
                div()
                    .id(SharedString::from(format!("picker-result-{index}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(metrics.spacing3())
                    .px(metrics.spacing5())
                    .py(metrics.spacing3())
                    .cursor_pointer()
                    .when(highlighted, |element| element.bg(theme.surface))
                    .when(!highlighted, |element| {
                        element.hover(|style| style.bg(theme.hover))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(metrics.scaled(16.0))
                            .child(SymbolGlyph::new(
                                "folder",
                                metrics.font_body(),
                                theme.fg_muted,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(px(0.0))
                            .gap(metrics.spacing1())
                            .child(
                                div()
                                    .font_family(MONOSPACE_FONT)
                                    .text_size(metrics.font_body())
                                    .text_color(theme.fg)
                                    .child(SharedString::from(result.name.clone())),
                            )
                            .child(
                                div()
                                    .font_family(MONOSPACE_FONT)
                                    .text_size(metrics.font_footnote())
                                    .text_color(theme.fg_dim)
                                    .truncate()
                                    .child(SharedString::from(result.display_path.clone())),
                            ),
                    )
                    .on_click(cx.listener(move |picker: &mut Self, _, _, cx| {
                        picker.activate_row(index, cx);
                    })),
            );
        }

        if let Some(notice) = self.search_notice() {
            list = list.child(
                div()
                    .flex()
                    .justify_center()
                    .py(metrics.spacing3())
                    .text_size(metrics.font_footnote())
                    .text_color(theme.fg_dim)
                    .child(SharedString::from(notice)),
            );
        }

        div()
            .id("picker-search-rows")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn search_notice(&self) -> Option<&'static str> {
        if self.session.folder_search_has_more_results {
            return Some("More matches available — keep typing to narrow the results.");
        }
        if self.session.folder_search_is_truncated {
            return Some(
                "Folder index safety limit reached — some paths may require direct entry.",
            );
        }
        None
    }

    fn footer(&self) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let mode = self.session.input_mode();
        let action_title = if mode == InputMode::FolderSearch {
            self.session.action_title()
        } else {
            self.session.top_right_action_title()
        };

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(metrics.scaled(18.0))
            .px(metrics.spacing5())
            .py(metrics.spacing4());

        for shortcut in shortcuts::ordered(mode, action_title) {
            let mut keycap = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.scaled(3.0))
                .px(metrics.scaled(4.0))
                .py(metrics.scaled(2.0))
                .rounded(metrics.radius_sm())
                .bg(theme.surface)
                .border_1()
                .border_color(theme.border);

            for part in shortcut.parts {
                keycap = keycap.child(match part {
                    KeycapPart::Symbol(symbol) => {
                        SymbolGlyph::new(*symbol, metrics.font_caption(), theme.fg_muted)
                            .into_any_element()
                    }
                    KeycapPart::Text(text) => div()
                        .font_family(MONOSPACE_FONT)
                        .text_size(metrics.font_caption())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(*text))
                        .into_any_element(),
                });
            }

            row = row.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(metrics.scaled(4.0))
                    .child(keycap)
                    .child(
                        div()
                            .text_size(metrics.font_footnote())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.fg_dim)
                            .child(SharedString::from(shortcut.label)),
                    ),
            );
        }

        row.into_any_element()
    }
}

impl Render for ProjectPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = self.metrics;
        let theme = self.theme.clone();
        if !self.focused {
            self.focused = true;
            window.focus(&self.input.focus_handle(cx));
        }

        let divider = || div().h(px(1.0)).flex_none().bg(theme.border);

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .key_context(KEY_CONTEXT)
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(Self::move_up))
                    .on_action(cx.listener(Self::move_down))
                    .on_action(cx.listener(Self::open_highlighted))
                    .on_action(cx.listener(Self::confirm_typed_path))
                    .on_action(cx.listener(Self::go_back))
                    .on_action(cx.listener(Self::complete_highlighted))
                    .on_action(cx.listener(Self::dismiss))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|picker: &mut Self, _, _, cx| {
                            cx.stop_propagation();
                            if picker.action_menu_open {
                                picker.action_menu_open = false;
                                cx.notify();
                            }
                        }),
                    )
                    .occlude()
                    .mt(metrics.scaled(PANEL_TOP))
                    .flex()
                    .flex_col()
                    .w(metrics.scaled(PANEL_WIDTH))
                    .h(metrics.scaled(PANEL_HEIGHT))
                    .rounded(metrics.radius_xl())
                    .bg(theme.bg)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .overflow_hidden()
                    .relative()
                    .child(self.path_bar(cx))
                    .child(divider())
                    .child(self.content(cx))
                    .child(divider())
                    .child(self.footer())
                    .when(self.action_menu_open, |element| {
                        element.child(self.action_menu(cx))
                    }),
            )
    }
}
