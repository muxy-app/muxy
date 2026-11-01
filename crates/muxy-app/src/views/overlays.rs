use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Styled, Window, div,
};

use super::menu::{self, Item, Menu};
use crate::model::AppModel;
use crate::model::tips::TipPlacement;

pub(crate) enum Overlay {
    Updates,
    Server,
    /// The tip popover beside the collapsed sidebar's tip button.
    Tip,
    Webview,
    /// The open extension popover (`AppModel::webviews.popover`).
    Popover,
    Native(Entity<super::native_modal::NativeModal>),
    Commands {
        palette: Entity<muxy_ui::command_palette::CommandPalette<super::command_palette::Handler>>,
        dark: bool,
    },
    Git(super::git::GitPicker),
    GitForm(Box<super::git::Form>),
    AiProvider(super::git::AiProviderMenu),
    PullRequest(super::git::PullRequestPopover),
    Sessions(super::session_picker::SessionPicker),
    Worktrees(super::worktree_picker::WorktreePicker),
    Layouts(crate::model::project_layouts::LayoutPicker),
    Menu(Menu),
    ProjectEditor(super::project_editor::Editor),
    ProjectIcons(super::project_editor::icons::Icons),
    ProjectLogo(super::project_editor::logo::Cropper),
    Projects(Entity<super::project_picker::ProjectPicker>),
    /// The popover above the sidebar's Remote section.
    RemoteServers,
    Agents,
    SessionProviders,
    ServerForm(Box<super::remote_servers::ServerForm>),
    PairForm(Box<super::pair_form::PairForm>),
    Password(Box<super::remote_servers::PasswordPrompt>),
    /// The status bar's proxy popover.
    Proxy(Box<super::proxy::ProxyForm>),
}

impl Overlay {
    /// The mark of the area the overlay covers, so native views elsewhere
    /// stay visible while it is open. Modal dialogs dim the whole window, so
    /// they have none. Rendering and [`AppModel::coverage`] both read it.
    pub(crate) fn mark(&self, status_bar_visible: bool) -> Option<Mark> {
        let marks = match self {
            Self::Proxy(_) => status_bar_visible,
            Self::Updates
            | Self::Server
            | Self::Tip
            | Self::RemoteServers
            | Self::Agents
            | Self::SessionProviders
            | Self::Git(_)
            | Self::AiProvider(_)
            | Self::PullRequest(_)
            | Self::Menu(_)
            | Self::ProjectEditor(_)
            | Self::ProjectIcons(_) => true,
            Self::Webview
            | Self::Popover
            | Self::Native(_)
            | Self::Commands { .. }
            | Self::GitForm(_)
            | Self::Sessions(_)
            | Self::Worktrees(_)
            | Self::Layouts(_)
            | Self::ProjectLogo(_)
            | Self::Projects(_)
            | Self::ServerForm(_)
            | Self::PairForm(_)
            | Self::Password(_) => false,
        };
        marks.then_some(Mark(()))
    }
}

/// Lets an overlay mark the area it covers. Only [`Overlay::mark`] makes one.
#[derive(Clone, Copy)]
pub(crate) struct Mark(());

#[allow(
    clippy::unused_self,
    reason = "Taking the mark is what lets a view mark its bounds"
)]
impl Mark {
    /// Marks the bounds of the element it is placed in.
    pub(crate) fn of(self, model: &AppModel) -> AnyElement {
        model.webview_occlusion()
    }

    /// Like [`Mark::of`], leaving `owner`'s web view uncut.
    pub(crate) fn of_except(self, model: &AppModel, owner: gpui::EntityId) -> AnyElement {
        model.webview_occlusion_except(Some(owner))
    }
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
        if matches!(self.overlay, Some(Overlay::ServerForm(_))) {
            self.clear_server_probe();
        }
        self.git.interaction = self.git.interaction.wrapping_add(1);
        if matches!(self.overlay, Some(Overlay::Password(_))) {
            self.pending_remote = None;
        }
        let closed = self.overlay.take();
        let back_to_remote =
            matches!(&closed, Some(Overlay::Menu(menu)) if menu.over_remote_servers);
        self.focus_return = match closed {
            Some(Overlay::Menu(menu)) => menu.returns_focus_to,
            Some(Overlay::SessionProviders) if self.appearance.agent_sessions_visible => self
                .agent_sessions
                .input
                .as_ref()
                .map(|input| gpui::Focusable::focus_handle(input, cx).downgrade()),
            _ => None,
        };
        self.overlay_subscription = None;
        self.focus_requested = true;
        if back_to_remote {
            self.overlay = Some(Overlay::RemoteServers);
        }
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
        // A focusable ancestor of the clicked row would otherwise take focus back.
        window.prevent_default();
        cx.notify();
    }
}

pub(crate) use muxy_ui::popover::clamp_to_viewport as clamp;

/// `content` with `mark` of the area it covers.
fn marked(content: AnyElement, mark: Option<Mark>, model: &AppModel) -> AnyElement {
    let Some(mark) = mark else {
        return content;
    };
    div()
        .relative()
        .child(content)
        .child(mark.of(model))
        .into_any_element()
}

/// The Remote popover above the sidebar's Remote section.
fn remote_servers(
    content: AnyElement,
    mark: Option<Mark>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let content = marked(content, mark, model);
    let anchor = model.remote_anchor.clone();
    let model = cx.entity().downgrade();
    muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
        let _ = model.update(cx, AppModel::dismiss_overlay);
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "Exhaustive overlay rendering dispatch"
)]
pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let Some(overlay) = &model.overlay else {
        return div().into_any_element();
    };
    let mark = overlay.mark(model.appearance.status_bar_visible);
    let content = match overlay {
        Overlay::Webview => return super::webview::modal::render(model, window, cx),
        Overlay::Popover => super::webview::popover::render(model, cx),
        Overlay::Server => {
            let content = marked(super::server_status::render(model, window, cx), mark, model);
            let anchor = model.server_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Overlay::Proxy(form) => {
            if model.appearance.status_bar_visible {
                let content = marked(super::proxy::render(form, model, window, cx), mark, model);
                let anchor = model.proxy.anchor.clone();
                let model = cx.entity().downgrade();
                muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                    let _ = model.update(cx, AppModel::dismiss_overlay);
                })
            } else {
                muxy_ui::popover::modal(&model.theme, super::proxy::render(form, model, window, cx))
            }
        }
        Overlay::Updates => {
            let content = marked(super::updates::render(model, window, cx), mark, model);
            let anchor = model.update_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Overlay::Tip => {
            let content = marked(super::sidebar::tips::popover_card(model, cx), mark, model);
            let anchor = model.tips.anchor.clone();
            if model.tip_placement() != Some(TipPlacement::Button) {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Overlay::RemoteServers => remote_servers(
            super::remote_servers::popover(model, window, cx),
            mark,
            model,
            cx,
        ),
        Overlay::Agents => super::agents_panel::popup(mark, model, window, cx),
        Overlay::SessionProviders => {
            let content = marked(
                super::agent_sessions::providers(model, window, cx),
                mark,
                model,
            );
            let anchor = model.agent_sessions.anchor.clone();
            let owner = cx.weak_entity();
            muxy_ui::popover::anchored_popover(anchor, content, move |_, cx| {
                let _ = owner.update(cx, AppModel::dismiss_overlay);
            })
        }
        Overlay::ServerForm(form) => muxy_ui::popover::modal(
            &model.theme,
            super::remote_servers::render_form(form, model, window, cx),
        ),
        Overlay::PairForm(form) => muxy_ui::popover::modal(
            &model.theme,
            super::pair_form::render(form, model, window, cx),
        ),
        Overlay::Password(prompt) => muxy_ui::popover::modal(
            &model.theme,
            super::remote_servers::render_password(prompt, model, window, cx),
        ),
        Overlay::Native(picker) => picker.clone().into_any_element(),
        Overlay::GitForm(form) => muxy_ui::popover::modal(
            &model.theme,
            super::git::render_form(form, model, window, cx),
        ),
        Overlay::AiProvider(menu) => {
            let entity = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(
                model.ai.anchors[menu.action.index()].clone(),
                marked(
                    super::git::render_provider_menu(menu, model, window, cx),
                    mark,
                    model,
                ),
                move |_, cx| {
                    let _ = entity.update(cx, AppModel::dismiss_overlay);
                },
            )
        }
        Overlay::Git(picker) => {
            let content = marked(picker.picker.clone().into_any_element(), mark, model);
            let model = cx.entity().downgrade();
            let dismiss = move |_: &mut Window, cx: &mut gpui::App| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            };
            muxy_ui::popover::anchored_popover_above(picker.anchor.clone(), content, dismiss)
        }
        Overlay::PullRequest(popover) => {
            let model_entity = cx.entity().downgrade();
            muxy_ui::popover::anchored_popover_above(
                model.git.pull_request_anchor.clone(),
                marked(super::git::render_pr(popover, model, cx), mark, model),
                move |_, cx| {
                    let _ = model_entity.update(cx, AppModel::dismiss_overlay);
                },
            )
        }
        Overlay::Menu(menu) if menu.over_remote_servers => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(remote_servers(
                super::remote_servers::list(model, window, cx).into_any_element(),
                mark,
                model,
                cx,
            ))
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
            .child(menu::render(menu, mark, model, window, cx))
            .into_any_element(),
        Overlay::Menu(menu) => menu::render(menu, mark, model, window, cx),
        Overlay::ProjectEditor(editor) => {
            super::project_editor::render(editor, mark, model, window, cx)
        }
        Overlay::ProjectIcons(picker) => {
            super::project_editor::icons::render(picker, mark, model, window, cx)
        }
        Overlay::ProjectLogo(cropper) => {
            super::project_editor::logo::render(cropper, model, window, cx)
        }
        Overlay::Sessions(picker) => picker.picker.clone().into_any_element(),
        Overlay::Worktrees(switcher) => switcher.picker.clone().into_any_element(),
        Overlay::Layouts(picker) => picker.picker.clone().into_any_element(),
        Overlay::Projects(picker) => picker.clone().into_any_element(),
        Overlay::Commands { palette, .. } => palette.clone().into_any_element(),
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
