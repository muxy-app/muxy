//! A tab floating over the main window: a card holding its splits. The pane
//! headers along its top move it and hold its controls; without them, a bar
//! of its own does.

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Styled, Window, div, px,
};
use muxy_app_core::{FloatScope, Tab, TabWindowId};
use muxy_ui::floating::FloatingCard;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::{splits, tab_window};
use crate::model::AppModel;
use crate::model::tab_windows::Surface;
use crate::model::tab_windows::floating::panel;

/// Each floating tab the main window shows, drawn as its own surface so
/// what it asks about the active tab and pane is about the floating tab.
pub(crate) fn cards(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> HashMap<TabWindowId, AnyElement> {
    model
        .shown_floats()
        .into_iter()
        .filter_map(|id| {
            let card = model.drawing(Surface::Tab(id), |model| card(model, id, window, cx))?;
            Some((id, card))
        })
        .collect()
}

fn card(
    model: &mut AppModel,
    id: TabWindowId,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let tab = model
        .window_tab(id)
        .and_then(|tab| model.tab(tab))
        .cloned()?;
    model.place_float(id, window);
    take_focus(model, id, window, cx);
    let frame = model.float_frame(id, window)?;
    let bare = model.headers_on_top(&tab);
    let m = model.metrics;
    let theme = model.theme.clone();
    let resize_owner = cx.weak_entity();
    let press_owner = cx.weak_entity();
    let body = splits::tab_window_body(&tab, model, cx);
    Some(
        FloatingCard::new(
            SharedString::from(format!("floating-tab-{id}")),
            frame,
            theme.clone(),
            m,
        )
        .on_resize(move |edges, pointer, window, cx| {
            let _ = resize_owner.update(cx, |model, cx| {
                model.begin_floating_drag(&panel(id), edges, pointer, window, cx);
            });
        })
        .on_press(move |_, cx| {
            let _ = press_owner.update(cx, |model, cx| {
                model.enter_float(id, cx);
            });
        })
        .child(
            div()
                .debug_selector(|| "floating-tab".into())
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.0))
                .rounded(m.radius_xl())
                .overflow_hidden()
                .bg(theme.bg)
                .capture_key_down(cx.listener(move |model, event, window, cx| {
                    model.enter_surface(Surface::Tab(id));
                    model.cancel_drag_on_escape(event, window, cx);
                }))
                .when(!bare, |card| {
                    card.child(bar(id, &tab, model, cx))
                        .child(div().h(px(1.0)).flex_none().bg(theme.border))
                })
                .child(
                    div()
                        .debug_selector(|| "floating-tab-content".into())
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .child(body),
                ),
        )
        .child(model.webview_occlusion())
        .into_any_element(),
    )
}

/// Gives the floating tab's focused pane the keyboard when it was asked to
/// take it.
fn take_focus(model: &mut AppModel, id: TabWindowId, window: &mut Window, cx: &Context<AppModel>) {
    let requested = model.tab_windows.key_window() == Some(id)
        && model.overlay.is_none()
        && model.focus_owner.wants_active_pane();
    if requested {
        model.focus_owner.settle();
    }
    if (model.tab_windows.take_focus(id) || requested) && model.overlay.is_none() {
        model.focus_active(window, cx);
    }
}

/// The bar along the card's top: dragging it moves the card.
fn bar(id: TabWindowId, tab: &Tab, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let owner = cx.weak_entity();
    let everywhere = model.float_scope(id) == Some(FloatScope::AllProjects);
    let project = model
        .tab_project(tab.id)
        .map(|project| model.project_title(project))
        .unwrap_or_default();
    let scope_label = if everywhere {
        tr!("Show Only in %@", project.as_str())
    } else {
        tr!("Show in All Projects")
    };
    div()
        .debug_selector(|| "floating-tab-bar".into())
        .flex()
        .flex_none()
        .items_center()
        .h(m.scaled(32.0))
        .pl(m.spacing5())
        .pr(m.spacing2())
        .gap(m.spacing2())
        .cursor_grab()
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                let _ = owner.update(cx, |model, cx| {
                    model.enter_surface(Surface::Tab(id));
                    model.begin_floating_drag(&panel(id), None, event.position, window, cx);
                });
                window.prevent_default();
                cx.stop_propagation();
            },
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .child(tab_window::title(tab, model, cx)),
        )
        .child(tab_window::button(
            "floating-tab-scope",
            if everywhere {
                Icon::Globe
            } else {
                Icon::Folder
            },
            &scope_label,
            model,
            cx.listener(move |model, _, _, cx| model.toggle_float_scope(id, cx)),
        ))
        .child(tab_window::button(
            "floating-tab-dock",
            Icon::AppWindow,
            &tr!("Dock as Tab"),
            model,
            cx.listener(move |model, _, _, cx| model.return_tab_window(id, None, cx)),
        ))
        .child(tab_window::button(
            "floating-tab-close",
            Icon::X,
            &tr!("Close"),
            model,
            cx.listener(move |model, _, _, cx| model.close_tab_window(id, cx)),
        ))
        .into_any_element()
}
