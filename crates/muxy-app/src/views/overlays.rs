use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, Styled, Window, div,
};
use muxy_ui::popover::PopoverAnchor;

use super::menu::{Item, Menu};
use crate::model::AppModel;

pub(crate) enum Overlay {
    Updates,
    Server,
    /// The tip popover beside the collapsed sidebar's tip button.
    Tip,
    Webview,
    /// The open extension popover (`AppModel::webviews.popover`).
    Popover,
    Native(Entity<super::native_modal::NativeModal>),
    Commands(super::command_palette::Palette),
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

/// One kind of overlay: what it draws, where, and what dismissing it
/// undoes. Each is implemented beside the overlay's rendering.
pub(crate) trait OverlayView {
    /// The overlay's content. `mark` is only given to overlays that place
    /// themselves; [`place`] marks the others.
    fn render(
        &self,
        mark: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement;

    fn placement(&self, _model: &AppModel) -> Placement {
        Placement::Own
    }

    /// Whether native views elsewhere stay visible while the overlay is
    /// open. Modal dialogs dim the whole window, so they don't.
    fn marks_bounds(&self, _model: &AppModel) -> bool {
        false
    }

    /// What gets the keyboard back when the overlay closes, instead of the
    /// active pane.
    fn returns_focus_to(&self, _model: &AppModel, _cx: &App) -> Option<FocusHandle> {
        None
    }

    /// Cancels what the overlay left pending, when the user dismisses it
    /// rather than finishing it.
    fn on_dismiss(&self, _model: &mut AppModel, _cx: &mut Context<AppModel>) -> Dismissal {
        Dismissal::Close
    }
}

pub(crate) enum Placement {
    /// A popover above `anchor`.
    Above(PopoverAnchor),
    /// A popover below `anchor`.
    Below(PopoverAnchor),
    /// Centered over the dimmed window.
    Modal,
    /// Where the content puts itself, over a backdrop that dismisses on a
    /// click.
    Own,
    /// Where the content puts itself, with its own backdrop.
    OwnBackdrop,
}

impl Placement {
    /// Above `anchor` while `shown`. Otherwise the anchor is forgotten, so
    /// the popover isn't drawn where its hidden button was.
    pub(crate) fn above_while(shown: bool, anchor: PopoverAnchor) -> Self {
        if !shown {
            anchor.set(None);
        }
        Self::Above(anchor)
    }
}

/// What happens to an overlay the user dismissed.
pub(crate) enum Dismissal {
    Close,
    /// It closes and `next` opens in its place.
    Reopen(Overlay),
    /// It stays open until its page agrees to close.
    Stay,
}

impl Overlay {
    pub(crate) fn view(&self) -> &dyn OverlayView {
        match self {
            Self::Updates => &super::updates::UpdatesPopover,
            Self::Server => &super::server_status::ServerPopover,
            Self::Tip => &super::sidebar::tips::TipPopover,
            Self::Webview => &super::webview::modal::WebviewModal,
            Self::Popover => &super::webview::popover::ExtensionPopover,
            Self::Native(modal) => modal,
            Self::Commands(palette) => palette,
            Self::Git(picker) => picker,
            Self::GitForm(form) => form.as_ref(),
            Self::AiProvider(menu) => menu,
            Self::PullRequest(popover) => popover,
            Self::Sessions(picker) => picker,
            Self::Worktrees(picker) => picker,
            Self::Layouts(picker) => picker,
            Self::Menu(menu) => menu,
            Self::ProjectEditor(editor) => editor,
            Self::ProjectIcons(icons) => icons,
            Self::ProjectLogo(cropper) => cropper,
            Self::Projects(picker) => picker,
            Self::RemoteServers => &super::remote_servers::RemotePopover,
            Self::Agents => &super::agents_panel::AgentsPopup,
            Self::SessionProviders => &super::agent_sessions::ProvidersMenu,
            Self::ServerForm(form) => form.as_ref(),
            Self::PairForm(form) => form.as_ref(),
            Self::Password(prompt) => prompt.as_ref(),
            Self::Proxy(form) => form.as_ref(),
        }
    }

    /// The mark of the area the overlay covers. Rendering and
    /// [`AppModel::coverage`] both read it.
    pub(crate) fn mark(&self, model: &AppModel) -> Option<Mark> {
        self.view().marks_bounds(model).then_some(Mark(()))
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
    /// Closes the overlay as the user's cancel: Escape, an outside click, or
    /// its toggle.
    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        let Some(overlay) = self.overlay.take() else {
            self.overlay_closed(None, cx);
            return;
        };
        let view = overlay.view();
        let returns_to = view.returns_focus_to(self, cx);
        match view.on_dismiss(self, cx) {
            Dismissal::Close => self.overlay_closed(returns_to.as_ref(), cx),
            Dismissal::Reopen(next) => {
                self.overlay_closed(returns_to.as_ref(), cx);
                self.overlay = Some(next);
            }
            Dismissal::Stay => self.overlay = Some(overlay),
        }
    }

    /// Closes the overlay once its work is done, keeping what it started.
    pub(crate) fn close_overlay(&mut self, cx: &mut Context<Self>) {
        let returns_to = self
            .overlay
            .take()
            .and_then(|overlay| overlay.view().returns_focus_to(self, cx));
        self.overlay_closed(returns_to.as_ref(), cx);
    }

    fn overlay_closed(&mut self, returns_to: Option<&FocusHandle>, cx: &mut Context<Self>) {
        self.project_logo_task = None;
        self.overlay_subscription = None;
        self.focus_owner.overlay_closed(returns_to);
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

/// `content` at `placement`, with `mark` of the area it covers.
pub(crate) fn place(
    placement: Placement,
    content: AnyElement,
    mark: Option<Mark>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let (anchor, above) = match placement {
        Placement::Above(anchor) => (anchor, true),
        Placement::Below(anchor) => (anchor, false),
        Placement::Modal => return muxy_ui::popover::modal(&model.theme, content),
        Placement::Own | Placement::OwnBackdrop => return content,
    };
    let content = match mark {
        Some(mark) => div()
            .relative()
            .child(content)
            .child(mark.of(model))
            .into_any_element(),
        None => content,
    };
    let owner = cx.weak_entity();
    let dismiss = move |_: &mut Window, cx: &mut App| {
        let _ = owner.update(cx, AppModel::dismiss_overlay);
    };
    if above {
        muxy_ui::popover::anchored_popover_above(anchor, content, dismiss)
    } else {
        muxy_ui::popover::anchored_popover(anchor, content, dismiss)
    }
}

/// A full-window layer that dismisses the overlay on a click.
pub(crate) fn backdrop(cx: &mut Context<AppModel>) -> AnyElement {
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
        )
        .into_any_element()
}

pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let Some(overlay) = &model.overlay else {
        return div().into_any_element();
    };
    let view = overlay.view();
    let mark = overlay.mark(model);
    let placement = view.placement(model);
    let places_itself = matches!(placement, Placement::Own | Placement::OwnBackdrop);
    let own_backdrop = matches!(placement, Placement::OwnBackdrop);
    let content = view.render(mark.filter(|_| places_itself), model, window, cx);
    let content = place(placement, content, mark, model, cx);
    if own_backdrop {
        return content;
    }
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(backdrop(cx))
        .child(content)
        .into_any_element()
}
