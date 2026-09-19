use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, list, px,
};
use muxy_ui::components::ButtonInteraction;
use muxy_ui::{controls, navigation};

use super::{Category, SettingsEvent, SettingsView};
use crate::views::titlebar;

impl Category {
    fn sections(self) -> &'static [&'static str] {
        match self {
            Self::QuickTerminal => &["General", "Shortcut", "Size", "Appearance"],
            Self::Composer => &["Behavior", "Text", "Voice"],
            Self::General => &["Closing terminals", "Window size"],
            Self::Appearance => &["Themes", "Interface"],
            Self::Keyboard => &["Shortcuts"],
            Self::Terminal => &["Text", "Behavior", "Configuration"],
            Self::Server => &["Connection", "Sessions", "Server control"],
            Self::Extensions => &[],
        }
    }
}

impl SettingsView {
    fn select_category(
        &mut self,
        category: Category,
        section: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recording = None;
        self.quick_recording = None;
        if section.is_none()
            && self.category == category
            && self.query.is_empty()
            && !self.expanded.remove(&category)
        {
            self.expanded.insert(category);
        }
        self.category = category;
        self.section = section;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.focus.focus(window);
        if category == Category::Server {
            cx.emit(SettingsEvent::ReadServer);
        }
        cx.notify();
    }

    fn disclosure(&self, category: Category, cx: &mut Context<Self>) -> AnyElement {
        navigation::disclosure(
            self.layout_style(),
            SharedString::from(format!("settings-disclosure-{}", category.label())),
            self.expanded.contains(&category),
            cx.listener(move |view, _, _, cx| {
                if !view.expanded.remove(&category) {
                    view.expanded.insert(category);
                }
                cx.notify();
            }),
        )
        .debug_selector(move || format!("settings-disclosure-{}", category.label()))
        .into_any_element()
    }

    fn categories(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut categories = div().flex().gap(px(2.0)).map(|nav| {
            if self.compact {
                nav.flex_wrap()
            } else {
                nav.flex_col()
            }
        });
        for category in Category::ALL {
            let selected = self.query.is_empty() && self.category == category;
            let expanded = self.expanded.contains(&category);
            categories = categories.child(
                navigation::item(
                    self.layout_style(),
                    SharedString::from(format!("settings-category-{}", category.label())),
                    selected,
                    cx.listener(move |view, _, window, cx| {
                        view.select_category(category, None, window, cx);
                    }),
                )
                .debug_selector(move || format!("settings-category-{}", category.label()))
                .when(category == Category::General, |row| {
                    row.track_focus(&self.navbar_focus)
                })
                .child(self.disclosure(category, cx))
                .child(category.label()),
            );
            if expanded && !self.compact {
                for &section in category.sections() {
                    categories = categories.child(
                        navigation::subitem(
                            self.layout_style(),
                            SharedString::from(format!("settings-subcategory-{section}")),
                            selected && self.section == Some(section),
                            cx.listener(move |view, _, window, cx| {
                                view.select_category(category, Some(section), window, cx);
                            }),
                        )
                        .debug_selector(move || format!("settings-subcategory-{section}"))
                        .child(section),
                    );
                }
            }
        }
        categories.into_any_element()
    }

    fn titlebar(&self, id: &'static str) -> gpui::Stateful<gpui::Div> {
        let focus = self.focus.clone();
        titlebar::background(id).on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
            focus.focus(window);
        })
    }

    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .debug_selector(|| "settings-navigation".into())
            .flex_none()
            .map(|nav| {
                if self.compact {
                    nav.w_full().border_b_1()
                } else {
                    nav.w(px(248.0)).h_full().border_r_1()
                }
            })
            .flex()
            .flex_col()
            .bg(self.theme.raised())
            .border_color(self.theme.border)
            .child(
                self.titlebar("settings-sidebar-titlebar")
                    .h(px(40.0))
                    .flex_none(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_h(px(0.0))
                    .flex_1()
                    .px(px(12.0))
                    .gap(px(12.0))
                    .child(
                        controls::search_field(
                            self.layout_style(),
                            "settings-search",
                            &self.search,
                        )
                        .debug_selector(|| "settings-search".into()),
                    )
                    .child(
                        div()
                            .id("settings-categories")
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .child(self.categories(cx)),
                    )
                    .when(!self.compact, |nav| nav.child(div().flex_1()))
                    .when(!self.compact, |nav| {
                        nav.child(
                            div()
                                .border_t_1()
                                .border_color(self.theme.border)
                                .py(px(14.0))
                                .px(px(8.0))
                                .text_size(px(11.0))
                                .text_color(self.theme.fg_dim)
                                .child(format!(
                                    "{}  Focus navbar",
                                    self.snapshot
                                        .settings
                                        .keymap
                                        .binding("settings.focus_navbar")
                                        .map_or(String::new(), ToString::to_string)
                                )),
                        )
                    })
                    .when(self.compact, |nav| nav.pb(px(12.0))),
            )
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let file = self.configuration_file();
        self.titlebar("settings-content-titlebar")
            .h(px(76.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .rounded(px(4.0))
                    .px(px(7.0))
                    .py(px(4.0))
                    .text_size(px(14.0))
                    .text_color(self.theme.accent)
                    .bg(self.theme.accent_soft)
                    .child(if self.category == Category::Server {
                        "Current device"
                    } else {
                        "User"
                    }),
            )
            .child(
                div()
                    .id("settings-open-configuration")
                    .debug_selector(|| "settings-open-configuration".into())
                    .px(px(9.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(self.theme.border)
                    .text_size(px(13.0))
                    .text_color(self.theme.fg_muted)
                    .cursor_pointer()
                    .hover(|button| button.bg(self.theme.hover).text_color(self.theme.fg))
                    .focus(|button| button.border_color(self.theme.accent))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .button_interaction(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsEvent::OpenConfiguration(file));
                    }))
                    .child(format!("Edit in {file}")),
            )
            .into_any_element()
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.category == Category::Extensions
            && self.query.is_empty()
            && let Some(extensions) = &self.extensions
        {
            return div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .h_full()
                .px(px(32.0))
                .child(extensions.clone())
                .into_any_element();
        }
        let view = cx.entity();
        div()
            .relative()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .px(px(if self.compact { 20.0 } else { 32.0 }))
            .child(self.header(cx))
            .when_some(self.errors.get("application"), |body, error| {
                body.child(
                    div()
                        .debug_selector(|| "settings-application-error".into())
                        .child(self.note(error, true)),
                )
            })
            .when_some(self.errors.get("configuration"), |body, error| {
                body.child(self.note(error, true))
            })
            .child(
                div()
                    .id("settings-sections")
                    .debug_selector(|| "settings-sections".into())
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(
                        list(self.results.state.clone(), move |index, window, cx| {
                            view.update(cx, |view, cx| view.result(index, window, cx))
                        })
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "settings-scrollbar".into())
                    .absolute()
                    .right(px(3.0))
                    .top(px(76.0))
                    .bottom(px(8.0))
                    .child(
                        self.scrollbar
                            .element(&self.results.state, self.theme.fg_alpha(0.18)),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn layout_content(
        &mut self,
        size: gpui::Size<gpui::Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        for anchor in self.picker_anchors.values() {
            anchor.set(None);
        }
        self.compact = size.width < px(660.0);
        self.refresh_results(size.height, window, cx);
        div()
            .debug_selector(|| "settings-container".into())
            .size_full()
            .flex()
            .when(self.compact, Styled::flex_col)
            .child(self.navigation(cx))
            .child(self.content(cx))
            .into_any_element()
    }
}
