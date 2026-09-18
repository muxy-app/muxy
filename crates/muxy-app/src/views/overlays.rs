use gpui::{
    AnyElement, Bounds, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, Styled, Window, div, point, px, size,
};
use muxy_ui::components::SymbolGlyph;

use super::menu::{self, Item, Menu};
use crate::model::AppModel;

pub(crate) enum Overlay {
    Commands {
        palette: Entity<muxy_ui::command_palette::CommandPalette<super::command_palette::Handler>>,
        dark: bool,
    },
    Git(super::git::GitPicker),
    GitForm(super::git::Form),
    Sessions(super::session_picker::SessionPicker),
    Menu(Menu),
    ProjectEditor(super::project_editor::Editor),
    ProjectColors(super::project_editor::Colors),
    Projects(Entity<super::project_picker::ProjectPicker>),
    Notifications {
        anchor: Option<Bounds<Pixels>>,
    },
}

impl AppModel {
    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        self.git.interaction = self.git.interaction.wrapping_add(1);
        self.overlay = None;
        self.overlay_subscription = None;
        self.focus_requested = true;
        cx.notify();
    }

    pub(crate) fn open_menu(
        &mut self,
        items: Vec<Item>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Menu(Menu::new(items, position)));
        self.overlay_focus.focus(window);
        cx.notify();
    }

    pub(crate) fn toggle_notifications(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Notifications { .. })) {
            self.dismiss_overlay(cx);
            return;
        }
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Notifications {
            anchor: self.notification_anchor,
        });
        self.overlay_focus.focus(window);
        cx.notify();
    }
}

pub(crate) use muxy_ui::popover::clamp_to_viewport as clamp;

pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let content = match &model.overlay {
        None => return div().into_any_element(),
        Some(Overlay::GitForm(form)) => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(super::git::render_form(form, model, window, cx))
            .into_any_element(),
        Some(Overlay::Git(picker)) => {
            let model = cx.entity().downgrade();
            let dismiss = move |_: &mut Window, cx: &mut gpui::App| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            };
            if picker.kind == super::git::Kind::Worktrees {
                muxy_ui::popover::anchored_popover(
                    picker.anchor.clone(),
                    picker.picker.clone().into_any_element(),
                    dismiss,
                )
            } else {
                muxy_ui::popover::anchored_popover_above(
                    picker.anchor.clone(),
                    picker.picker.clone().into_any_element(),
                    dismiss,
                )
            }
        }
        Some(Overlay::Menu(menu)) => menu::render(menu, model, window, cx),
        Some(Overlay::ProjectEditor(editor)) => {
            super::project_editor::render(editor, model, window, cx)
        }
        Some(Overlay::ProjectColors(colors)) => {
            super::project_editor::render_colors(colors, model, window, cx)
        }
        Some(Overlay::Sessions(picker)) => picker.picker.clone().into_any_element(),
        Some(Overlay::Projects(picker)) => picker.clone().into_any_element(),
        Some(Overlay::Commands { palette, .. }) => palette.clone().into_any_element(),
        Some(Overlay::Notifications { anchor }) => notifications(*anchor, model, window, cx),
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|model, _, _, cx| {
                        model.dismiss_overlay(cx);
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|model, _, _, cx| {
                        model.dismiss_overlay(cx);
                        cx.stop_propagation();
                    }),
                ),
        )
        .child(content)
        .into_any_element()
}

fn notifications(
    anchor: Option<Bounds<Pixels>>,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let viewport = window.viewport_size();
    let height = px(400.0).min(viewport.height - px(16.0));
    let origin = anchor.map_or(
        point(px(8.0), viewport.height - height - px(8.0)),
        |anchor| point(anchor.origin.x, anchor.origin.y - height - px(4.0)),
    );
    let origin = clamp(origin, size(px(320.0), height), viewport);
    div()
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(cx.listener(|model, _: &menu::DismissMenu, _, cx| model.dismiss_overlay(cx)))
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(px(320.0))
        .h(height)
        .flex()
        .flex_col()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .rounded(m.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
        .child(
            div()
                .px(m.spacing5())
                .py(m.spacing4())
                .border_b_1()
                .border_color(theme.border)
                .text_size(m.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .child("Notifications"),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(m.spacing4())
                .child(SymbolGlyph::new("bell.slash", m.icon_xl(), theme.fg_dim))
                .child(
                    div()
                        .text_size(m.font_body())
                        .text_color(theme.fg_muted)
                        .child("No notifications"),
                ),
        )
        .into_any_element()
}
