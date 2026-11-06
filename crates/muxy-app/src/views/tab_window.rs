//! A tab in a window of its own: its splits under a titlebar with the
//! system's window controls, the tab's title, and a way back.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement, Styled, Subscription,
    WeakEntity, Window, div, px, relative,
};
use muxy_app_core::{Tab, TabWindowId};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::overlays::Overlay;
use super::{move_drag, overlays, splits, titlebar};
use crate::model::AppModel;
use crate::model::tab_windows::Surface;

pub(crate) struct TabWindowView {
    model: WeakEntity<AppModel>,
    id: TabWindowId,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl TabWindowView {
    pub(crate) fn new(
        model: WeakEntity<AppModel>,
        id: TabWindowId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let activation = cx.observe_window_activation(window, |view: &mut Self, window, cx| {
            let (id, active) = (view.id, window.is_window_active());
            let _ = view
                .model
                .update(cx, |model, cx| model.tab_window_activated(id, active, cx));
        });
        let bounds = cx.observe_window_bounds(window, |view: &mut Self, window, cx| {
            let id = view.id;
            let _ = view
                .model
                .update(cx, |model, cx| model.tab_window_moved(id, window, cx));
        });
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        cx.on_release(|view: &mut Self, cx| {
            let id = view.id;
            let _ = view
                .model
                .update(cx, |model, cx| model.tab_window_closed(id, cx));
        })
        .detach();
        Self {
            model,
            id,
            focus: cx.focus_handle(),
            _subscriptions: vec![activation, bounds, appearance],
        }
    }
}

impl Render for TabWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id;
        let focus = self.focus.clone();
        self.model
            .update(cx, |model, cx| {
                model.drawing(Surface::Tab(id), |model| {
                    model.render_tab_window(id, &focus, window, cx)
                })
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl AppModel {
    fn render_tab_window(
        &mut self,
        id: TabWindowId,
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(tab) = self.window_tab(id).and_then(|tab| self.tab(tab)).cloned() else {
            return div().into_any_element();
        };
        self.metrics.apply_to_window(window);
        self.prepare_tab_window(id, &tab, window, cx);
        let theme = &self.theme;
        let menu = self.tab_windows.overlay == Surface::Tab(id)
            && matches!(self.overlay, Some(Overlay::Menu(_)));
        let bare = self.headers_on_top(&tab);
        let body = splits::tab_window_body(&tab, self, cx);
        let content = super::workspace::action_handlers(cx)
            .capture_any_mouse_down(cx.listener(move |model, _, _, _| {
                model.enter_surface(Surface::Tab(id));
            }))
            .capture_key_down(cx.listener(move |model, event, window, cx| {
                model.enter_surface(Surface::Tab(id));
                model.cancel_drag_on_escape(event, window, cx);
            }))
            .track_focus(focus)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.fg)
            .font_family(".SystemUIFont")
            .line_height(relative(1.2))
            .when(!bare, |content| {
                content
                    .child(titlebar(id, &tab, self, window, cx))
                    .child(div().h(px(1.0)).flex_none().bg(theme.border))
            })
            .child(
                div()
                    .debug_selector(|| "tab-window-content".into())
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(body),
            )
            .child(splits::drag::track_pointer(cx))
            .child(splits::drag::overlay(self, window))
            .children(
                bare.then(|| window_controls(id, self, window, cx))
                    .flatten(),
            )
            .child(move_drag::track_carry(id, cx))
            .child(move_drag::overlay(self, window))
            .when(menu, |content| {
                content.child(titlebar::overlay(overlays::layer(self, window, cx), window))
            });
        #[cfg(target_os = "linux")]
        let content = titlebar::frame::render(content, theme, None, None, window);
        content.into_any_element()
    }

    /// Readies a tab window to draw: its title and traffic lights, its pages,
    /// the drop zones it records, and the keyboard when asked.
    fn prepare_tab_window(
        &mut self,
        id: TabWindowId,
        tab: &Tab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let here = window.window_handle().window_id();
        #[cfg(target_os = "linux")]
        self.tab_windows
            .set_controls(id, titlebar::linux::controls_width(window, self.metrics));
        self.layout_drag.geometry.clear(here);
        self.move_drag.zones.note_window(window);
        window.set_traffic_light_position(titlebar::traffic_light_position(
            self.metrics,
            window.scale_factor(),
        ));
        let title = self.tab_window_title(tab.id, cx);
        if self.tab_windows.retitle(id, &title) {
            window.set_window_title(&title);
        }
        let descriptors = self.ensure_webview_panes(window, cx);
        let coverage = self.coverage();
        self.present_page_panes(&descriptors, coverage.hides_webviews(), false, window, cx);
        let requested = self.tab_windows.key_window() == Some(id)
            && self.overlay.is_none()
            && self.focus_owner.wants_active_pane();
        if requested {
            self.focus_owner.settle();
        }
        if (self.tab_windows.take_focus(id) || requested) && self.overlay.is_none() {
            self.focus_active(window, cx);
        }
    }
}

fn titlebar(
    id: TabWindowId,
    tab: &Tab,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let leading = if cfg!(target_os = "macos") {
        m.traffic_light_width()
    } else {
        m.spacing5()
    };
    let weak = cx.weak_entity();
    let drag = titlebar::background_with("tab-window-titlebar", move |event, window, cx| {
        let position = event.position;
        let _ = weak.update(cx, |model, cx| model.begin_carry(id, position, window, cx));
    })
    .flex()
    .flex_1()
    .min_w(px(0.0))
    .h_full()
    .items_center()
    .pl(leading)
    .child(title(tab, model, cx));
    let bar = div()
        .debug_selector(|| "tab-window-titlebar".into())
        .flex()
        .flex_none()
        .items_center()
        .h(m.title_bar_height())
        .child(drag)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(m.spacing2())
                .px(m.spacing3())
                .child(button(
                    "tab-window-return",
                    Icon::AppWindow,
                    &tr!("Move to Main Window"),
                    model,
                    cx.listener(move |model, _, _, cx| model.return_tab_window(id, None, cx)),
                )),
        );
    #[cfg(target_os = "linux")]
    if let Some(controls) = super::titlebar::linux::controls(
        window,
        &model.theme,
        m,
        cx.listener(move |model, _, _, cx| model.close_tab_window(id, cx)),
    ) {
        return bar
            .child(controls.border_l_1().border_color(model.theme.border))
            .into_any_element();
    }
    #[cfg(not(target_os = "linux"))]
    let _ = window;
    bar.into_any_element()
}

/// The system's window buttons at the top right, over the pane header
/// there, where the system draws none.
fn window_controls(
    id: TabWindowId,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    #[cfg(target_os = "linux")]
    {
        let controls = super::titlebar::linux::controls(
            window,
            &model.theme,
            model.metrics,
            cx.listener(move |model, _, _, cx| model.close_tab_window(id, cx)),
        )?;
        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .h(model.metrics.title_bar_height())
                .flex()
                .items_center()
                .child(controls)
                .into_any_element(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (id, model, window, cx);
        None
    }
}

/// The tab's color, title and project.
fn title(tab: &Tab, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let project = model
        .tab_project(tab.id)
        .map(|project| model.project_title(project))
        .unwrap_or_default();
    let title = SharedString::from(model.webview_title(tab, cx).to_owned());
    let color = tab
        .color
        .as_ref()
        .and_then(|color| muxy_ui::theme::parse_hex(color.as_str()))
        .map(gpui::Hsla::from);
    div()
        .flex()
        .items_center()
        .gap(m.spacing3())
        .min_w(px(0.0))
        .when_some(color, |row, color| {
            row.child(
                div()
                    .size(m.scaled(8.0))
                    .flex_none()
                    .rounded_full()
                    .bg(color),
            )
        })
        .child(
            div()
                .debug_selector(|| "tab-window-title".into())
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(m.font_body())
                .child(title),
        )
        .child(
            div()
                .debug_selector(|| "tab-window-project".into())
                .flex_none()
                .text_size(m.font_footnote())
                .text_color(model.theme.fg_muted)
                .child(project),
        )
        .into_any_element()
}

fn button(
    id: &'static str,
    icon: Icon,
    label: &SharedString,
    model: &AppModel,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let m = model.metrics;
    let hover = model.theme.fg.opacity(0.08);
    titlebar::with_tooltip(
        div()
            .id(id)
            .debug_selector(move || id.into())
            .role(Role::Button)
            .aria_label(label.clone())
            .flex()
            .items_center()
            .justify_center()
            .size(m.control_medium())
            .rounded(m.radius_md())
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(on_click)
            .child(muxy_ui::components::IconGlyph::new(
                icon,
                m.icon_md(),
                model.theme.fg_muted,
            )),
        label.to_string(),
        model,
    )
    .into_any_element()
}
