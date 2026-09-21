use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Styled, Window, div,
};

use super::menu::{self, Item, Menu};
use crate::model::AppModel;

pub(crate) enum Overlay {
    Updates,
    Webview,
    Native(Entity<super::native_modal::NativeModal>),
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
}

impl AppModel {
    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Webview)) {
            self.dismiss_webview_modal(cx);
            return;
        }
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
}

pub(crate) use muxy_ui::popover::clamp_to_viewport as clamp;

pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let content = match &model.overlay {
        None => return div().into_any_element(),
        Some(Overlay::Webview) => return super::webview::modal::render(model, window, cx),
        Some(Overlay::Updates) => {
            let content = super::updates::render(model, window, cx);
            let anchor = model.update_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Some(Overlay::Native(picker)) => picker.clone().into_any_element(),
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
