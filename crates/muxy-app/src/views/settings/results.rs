use super::{Category, SettingsView, appearance, keyboard, server, terminal};
use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement, IntoElement, ListAlignment, ListState,
    ParentElement, Pixels, Styled, Window, div, px,
};
use std::{cell::Cell, collections::HashMap, rc::Rc};

pub(super) struct Results {
    pub(super) state: ListState,
    items: Vec<Item>,
    focus: HashMap<Category, FocusHandle>,
    pub(super) row_focus: HashMap<String, FocusHandle>,
    pub(super) shortcut_focus: Vec<[FocusHandle; 2]>,
    pub(super) reveal_focus: Rc<Cell<bool>>,
    pub(super) dirty: bool,
    reset_scroll: bool,
    overdraw: Pixels,
}

#[derive(Clone, Copy)]
enum Item {
    Section(Category),
    KeyboardHeading,
    Shortcut(usize),
    Empty,
    Footer,
}

impl Results {
    pub(super) fn new(cx: &mut Context<SettingsView>) -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(0.0)),
            items: Vec::new(),
            focus: Category::ALL
                .into_iter()
                .map(|category| (category, cx.focus_handle()))
                .collect(),
            row_focus: super::catalog::SETTINGS
                .iter()
                .map(|setting| setting.id)
                .chain(muxy_core::shortcuts::ALL.iter().map(|shortcut| shortcut.id))
                .map(|id| (id.to_owned(), cx.focus_handle()))
                .collect(),
            shortcut_focus: muxy_core::shortcuts::ALL
                .iter()
                .map(|_| {
                    [
                        cx.focus_handle().tab_stop(true),
                        cx.focus_handle().tab_stop(true),
                    ]
                })
                .collect(),
            reveal_focus: Rc::new(Cell::new(false)),
            dirty: true,
            reset_scroll: true,
            overdraw: px(0.0),
        }
    }

    pub(super) fn reset(&mut self) {
        self.dirty = true;
        self.reset_scroll = true;
    }
}

impl SettingsView {
    pub(super) fn advance_focus(
        &mut self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.results.reveal_focus.set(true);
        let controls: Vec<_> = self
            .results
            .items
            .iter()
            .enumerate()
            .filter_map(|(item, entry)| match entry {
                Item::Shortcut(index) => Some((item, &self.results.shortcut_focus[*index])),
                _ => None,
            })
            .flat_map(|(item, handles)| handles.iter().map(move |focus| (item, focus)))
            .collect();
        let target = controls
            .iter()
            .position(|(_, focus)| focus.is_focused(window))
            .and_then(|current| {
                if backwards {
                    current.checked_sub(1)
                } else {
                    current.checked_add(1)
                }
            })
            .and_then(|index| controls.get(index));
        if let Some((item, focus)) = target {
            let viewport = self.results.state.viewport_bounds();
            if self
                .results
                .state
                .bounds_for_item(*item)
                .is_none_or(|bounds| {
                    bounds.top() < viewport.top() || bounds.bottom() > viewport.bottom()
                })
            {
                self.results.state.scroll_to_reveal_item(*item);
            }
            focus.focus(window);
        } else if backwards {
            window.focus_prev();
        } else {
            window.focus_next();
        }
        cx.notify();
    }

    pub(super) fn refresh_results(
        &mut self,
        overdraw: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self.results.dirty && self.results.overdraw == overdraw {
            return;
        }
        let mut items = Vec::new();
        let order = if self.query.is_empty() {
            Category::ALL
        } else {
            [
                Category::General,
                Category::QuickTerminal,
                Category::Composer,
                Category::Appearance,
                Category::Terminal,
                Category::Keyboard,
                Category::Server,
                Category::Extensions,
            ]
        };
        for category in order {
            if self.query.is_empty() && category != self.category {
                continue;
            }
            if category == Category::Keyboard {
                let shortcuts = keyboard::matching(self);
                if !shortcuts.is_empty() {
                    items.push(Item::KeyboardHeading);
                    items.extend(shortcuts.into_iter().map(Item::Shortcut));
                }
            } else if !self.section_rows(category, window, cx).is_empty() {
                items.push(Item::Section(category));
            }
        }
        if items.is_empty() {
            items.push(Item::Empty);
        }
        items.push(Item::Footer);
        let offset = self.results.state.logical_scroll_top();
        self.scrollbar.reset();
        if self.results.overdraw == overdraw {
            self.results.state.reset(items.len());
        } else {
            self.results.state = ListState::new(items.len(), ListAlignment::Top, overdraw);
            self.results.overdraw = overdraw;
        }
        self.results.state.splice_focusable(
            0..items.len(),
            items.iter().map(|item| match item {
                Item::Section(category) => Some(self.results.focus[category].clone()),
                Item::Shortcut(index) => {
                    Some(self.results.row_focus[muxy_core::shortcuts::ALL[*index].id].clone())
                }
                _ => None,
            }),
        );
        if !self.results.reset_scroll {
            self.results.state.scroll_to(offset);
        }
        self.results.items = items;
        self.results.dirty = false;
        self.results.reset_scroll = false;
    }

    fn section_rows(
        &self,
        category: Category,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match category {
            Category::General | Category::Appearance => appearance::rows(self, category, cx),
            Category::Composer => super::composer::rows(self, cx),
            Category::Terminal => terminal::rows(self, window, cx),
            Category::Server => server::rows(self, cx),
            Category::Keyboard => Vec::new(),
            Category::Extensions => {
                if self.matches(
                    Category::Extensions,
                    "Manage extensions marketplace installed unpacked",
                ) {
                    vec![
                        muxy_ui::controls::button(
                            self.style(),
                            "manage-extensions",
                            "Manage Extensions",
                            true,
                            cx.listener(|view, _, _, cx| view.show_extensions(cx)),
                        )
                        .into_any_element(),
                    ]
                } else {
                    Vec::new()
                }
            }
            Category::QuickTerminal => super::quick_terminal::rows(self, cx),
        }
    }

    fn section_heading(&self, category: Category, first: bool) -> AnyElement {
        let metrics = self.layout_style().metrics;
        div()
            .pt(if first {
                metrics.spacing2()
            } else {
                metrics.spacing9()
            })
            .text_size(metrics.font_display())
            .child(category.label())
            .into_any_element()
    }

    pub(super) fn subsection_heading(&self, section: &'static str) -> AnyElement {
        muxy_ui::form::section_heading(self.layout_style(), section)
            .debug_selector(move || format!("settings-heading-{section}"))
            .into_any_element()
    }

    pub(super) fn result(
        &mut self,
        index: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.results.items[index] {
            Item::Section(category) => div()
                .w_full()
                .min_w(px(0.0))
                .track_focus(&self.results.focus[&category])
                .debug_selector(move || format!("settings-section-{}", category.label()))
                .child(self.section_heading(category, index == 0))
                .children(self.section_rows(category, window, cx))
                .into_any_element(),
            Item::KeyboardHeading => div()
                .w_full()
                .debug_selector(|| "settings-section-Keyboard".into())
                .child(self.section_heading(Category::Keyboard, index == 0))
                .child(self.subsection_heading("Shortcuts"))
                .child(self.note("Click a shortcut to record it. Escape cancels. Conflicts in the same context must be resolved first.", false))
                .into_any_element(),
            Item::Shortcut(index) => {
                #[cfg(test)]
                { self.shortcut_row_count += 1; }
                keyboard::row(self, index, cx)
            },
            Item::Empty => div()
                .w_full()
                .debug_selector(|| "settings-empty".into())
                .child(self.note("No settings found. Try another search.", false))
                .into_any_element(),
            Item::Footer => self.note(
                    "Changes apply immediately. Press Return or leave a field to save text.",
                    false,
                )
                .into_any_element(),
        }
    }
}
