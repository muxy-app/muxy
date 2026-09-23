use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Styled, Window, div,
};

use super::menu::{self, Item, Menu};
use crate::model::AppModel;

pub(crate) enum Overlay {
    Updates,
    Server,
    Webview,
    /// The open extension popover (`AppModel::webviews.popover`).
    Popover,
    Native(Entity<super::native_modal::NativeModal>),
    Commands {
        palette: Entity<muxy_ui::command_palette::CommandPalette<super::command_palette::Handler>>,
        dark: bool,
    },
    Git(super::git::GitPicker),
    GitForm(super::git::Form),
    AiAction(super::git::AiSheet),
    PullRequest(super::git::PullRequestPopover),
    Sessions(super::session_picker::SessionPicker),
    Menu(Menu),
    ProjectEditor(super::project_editor::Editor),
    ProjectIcons(super::project_editor::icons::Icons),
    ProjectLogo(super::project_editor::logo::Cropper),
    ProjectColors(super::project_editor::Colors),
    Projects(Entity<super::project_picker::ProjectPicker>),
}

impl AppModel {
    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Webview)) {
            self.dismiss_webview_modal(cx);
            return;
        }
        if matches!(self.overlay, Some(Overlay::Popover)) {
            self.close_extension_popover(cx);
            return;
        }
        self.project_logo_task = None;
        self.git.interaction = self.git.interaction.wrapping_add(1);
        if let Some(Overlay::AiAction(sheet)) = self.overlay.take() {
            self.ai_sheet_dismissed(&sheet);
        }
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
        self.project_logo_task = None;
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Menu(Menu::new(items, position)));
        self.overlay_focus.focus(window);
        cx.notify();
    }
}

pub(crate) use muxy_ui::popover::clamp_to_viewport as clamp;

#[allow(
    clippy::too_many_lines,
    reason = "Exhaustive overlay rendering dispatch"
)]
pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let content = match &model.overlay {
        None => return div().into_any_element(),
        Some(Overlay::Webview) => return super::webview::modal::render(model, window, cx),
        Some(Overlay::Popover) => super::webview::popover::render(model, cx),
        Some(Overlay::Server) => {
            let content = super::server_status::render(model, window, cx);
            let anchor = model.server_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
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
        Some(Overlay::AiAction(sheet)) => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(super::git::render_sheet(sheet, model, window, cx))
            .into_any_element(),
        Some(Overlay::Git(picker)) => {
            let model = cx.entity().downgrade();
            let dismiss = move |_: &mut Window, cx: &mut gpui::App| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            };
            muxy_ui::popover::anchored_popover_above(
                picker.anchor.clone(),
                picker.picker.clone().into_any_element(),
                dismiss,
            )
        }
        Some(Overlay::PullRequest(popover)) => {
            let model_entity = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(
                model.git.pull_request_anchor.clone(),
                super::git::render_pr(popover, model, cx),
                move |_, cx| {
                    let _ = model_entity.update(cx, AppModel::dismiss_overlay);
                },
            )
        }
        Some(Overlay::Menu(menu)) => menu::render(menu, model, window, cx),
        Some(Overlay::ProjectEditor(editor)) => {
            super::project_editor::render(editor, model, window, cx)
        }
        Some(Overlay::ProjectIcons(picker)) => {
            super::project_editor::icons::render(picker, model, window, cx)
        }
        Some(Overlay::ProjectLogo(cropper)) => {
            super::project_editor::logo::render(cropper, model, window, cx)
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
