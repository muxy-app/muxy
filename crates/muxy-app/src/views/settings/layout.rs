use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, list, px,
};
use muxy_ui::accessibility;
use muxy_ui::components::{ButtonInteraction, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::{controls, navigation, tr};

use super::{Category, SettingsEvent, SettingsView};

impl Category {
    fn sections(self) -> &'static [&'static str] {
        match self {
            Self::QuickTerminal => &[
                tr_key!("General"),
                tr_key!("Shortcut"),
                tr_key!("Size"),
                tr_key!("Appearance"),
            ],
            Self::Composer => &[tr_key!("Behavior"), tr_key!("Text")],
            Self::Browser => &[tr_key!("General"), tr_key!("Browsing"), tr_key!("Profiles")],
            Self::Ai => &[tr_key!("Commit"), tr_key!("Create Pull Request")],
            Self::General => &[
                tr_key!("Closing terminals"),
                tr_key!("Files"),
                tr_key!("Window size"),
                tr_key!("Worktrees"),
                tr_key!("Privacy"),
            ],
            Self::Appearance => &[
                tr_key!("Language"),
                tr_key!("Themes"),
                tr_key!("Interface"),
                tr_key!("Agents Panel"),
            ],
            Self::Keyboard => &[tr_key!("Shortcuts")],
            Self::Terminal => super::terminal::SECTIONS,
            Self::Servers | Self::Extensions | Self::Commands | Self::About => &[],
            Self::Backup => &[
                tr_key!("Backup & Restore"),
                tr_key!("Migration"),
                tr_key!("Configuration"),
            ],
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
        self.finish_slider(cx);
        self.recording = None;
        self.cancel_quick_recording(cx);
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
        if category == Category::Servers {
            cx.emit(SettingsEvent::ReadServer);
        }
        if category == Category::Extensions {
            cx.emit(SettingsEvent::CheckExtensionUpdates);
        }
        cx.notify();
    }

    fn disclosure(&self, category: Category, cx: &mut Context<Self>) -> AnyElement {
        navigation::disclosure(
            self.layout_style(),
            SharedString::from(format!("settings-disclosure-{}", category.label())),
            self.expanded.contains(&category),
            cx.listener(move |view, _, _, cx| view.toggle_sections(category, cx)),
        )
        .debug_selector(move || format!("settings-disclosure-{}", category.label()))
        .into_any_element()
    }

    fn toggle_sections(&mut self, category: Category, cx: &mut Context<Self>) {
        if !self.expanded.remove(&category) {
            self.expanded.insert(category);
        }
        cx.notify();
    }

    fn categories(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut categories = div()
            .id("settings-category-tabs")
            .role(Role::TabList)
            .aria_label(tr!("Settings"))
            .flex()
            .gap(self.layout_metrics.scaled(2.0))
            .map(|nav| {
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
                    translate(category.label()),
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
                .child(translate(category.label())),
            );
            if !self.compact && !category.sections().is_empty() {
                let view = cx.weak_entity();
                categories = categories.child(
                    accessibility::hidden_button(
                        format!("settings-disclosure-{}", category.label()),
                        tr!("Sections of %@", &translate(category.label())),
                        move |_, _, cx| {
                            let _ = view.update(cx, |view, cx| view.toggle_sections(category, cx));
                        },
                    )
                    .aria_expanded(expanded),
                );
            }
            if expanded && !self.compact {
                for &section in category.sections() {
                    if category == Category::Terminal
                        && section == tr_key!("Migration")
                        && self.snapshot.terminal.diagnostics.is_empty()
                    {
                        continue;
                    }
                    categories = categories.child(
                        navigation::subitem(
                            self.layout_style(),
                            // The same section can sit under two open categories.
                            SharedString::from(format!(
                                "settings-subcategory-{}-{section}",
                                category.label()
                            )),
                            translate(section),
                            selected && self.section == Some(section),
                            cx.listener(move |view, _, window, cx| {
                                view.select_category(category, Some(section), window, cx);
                            }),
                        )
                        .debug_selector(move || format!("settings-subcategory-{section}"))
                        .child(translate(section)),
                    );
                }
            }
        }
        categories.into_any_element()
    }

    fn header_background(&self, id: &'static str) -> gpui::Stateful<gpui::Div> {
        let focus = self.focus.clone();
        div()
            .id(id)
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
                focus.focus(window);
            })
    }

    pub(super) fn navigation(&self, _: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .debug_selector(|| "settings-navigation".into())
            .flex_none()
            .map(|nav| {
                if self.compact {
                    nav.w_full().border_b_1()
                } else {
                    nav.w(self.layout_metrics.scaled(248.0))
                        .h_full()
                        .border_r_1()
                }
            })
            .flex()
            .flex_col()
            .bg(self.theme.sunken())
            .border_color(self.theme.border)
            .child(
                self.header_background("settings-sidebar-titlebar")
                    .h(self.layout_metrics.scaled(12.0))
                    .flex_none(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_h(px(0.0))
                    .flex_1()
                    .px(self.layout_metrics.scaled(12.0))
                    .gap(self.layout_metrics.scaled(12.0))
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
                                .py(self.layout_metrics.scaled(14.0))
                                .px(self.layout_metrics.scaled(8.0))
                                .text_size(self.layout_metrics.scaled(11.0))
                                .text_color(self.theme.fg_dim)
                                .child(tr!(
                                    "%@  Focus navbar",
                                    self.snapshot
                                        .settings
                                        .keymap
                                        .binding("settings.focus_navbar")
                                        .map_or(String::new(), ToString::to_string)
                                )),
                        )
                    })
                    .when(self.compact, |nav| nav.pb(self.layout_metrics.scaled(12.0))),
            )
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let tour = self.header_button(
            "settings-welcome-tour",
            tr!("Welcome Tour"),
            cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::ShowWelcomeTour)),
        );
        let agent = self.header_button(
            "settings-ask-agent",
            tr!("Ask Agent…"),
            cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::AskAgent)),
        );
        if !self.results.editable {
            return div()
                .h(self.layout_metrics.scaled(56.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .gap(self.layout_metrics.scaled(8.0))
                .child(agent)
                .child(tour)
                .into_any_element();
        }
        let file = self.configuration_file();
        self.header_background("settings-content-titlebar")
            .h(self.layout_metrics.scaled(56.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(self.layout_metrics.scaled(12.0))
            .child(
                div()
                    .rounded(self.layout_metrics.scaled(4.0))
                    .px(self.layout_metrics.scaled(7.0))
                    .py(self.layout_metrics.scaled(4.0))
                    .text_size(self.layout_metrics.scaled(14.0))
                    .text_color(self.theme.accent)
                    .bg(self.theme.accent_soft)
                    .child(if self.category == Category::Servers {
                        if cfg!(target_os = "macos") {
                            tr!("This Mac")
                        } else {
                            tr!("This Computer")
                        }
                    } else {
                        tr!("User")
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(self.layout_metrics.scaled(8.0))
                    .child(agent)
                    .child(tour)
                    .child(self.header_button(
                        "settings-open-configuration",
                        tr!("Edit in %@", file),
                        cx.listener(move |_, _, _, cx| {
                            cx.emit(SettingsEvent::OpenConfiguration(file));
                        }),
                    )),
            )
            .into_any_element()
    }

    fn header_button(
        &self,
        id: &'static str,
        label: SharedString,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .role(Role::Button)
            .accessibility_id(id)
            .aria_label(label.clone())
            .debug_selector(move || id.into())
            .px(self.layout_metrics.scaled(9.0))
            .py(self.layout_metrics.scaled(4.0))
            .rounded(self.layout_metrics.scaled(4.0))
            .border_1()
            .border_color(self.theme.border)
            .text_size(self.layout_metrics.scaled(13.0))
            .text_color(self.theme.fg_muted)
            .cursor_pointer()
            .hover(|button| button.bg(self.theme.hover).text_color(self.theme.fg))
            .focus(|button| button.border_color(self.theme.accent))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .button_interaction(on_click)
            .child(label)
    }

    fn dismissible_error(
        &self,
        id: &'static str,
        error: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .debug_selector(move || format!("settings-{id}-error"))
            .flex()
            .items_center()
            .gap(self.layout_metrics.scaled(8.0))
            .child(accessibility::announced(
                self.note(error, true)
                    .flex_1()
                    .min_w(px(0.0))
                    .id(SharedString::from(format!("settings-{id}-error"))),
                error.to_owned(),
                true,
            ))
            .child(
                div()
                    .id(SharedString::from(format!("settings-dismiss-{id}-error")))
                    .role(Role::Button)
                    .accessibility_id(format!("settings-dismiss-{id}-error"))
                    .aria_label(tr!("Dismiss"))
                    .debug_selector(move || format!("settings-dismiss-{id}-error"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(self.metrics.control_small())
                    .rounded(self.metrics.radius_sm())
                    .cursor_pointer()
                    .hover(|button| button.bg(self.theme.hover))
                    .focus(|button| button.bg(self.theme.hover))
                    .tooltip(muxy_ui::components::tooltip(tr!("Dismiss"), &self.theme))
                    .button_interaction(cx.listener(move |view, _, _, cx| {
                        view.dismiss_error(id, cx);
                    }))
                    .child(IconGlyph::new(
                        Icon::X,
                        self.metrics.icon_sm(),
                        self.theme.fg_muted,
                    )),
            )
            .into_any_element()
    }

    pub(super) fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        for anchor in self.picker_anchors.values() {
            anchor.set(None);
        }
        if self.category == Category::Extensions
            && self.query.is_empty()
            && let Some(extensions) = &self.extensions
        {
            return div()
                .w_full()
                .min_w_0()
                .min_h_0()
                .h_full()
                .px(self
                    .layout_metrics
                    .scaled(if self.compact { 20.0 } else { 32.0 }))
                .when(cfg!(target_os = "linux"), |content| {
                    content.pt(self.layout_metrics.scaled(32.0))
                })
                .child(extensions.clone())
                .into_any_element();
        }
        let view = cx.entity();
        div()
            .relative()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .px(self
                .layout_metrics
                .scaled(if self.compact { 20.0 } else { 32.0 }))
            .child(self.header(cx))
            .children(
                ["application", "configuration"]
                    .into_iter()
                    .filter_map(|id| {
                        self.errors
                            .get(id)
                            .map(|error| self.dismissible_error(id, error, cx))
                    }),
            )
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
                    .top(self.layout_metrics.scaled(76.0))
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
        self.compact = size.width < self.layout_metrics.scaled(660.0);
        self.refresh_results(size.height, window, cx);
        div()
            .debug_selector(|| "settings-container".into())
            .size_full()
            .flex()
            .when(self.compact, Styled::flex_col)
            .child(if self.compact {
                self.navigation(window, cx)
            } else {
                gpui::AnyView::from(self.navigation_view.clone())
                    .cached(
                        div()
                            .w(self.layout_metrics.scaled(248.0))
                            .h_full()
                            .flex_none()
                            .style()
                            .clone(),
                    )
                    .into_any_element()
            })
            .child(
                gpui::AnyView::from(self.content_view.clone()).cached(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .h_full()
                        .style()
                        .clone(),
                ),
            )
            .into_any_element()
    }
}
