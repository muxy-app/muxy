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

const WIDTH: f32 = 320.0;
const LOADING_DELAY: Duration = Duration::from_millis(150);

pub(crate) type Lister = Arc<dyn Fn(&str) -> Result<Vec<Entry>, String> + Send + Sync>;

pub(crate) struct Source {
    pub(crate) list: Result<Lister, String>,
    pub(crate) home: Option<String>,
    pub(crate) recents: Vec<String>,
}

pub(crate) struct InsertPath {
    pub(crate) picker: Entity<Picker>,
    anchor: PopoverAnchor,
    browser: Option<Browser>,
    list: Option<Lister>,
    listing: Listing,
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

    fn lists(&self, generation: usize) -> bool {
        self.insert_path.as_ref().is_some_and(|open| {
            open.generation == generation && matches!(open.listing, Listing::Loading)
        })
    }

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
        let first = items.iter().cloned().find_map(PickerItem::row_data);
        open.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            if let Some(first) = first {
                let _ = picker.select_row(&first.id, cx);
            }
        });
    }

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

    fn choose_path(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let base = self.server_directory();
        let Some(open) = &mut self.insert_path else {
            return;
        };
        let (Listing::Listed(entries), Some(browser)) = (&open.listing, &mut open.browser) else {
            return;
        };
        let Some(entry) = entries.iter().find(|entry| entry.name == name) else {
            return;
        };
        browser.rebase(base);
        let chosen = browser.chosen(&entry.name, entry.is_directory);
        let picker = open.picker.clone();
        if self.state != PaneState::Live || self.channel().is_none() {
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
