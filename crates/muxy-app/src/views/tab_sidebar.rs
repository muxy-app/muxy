use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_app_core::{Project, ProjectId, ProjectStatus, Tab, TabId, settings::AppLayout};
use muxy_ui::components::{IconButton, IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;

use super::{tab_activity, tab_strip, titlebar};
use crate::model::AppModel;

impl AppModel {
    pub(crate) fn set_layout(&mut self, layout: AppLayout, cx: &mut Context<Self>) {
        if self.appearance.layout == layout {
            return;
        }
        self.cancel_titlebar_drag(cx);
        self.appearance.layout = layout;
        self.tab_sidebar_selection = None;
        self.save_appearance(cx);
        self.dismiss_overlay(cx);
        cx.notify();
    }

    pub(super) fn project_expanded(&self, id: ProjectId) -> bool {
        self.appearance
            .tab_focused_expanded
            .get(&id)
            .copied()
            .unwrap_or_else(|| {
                let active = self.state.current_project();
                active.id == id
            })
    }

    fn toggle_project_tabs(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        let expanded = !self.project_expanded(id);
        self.appearance.tab_focused_expanded.insert(id, expanded);
        self.cancel_titlebar_drag(cx);
        self.save_appearance(cx);
        cx.notify();
    }

    pub(crate) fn sync_tab_sidebar(&mut self, cx: &mut Context<Self>) {
        let selected = (self.state.current_project().id, self.active_tab());
        let previous = self.tab_sidebar_selection.replace(selected);
        if previous == Some(selected) {
            return;
        }
        let project = self.state.current_project();
        let id = project.id;
        let parent = project.parent_id.unwrap_or(id);
        let switched = previous.is_none_or(|previous| previous.0 != id);
        let mut changed = false;
        if switched {
            self.appearance
                .worktree_recent
                .retain(|candidate| *candidate != id && self.state.project(*candidate).is_some());
            self.appearance.worktree_recent.insert(0, id);
            self.expanded_worktrees
                .retain(|id| self.state.project(*id).is_some());
            if self.appearance.auto_expand_worktrees {
                self.expanded_worktrees.insert(parent);
            }
            changed = true;
        }
        if self.appearance.layout == AppLayout::TabFocused
            && (!self.appearance.tab_focused_expanded.contains_key(&id)
                || (previous.is_some() && !self.project_expanded(id)))
        {
            self.appearance.tab_focused_expanded.insert(id, true);
            changed = true;
        }
        if changed {
            self.save_appearance(cx);
        }
    }

    pub(crate) fn navigation_tabs(&self) -> Vec<TabId> {
        if self.appearance.layout == AppLayout::ProjectFocused {
            return self
                .state
                .current_project()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect();
        }
        self.sidebar_projects()
            .into_iter()
            .filter(|project| {
                project.status() == ProjectStatus::Available && self.project_expanded(project.id)
            })
            .flat_map(|project| project.tabs.iter().map(|tab| tab.id))
            .collect()
    }
}

pub(super) fn contents(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let targets = tab_strip::drag::TabBounds::default();
    let numbers = model.navigation_tabs();
    let mut rows = div()
        .flex()
        .flex_col()
        .pt(model.metrics.spacing5())
        .pb(model.metrics.spacing3());
    for project in model.sidebar_projects() {
        rows = rows.child(project_group(
            project,
            &numbers,
            targets.clone(),
            window.modifiers(),
            model,
            cx,
        ));
    }
    if !model.appearance.sidebar_focus {
        rows = rows.child(add_project_button(model, cx));
    }
    div()
        .debug_selector(|| "tab-sidebar".into())
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .child(
            div()
                .id("tab-sidebar-scroll")
                .debug_selector(|| "tab-sidebar-scroll".into())
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(rows),
        )
        .child(tab_strip::drag::track_pointer(targets, cx))
        .into_any_element()
}

fn add_project_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id("add-project")
        .debug_selector(|| "tab-add-project".into())
        .mx(m.spacing3())
        .my(m.spacing1())
        .px(m.spacing3())
        .h(m.control_large())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .rounded(m.radius_lg())
        .cursor_pointer()
        .text_size(m.font_headline())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
        .hover(|style| style.bg(theme.hover).text_color(theme.fg))
        .on_click(cx.listener(|model, _, window, cx| model.open_project_picker(window, cx)))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.control_small())
                .child(IconGlyph::new(
                    Icon::Plus,
                    m.font_headline(),
                    theme.fg_muted,
                )),
        )
        .child("Add Project")
        .into_any_element()
}

fn project_group(
    project: &Project,
    numbers: &[TabId],
    targets: tab_strip::drag::TabBounds,
    modifiers: gpui::Modifiers,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let missing = project.status() == ProjectStatus::Missing;
    let expanded = model.project_expanded(id);
    let mut group = div()
        .flex()
        .flex_col()
        .child(project_header(project, model, cx));
    if expanded && !missing {
        let ids: Vec<_> = project.tabs.iter().map(|tab| tab.id).collect();
        let measured = ids.clone();
        group = group.child(
            div()
                .flex()
                .flex_col()
                .children(project.tabs.iter().map(|tab| {
                    let number = numbers
                        .iter()
                        .position(|id| *id == tab.id)
                        .filter(|index| *index < 9);
                    tab_row(project.id, tab, number, modifiers, model, cx)
                }))
                .on_children_prepainted(move |bounds, window, _| {
                    let clip = window.content_mask().bounds;
                    let mut targets = targets.borrow_mut();
                    targets.retain(|(id, _)| !measured.contains(id));
                    targets.extend(
                        measured
                            .iter()
                            .zip(bounds)
                            .map(|(id, bounds)| (*id, bounds.intersect(&clip))),
                    );
                }),
        );
    }
    group.into_any_element()
}

fn project_header(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let theme = &model.theme;
    let missing = project.status() == ProjectStatus::Missing;
    let active = model.state.current_project().id == id;
    let m = model.metrics;
    let group = SharedString::from(format!("tab-project-{id}"));
    div()
        .id(group.clone())
        .group(group.clone())
        .relative()
        .debug_selector(move || format!("tab-project-{id}"))
        .mx(m.spacing3())
        .my(m.spacing1())
        .px(m.spacing3())
        .h(m.control_large())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .rounded(m.radius_lg())
        .cursor_pointer()
        .when(missing, |row| row.opacity(0.5))
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(move |model, _, window, cx| {
            if !missing {
                model.select_project(id, cx);
                if !model.project_expanded(id) {
                    model.toggle_project_tabs(id, cx);
                }
                model.focus_active(window, cx);
            }
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(
                        super::project_menu::items(project),
                        event.position,
                        window,
                        cx,
                    );
                }
            }),
        )
        .child(project_disclosure(project, model, cx))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(m.font_headline())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if active { theme.fg } else { theme.fg_muted })
                .when(model.appearance.sidebar_focus, |label| {
                    label.pr(m.control_small() * 2.0)
                })
                .group_hover(group.clone(), |style| {
                    style.text_color(theme.fg).pr(m.control_small() * 2.0)
                })
                .child(project.name.clone()),
        )
        .child(project_accessory(project, group, model, cx))
        .into_any_element()
}

fn project_disclosure(
    project: &Project,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let expanded = model.project_expanded(id);
    let missing = project.status() == ProjectStatus::Missing;
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("tab-project-toggle-{id}"))
        .flex_none()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                SharedString::from(format!("tab-project-toggle-{id}")),
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                m.icon_xs(),
                m.control_small(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(
                if expanded {
                    "Collapse Project"
                } else {
                    "Expand Project"
                },
                theme.raised(),
                theme.fg,
                theme.border,
            )
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                if !missing {
                    model.toggle_project_tabs(id, cx);
                    model.focus_active(window, cx);
                }
            })),
        )
        .into_any_element()
}

fn project_accessory(
    project: &Project,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let activity = tab_activity::project_status(id, model);
    let has_activity = activity != tab_activity::Status::None;
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .h(m.control_small())
        .child(
            div()
                .absolute()
                .right_full()
                .top_0()
                .opacity(if model.appearance.sidebar_focus {
                    1.0
                } else {
                    0.0
                })
                .group_hover(group, |style| style.opacity(1.0))
                .child(project_controls(project, model, cx)),
        )
        .when(project.parent_id.is_some() && !has_activity, |row| {
            row.child(IconGlyph::new(
                Icon::GitBranch,
                m.icon_sm(),
                model.theme.fg_muted,
            ))
        })
        .when(has_activity, |row| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .min_w(m.control_small())
                    .h(m.control_small())
                    .child(tab_activity::status_glyph(
                        format!("project-activity-{id}"),
                        activity,
                        m.icon_sm(),
                        model,
                    )),
            )
        })
        .into_any_element()
}

fn project_controls(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let theme = &model.theme;
    let missing = project.status() == ProjectStatus::Missing;
    div()
        .flex()
        .flex_none()
        .when(
            (!missing || model.appearance.sidebar_focus) && project.parent_id.is_none(),
            |row| {
                row.child(
                    IconButton::new(
                        SharedString::from(format!("focus-project-{id}")),
                        Icon::Eye,
                        model.metrics.icon_sm(),
                        model.metrics.control_small(),
                        theme.fg_muted,
                        theme.fg,
                    )
                    .tooltip(
                        if model.appearance.sidebar_focus {
                            "Show All Projects"
                        } else {
                            "Focus Project"
                        },
                        theme.raised(),
                        theme.fg,
                        theme.border,
                    )
                    .on_click(cx.listener(move |model, _, window, cx| {
                        cx.stop_propagation();
                        let focus = !model.appearance.sidebar_focus;
                        if focus {
                            let selected = model.preferred_worktree(id);
                            model.select_project(selected, cx);
                            model.appearance.tab_focused_expanded.insert(selected, true);
                        }
                        model.appearance.sidebar_focus = focus;
                        model.save_appearance(cx);
                        model.focus_active(window, cx);
                        cx.notify();
                    })),
                )
            },
        )
        .when(!missing, |row| {
            row.child(
                div()
                    .debug_selector(move || format!("project-new-tab-{id}"))
                    .child(
                        IconButton::new(
                            SharedString::from(format!("project-new-tab-{id}")),
                            Icon::Plus,
                            model.metrics.icon_sm(),
                            model.metrics.control_small(),
                            theme.fg_muted,
                            theme.fg,
                        )
                        .tooltip("New Terminal Tab", theme.raised(), theme.fg, theme.border)
                        .on_click(cx.listener(
                            move |model, _, window, cx| {
                                cx.stop_propagation();
                                model.select_project(id, cx);
                                model.new_tab(cx);
                                model.focus_active(window, cx);
                            },
                        )),
                    ),
            )
        })
        .into_any_element()
}

fn tab_row(
    project: ProjectId,
    tab: &Tab,
    number: Option<usize>,
    modifiers: gpui::Modifiers,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = tab.id;
    let m = model.metrics;
    let theme = &model.theme;
    let active = model.active_tab() == Some(id) && model.state.current_project().id == project;
    let bell = tab.panes.iter().any(|pane| {
        model
            .terminal(&pane.id)
            .is_some_and(|pane| pane.view.read(cx).bell_flashing)
    });
    let title = model.webview_title(tab, cx);
    let color = tab
        .color
        .as_ref()
        .and_then(|color| muxy_ui::theme::parse_hex(color.as_str()))
        .map(gpui::Hsla::from);
    let group = SharedString::from(format!("sidebar-tab-{id}"));
    let hover = color.map_or(if active { theme.surface } else { theme.hover }, |color| {
        color.opacity(if active { 0.18 } else { 0.08 })
    });
    div()
        .id(group.clone())
        .debug_selector(move || format!("sidebar-tab-{id}"))
        .when(!model.tab_drag.is_active(), |row| row.group(group.clone()))
        .mx(m.spacing3())
        .my(m.spacing1())
        .pl(m.control_small() + m.spacing3() * 2.0)
        .pr(m.spacing3())
        .h(m.control_large())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .rounded(m.radius_lg())
        .cursor_pointer()
        .text_size(m.font_headline())
        .text_color(if active { theme.fg } else { theme.fg_muted })
        .when(active, |row| row.bg(theme.surface))
        .when_some(color, |row, color| {
            row.bg(color.opacity(if active { 0.18 } else { 0.04 }))
        })
        .when(!model.tab_drag.is_active(), |row| {
            row.hover(|style| style.bg(hover))
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.open_tab_menu(id, event.position, window, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.select_tab(id, cx);
                model.tab_drag.begin(project, id, event.position);
                model.focus_active(window, cx);
            }),
        )
        .on_click(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.close_tab(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-icon-{id}"))
                .size(m.icon_md())
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(tab_activity::icon(
                    tab,
                    model,
                    m.icon_md(),
                    webview_tab_icon(tab, model, active, cx),
                )),
        )
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-title-{id}"))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(title.to_owned()),
        )
        .child(tab_accessory(
            tab,
            number.and_then(|index| tab_shortcut(index, &model.settings.keymap, modifiers)),
            bell,
            group,
            model,
            cx,
        ))
        .into_any_element()
}

pub(super) fn titlebar(
    model: &AppModel,
    sidebar_width: f32,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let project = model.state.current_project();
    let available = project.status() == ProjectStatus::Available;
    let count = model.existing_terminal_count();
    let tab = project
        .tabs
        .iter()
        .find(|tab| Some(tab.id) == model.active_tab());
    titlebar::background("project-titlebar")
        .flex()
        .items_center()
        .flex_none()
        .h(px(32.0))
        .pl(px((titlebar::navigation_width(model) - sidebar_width)
            .max(0.0)
            + 12.0))
        .child(
            div()
                .debug_selector(|| "project-title".into())
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(project.name.clone()),
        )
        .when(available, |bar| {
            bar.child(
                div()
                    .debug_selector(|| "new-tab-button".into())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new(
                            "new-tab",
                            Icon::Plus,
                            px(13.0),
                            px(24.0),
                            theme.fg_muted,
                            theme.fg,
                        )
                        .tooltip("New Terminal Tab", theme.raised(), theme.fg, theme.border)
                        .on_click(cx.listener(|model, _, _, cx| {
                            cx.stop_propagation();
                            model.new_tab(cx);
                        })),
                    ),
            )
        })
        .when(available && count > 0, |bar| {
            bar.child(tab_strip::existing_terminals_button(count, model, cx))
        })
        .when_some(
            tab.filter(|tab| tab.zoomed.is_some() || tab.panes.len() > 1),
            |bar, tab| bar.child(tab_strip::zoom_control(tab.zoomed.is_some(), model, cx)),
        )
        .child(model.extension_toolbar(cx))
        .child(tab_strip::settings_button(model, cx))
        .into_any_element()
}

fn tab_shortcut(
    index: usize,
    keymap: &muxy_app_core::settings::Keymap,
    modifiers: gpui::Modifiers,
) -> Option<String> {
    use muxy_core::shortcuts::ShortcutId;
    let id = [
        ShortcutId::SelectTab1,
        ShortcutId::SelectTab2,
        ShortcutId::SelectTab3,
        ShortcutId::SelectTab4,
        ShortcutId::SelectTab5,
        ShortcutId::SelectTab6,
        ShortcutId::SelectTab7,
        ShortcutId::SelectTab8,
        ShortcutId::SelectTab9,
    ]
    .get(index)?;
    let chord = keymap.chord(*id)?;
    let key = gpui::Keystroke::parse(chord.as_str()).ok()?;
    if modifiers == gpui::Modifiers::default() || key.modifiers != modifiers {
        return None;
    }
    Some(match key.key.as_str() {
        "enter" => "↩".into(),
        "escape" => "⎋".into(),
        "tab" => "⇥".into(),
        "space" => "␣".into(),
        "backspace" => "⌫".into(),
        "delete" => "⌦".into(),
        "insert" => "Ins".into(),
        "home" => "↖".into(),
        "end" => "↘".into(),
        "pageup" => "⇞".into(),
        "pagedown" => "⇟".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        key => key.to_uppercase(),
    })
}

fn close_tab_button(
    id: TabId,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    div()
        .id(SharedString::from(format!("sidebar-close-{id}")))
        .debug_selector(move || format!("sidebar-close-{id}"))
        .size(model.metrics.control_small())
        .flex()
        .items_center()
        .justify_center()
        .rounded(model.metrics.radius_sm())
        .absolute()
        .inset_0()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.close_tab(id, cx);
            model.focus_active(window, cx);
        }))
        .child(
            div()
                .id(SharedString::from(format!("sidebar-close-glyph-{id}")))
                .size(model.metrics.icon_md())
                .flex()
                .items_center()
                .justify_center()
                .rounded(model.metrics.radius_sm())
                .hover(|style| style.bg(theme.hover))
                .child(IconGlyph::new(
                    Icon::X,
                    model.metrics.icon_xs(),
                    theme.fg_muted,
                )),
        )
        .into_any_element()
}

impl AppModel {
    pub(crate) fn set_project_focus(&mut self, focused: bool, cx: &mut Context<Self>) {
        self.appearance.sidebar_focus = focused;
        if focused {
            let project = self.state.current_project();
            self.appearance
                .tab_focused_expanded
                .insert(project.id, true);
        }
        self.save_appearance(cx);
        cx.notify();
    }
}

fn tab_accessory(
    tab: &Tab,
    shortcut: Option<String>,
    bell: bool,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = tab.id;
    let m = model.metrics;
    let status = tab_activity::tab_status(tab, model);
    let content = if tab.pinned {
        div()
            .debug_selector(move || format!("sidebar-tab-pin-{id}"))
            .child(SymbolGlyph::new(
                "pin.fill",
                m.font_xs(),
                model.theme.fg_muted,
            ))
            .into_any_element()
    } else if let Some(shortcut) = shortcut {
        div()
            .debug_selector(move || format!("sidebar-tab-shortcut-{id}"))
            .text_size(m.font_caption())
            .text_color(model.theme.fg_muted)
            .child(shortcut)
            .into_any_element()
    } else if status != tab_activity::Status::None {
        tab_activity::status_glyph(format!("tab-{id}"), status, m.icon_sm(), model)
    } else if bell {
        div()
            .size(m.scaled(7.0))
            .rounded_full()
            .bg(model.theme.warning)
            .into_any_element()
    } else {
        div().into_any_element()
    };
    div()
        .relative()
        .flex_none()
        .size(m.control_small())
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-accessory-{id}"))
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .when(!tab.pinned, |slot| {
                    slot.group_hover(group.clone(), |style| style.opacity(0.0))
                })
                .child(content),
        )
        .when(!tab.pinned, |slot| {
            slot.child(close_tab_button(id, group, model, cx))
        })
        .into_any_element()
}

fn webview_tab_icon(tab: &Tab, model: &AppModel, active: bool, cx: &gpui::App) -> AnyElement {
    let color = if active {
        model.theme.fg
    } else {
        model.theme.fg_muted
    };
    model
        .webview_glyph(tab, model.metrics.font_footnote(), color, cx)
        .unwrap_or_else(|| {
            SymbolGlyph::new("terminal", model.metrics.font_footnote(), color).into_any_element()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_app_core::settings::Keymap;
    use muxy_core::shortcuts::ShortcutId;

    #[test]
    fn tab_hints_follow_custom_keys_and_only_appear_for_matching_modifiers() {
        let control = gpui::Modifiers {
            control: true,
            ..Default::default()
        };
        for (chord, label) in [("ctrl-x", "X"), ("ctrl-enter", "↩"), ("ctrl-f12", "F12")] {
            let keymap = Keymap::default()
                .with_binding(
                    ShortcutId::SelectTab1.name(),
                    Some(chord.parse().expect("chord")),
                )
                .expect("custom binding");
            assert_eq!(tab_shortcut(0, &keymap, control).as_deref(), Some(label));
            assert_eq!(tab_shortcut(0, &keymap, gpui::Modifiers::default()), None);
            assert_eq!(
                tab_shortcut(
                    0,
                    &keymap,
                    gpui::Modifiers {
                        shift: true,
                        ..control
                    }
                ),
                None
            );
        }
    }
}
