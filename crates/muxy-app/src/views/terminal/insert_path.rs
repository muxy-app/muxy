//! Insert Path: a popover at the cursor that browses the folders of the
//! terminal's computer and types the path of the entry the user picks.

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Subscription, Task, Window, deferred, div,
};
use muxy_protocol::ServerPath;
use muxy_ui::icon::Icon;
use muxy_ui::picker::{
    Picker, PickerConfig, PickerEvent, PickerItem, PickerLeading, PickerRow, PickerStatus,
};
use muxy_ui::popover::PopoverAnchor;
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

use super::pane::{PaneEvent, PaneState, TerminalPane};

mod browser;

use browser::Browser;
pub(crate) use browser::Entry;

/// How wide the popover is.
const WIDTH: f32 = 320.0;
/// How long a folder may take to list before the popover says it is loading,
/// so the rows of the folder before don't blink away for a quick answer.
const LOADING_DELAY: Duration = Duration::from_millis(150);

/// Lists the files and folders directly inside an absolute path on the
/// terminal's computer, or says why it can't.
pub(crate) type Lister = Arc<dyn Fn(&str) -> Result<Vec<Entry>, String> + Send + Sync>;

/// What the app gives a pane to browse its server's folders with.
pub(crate) struct Source {
    /// `Err` says why folders can't be listed now.
    pub(crate) list: Result<Lister, String>,
    /// The Home folder there, which `~` stands for.
    pub(crate) home: Option<String>,
    /// Paths chosen there before, most recent first.
    pub(crate) recents: Vec<String>,
}

pub(crate) struct InsertPath {
    pub(crate) picker: Entity<Picker>,
    /// The cursor's cell, which the popover opens from.
    anchor: PopoverAnchor,
    browser: Option<Browser>,
    list: Option<Lister>,
    listing: Listing,
    /// Counts listings asked for, so only the latest one's answer is shown.
    generation: usize,
    load: Option<Task<()>>,
    _subscriptions: [Subscription; 2],
}

enum Listing {
    Loading,
    Listed(Vec<Entry>),
    Failed(String),
}

impl TerminalPane {
    /// Opens the popover at the cursor, or closes the open one. The app
    /// answers [`PaneEvent::BrowsePaths`] with what to browse.
    pub(crate) fn open_insert_path(
        &mut self,
        theme: &Theme,
        metrics: Metrics,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.insert_path.is_some() {
            self.close_insert_path(true, window, cx);
            return;
        }
        if self.grid.is_none() || self.state != PaneState::Live {
            return;
        }
        if self.scroll.view.is_some() {
            self.scroll_to_bottom(cx);
        }
        let picker = cx.new(|cx| {
            let config = PickerConfig {
                width: Some(WIDTH),
                compact: true,
                completion_on_tab: true,
                ..PickerConfig::popover("insert-path", "")
            };
            let mut picker = Picker::new(config, theme.clone(), metrics, cx);
            picker.set_status(PickerStatus::Loading(tr!("Loading…")), cx);
            picker
        });
        let events = cx.subscribe_in(&picker, window, |pane, _, event, window, cx| match event {
            PickerEvent::QueryChanged { query, .. } => pane.filter_paths(query, cx),
            PickerEvent::Confirmed(selection) => pane.choose_path(&selection.id, window, cx),
            PickerEvent::CompletionRequested => pane.enter_selected_folder(cx),
            PickerEvent::NavigateBackRequested => pane.leave_folder(cx),
            PickerEvent::Dismissed => pane.close_insert_path(true, window, cx),
            _ => {}
        });
        let blur = cx.on_focus_out(&picker.focus_handle(cx), window, |pane, _, window, cx| {
            // A window that stops being the active one reports the same,
            // with the keyboard still in the popover, which then stays.
            let kept = pane
                .insert_path
                .as_ref()
                .is_some_and(|open| open.picker.focus_handle(cx).contains_focused(window, cx));
            if !kept {
                pane.close_insert_path(false, window, cx);
            }
        });
        self.insert_path = Some(InsertPath {
            picker,
            anchor: PopoverAnchor::default(),
            browser: None,
            list: None,
            listing: Listing::Loading,
            generation: 0,
            load: None,
            _subscriptions: [events, blur],
        });
        cx.emit(PaneEvent::BrowsePaths);
        cx.notify();
    }

    /// Closes the popover. `refocus` gives the keyboard back to the
    /// terminal, unless something else already took it.
    pub(crate) fn close_insert_path(
        &mut self,
        refocus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.insert_path.take().is_none() {
            return;
        }
        if refocus {
            self.focus.focus(window);
        }
        cx.notify();
    }

    pub(crate) fn update_insert_path_appearance(
        &mut self,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        if let Some(open) = &self.insert_path {
            open.picker.update(cx, |picker, cx| {
                picker.set_appearance(theme.clone(), metrics, cx);
            });
        }
    }

    /// Starts browsing with `source`, in the terminal's folder.
    pub(crate) fn browse_paths(&mut self, source: Source, cx: &mut Context<Self>) {
        let base = self.server_directory();
        let Some(open) = &mut self.insert_path else {
            return;
        };
        open.browser = Browser::new(base, source.home, source.recents);
        open.listing = Listing::Loading;
        open.load = None;
        open.generation += 1;
        match (source.list, &open.browser) {
            (Ok(list), Some(_)) => {
                open.list = Some(list);
                self.list_folder(cx);
            }
            (Err(message), _) => {
                open.listing = Listing::Failed(message);
                self.show_paths(cx);
            }
            (Ok(_), None) => {
                open.listing =
                    Listing::Failed(tr!("This terminal's folder isn't known yet.").to_string());
                self.show_paths(cx);
            }
        }
    }

    /// Lists the browsed folder. The rows on show stay until the answer
    /// comes, or until it has taken long enough to say so.
    fn list_folder(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &mut self.insert_path else {
            return;
        };
        let (Some(browser), Some(list)) = (&open.browser, open.list.clone()) else {
            return;
        };
        open.generation += 1;
        open.listing = Listing::Loading;
        let generation = open.generation;
        let directory = browser.directory().to_owned();
        let label = browser.label();
        open.picker.update(cx, |picker, cx| {
            picker.set_placeholder(label, cx);
            cx.notify();
        });
        let listed = cx
            .background_executor()
            .spawn(async move { list(&directory) });
        let slow = cx.background_executor().timer(LOADING_DELAY);
        open.load = Some(cx.spawn(async move |pane, cx| {
            let slow = cx.spawn({
                let pane = pane.clone();
                async move |cx| {
                    slow.await;
                    let _ = pane.update(cx, |pane, cx| {
                        if pane.lists(generation) {
                            pane.show_paths(cx);
                        }
                    });
                }
            });
            let listed = listed.await;
            drop(slow);
            let _ = pane.update(cx, |pane, cx| {
                if !pane.lists(generation) {
                    return;
                }
                if let Some(open) = &mut pane.insert_path {
                    open.listing = match listed {
                        Ok(entries) => Listing::Listed(entries),
                        Err(message) => Listing::Failed(message),
                    };
                }
                pane.show_paths(cx);
            });
        }));
    }

    /// Whether the popover still waits for listing `generation`.
    fn lists(&self, generation: usize) -> bool {
        self.insert_path.as_ref().is_some_and(|open| {
            open.generation == generation && matches!(open.listing, Listing::Loading)
        })
    }

    /// Shows the rows of the browsed folder that match the filter.
    fn show_paths(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.insert_path else {
            return;
        };
        let filter = open.picker.read(cx).query().to_owned();
        let (items, status) = match (&open.listing, &open.browser) {
            (Listing::Loading, _) => (Vec::new(), PickerStatus::Loading(tr!("Loading…"))),
            (Listing::Failed(message), _) => {
                (Vec::new(), PickerStatus::Error(message.clone().into()))
            }
            (Listing::Listed(_), None) => (Vec::new(), PickerStatus::Ready),
            (Listing::Listed(entries), Some(browser)) => {
                let items: Vec<_> = browser
                    .rows(entries, &filter)
                    .into_iter()
                    .map(|row| {
                        let mut item = PickerRow::new(row.name.clone(), row.name);
                        item.leading = Some(PickerLeading::Icon(if row.is_directory {
                            Icon::Folder
                        } else {
                            Icon::File
                        }));
                        item.trailing_icon = row.recent.then_some(Icon::History);
                        PickerItem::Row(item)
                    })
                    .collect();
                let status = if !items.is_empty() {
                    PickerStatus::Ready
                } else if entries.is_empty() {
                    PickerStatus::Empty(tr!("Folder is empty"))
                } else if filter.is_empty() && Browser::hides(entries) {
                    PickerStatus::Empty(tr!("Only hidden files. Type . to list them."))
                } else {
                    PickerStatus::Empty(tr!("No matching files or folders"))
                };
                (items, status)
            }
        };
        // A new filter or folder starts from its best row; the picker would
        // otherwise keep a row chosen before, wherever it now is.
        let first = items.iter().cloned().find_map(PickerItem::row_data);
        open.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            if let Some(first) = first {
                let _ = picker.select_row(&first.id, cx);
            }
        });
    }

    /// Follows the folders the filter names, then filters by what is left.
    fn filter_paths(&mut self, query: &str, cx: &mut Context<Self>) {
        let Some(open) = &mut self.insert_path else {
            return;
        };
        let entries = match &open.listing {
            Listing::Listed(entries) => entries.as_slice(),
            Listing::Loading | Listing::Failed(_) => &[],
        };
        let followed = open
            .browser
            .as_mut()
            .and_then(|browser| browser.follow(query, entries));
        match followed {
            Some(rest) => {
                open.picker
                    .update(cx, |picker, cx| picker.set_query(rest, cx));
                self.list_folder(cx);
            }
            None if matches!(open.listing, Listing::Loading) => {}
            None => self.show_paths(cx),
        }
    }

    /// Browses the selected folder. A selected file stays as it is.
    fn enter_selected_folder(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &mut self.insert_path else {
            return;
        };
        let Some(name) = open.picker.read(cx).selected_row_id().map(str::to_owned) else {
            return;
        };
        let folder = matches!(&open.listing, Listing::Listed(entries)
            if entries.iter().any(|entry| entry.is_directory && entry.name == name));
        let Some(browser) = open.browser.as_mut().filter(|_| folder) else {
            return;
        };
        browser.enter(&name);
        open.picker
            .update(cx, |picker, cx| picker.set_query("", cx));
        self.list_folder(cx);
    }

    /// Browses the folder above the browsed one.
    fn leave_folder(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &mut self.insert_path else {
            return;
        };
        if open.browser.as_mut().is_some_and(Browser::leave) {
            open.picker
                .update(cx, |picker, cx| picker.set_query("", cx));
            self.list_folder(cx);
        }
    }

    /// Types the path of the row `name` at the cursor and closes the
    /// popover. A path that can't be typed safely stays untyped, and the
    /// popover says so.
    fn choose_path(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.insert_path else {
            return;
        };
        let (Listing::Listed(entries), Some(browser)) = (&open.listing, &open.browser) else {
            return;
        };
        let Some(entry) = entries.iter().find(|entry| entry.name == name) else {
            return;
        };
        let chosen = browser.chosen(&entry.name, entry.is_directory);
        let picker = open.picker.clone();
        if self.state != PaneState::Live || self.channel().is_none() {
            // The terminal ended or lost its server meanwhile, so nothing
            // can be typed into it.
            self.close_insert_path(true, window, cx);
            return;
        }
        if !self.type_path(&ServerPath::from(chosen.typed.as_str()), cx) {
            let message = tr!("This path can't be typed safely into the program running here.");
            picker.update(cx, |picker, cx| {
                picker.set_status(PickerStatus::Error(message), cx);
            });
            return;
        }
        self.close_insert_path(true, window, cx);
        cx.emit(PaneEvent::PathChosen(chosen.absolute));
    }

    /// Backspace in an empty filter goes to the folder above.
    fn backspace_in_filter(&mut self, cx: &mut Context<Self>) {
        let empty = self
            .insert_path
            .as_ref()
            .is_some_and(|open| open.picker.read(cx).query().is_empty());
        if empty {
            self.leave_folder(cx);
            cx.stop_propagation();
        }
    }
}

/// The popover, placed at the cursor above everything else in the window.
pub(super) fn popover(pane: &TerminalPane, cx: &mut Context<TerminalPane>) -> Option<AnyElement> {
    let open = pane.insert_path.as_ref()?;
    open.anchor.set(pane.composition_bounds());
    let content = div()
        .capture_action(
            cx.listener(|pane, _: &muxy_ui::text_input::Backspace, _, cx| {
                pane.backspace_in_filter(cx);
            }),
        )
        .child(open.picker.clone())
        .into_any_element();
    let owner = cx.weak_entity();
    let popover =
        muxy_ui::popover::anchored_popover(open.anchor.clone(), content, move |window, cx| {
            let _ = owner.update(cx, |pane, cx| pane.close_insert_path(true, window, cx));
        });
    Some(deferred(popover).with_priority(1).into_any_element())
}
