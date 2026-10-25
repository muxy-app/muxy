use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    Window, div,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::panel::{
    PanelAction, PanelChrome, PanelControl, PanelFrame, PanelMode, PanelPlacement, PanelPosition,
    PanelSizeBounds, PanelSizing, PanelStyle,
};
use muxy_ui::tr;

use super::workspace::{
    ClosePane, CloseTab, DetachTerminal, EqualizeSplits, SplitDown, SplitRight, ToggleZoomPane,
};
use crate::model::AppModel;

const PANEL: &str = "builtin:pinned-terminal";

pub(crate) fn panel_content(
    content: AnyElement,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    if !model.pinned_terminal_shown() {
        return content;
    }
    let m = model.metrics;
    let scale = f32::from(m.scaled(1.0));
    let placement = PanelPlacement::new(PANEL, PanelPosition::Right, PanelMode::Pinned);
    let maximum = f32::from(window.viewport_size().width) - model.sidebar_width() - 100.0;
    let sizing = PanelSizing::new(
        &placement,
        f32::from(m.scaled(model.appearance.pinned_terminal_width.unwrap_or(420.0))),
        PanelSizeBounds::new(
            f32::from(m.scaled(240.0)),
            maximum.min(f32::from(m.scaled(1200.0))),
        ),
        model.pinned.resize.clone(),
    );
    let owner = cx.weak_entity();
    let frame = PanelFrame::new(
        placement,
        sizing,
        chrome(model, cx),
        body(model, cx),
        move |width, _, cx| {
            let _ = owner.update(cx, |model, cx| {
                model.appearance.pinned_terminal_width = Some(width / scale);
                if !model.pinned.resize.is_active()
                    && model.appearance.pinned_terminal_width
                        != model.settings.appearance.pinned_terminal_width
                {
                    model.save_appearance(cx);
                }
                cx.notify();
            });
        },
        PanelStyle::new(model.theme.clone(), m),
    )
    .grip(model.webview_grip(None, gpui::CursorStyle::ResizeLeftRight));
    div()
        .debug_selector(|| "pinned-terminal-panel".into())
        .flex()
        .size_full()
        .min_w_0()
        .min_h_0()
        .child(
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(content),
        )
        .child(frame)
        .into_any_element()
}

fn chrome(model: &AppModel, cx: &mut Context<AppModel>) -> PanelChrome {
    let m = model.metrics;
    let [header, unpin, close] = model.pinned.controls.clone();
    let title: SharedString = model
        .state
        .pinned_terminal()
        .map(|pane| pane.title.clone())
        .filter(|title| !title.is_empty())
        .map_or_else(|| tr!("Pinned Terminal"), Into::into);
    let owner = cx.weak_entity();
    let unpin = PanelAction::icon(
        "unpin-pinned-terminal",
        tr!("Move to Tab"),
        Icon::PinOff,
        unpin,
        move |_, cx| {
            let _ = owner.update(cx, AppModel::unpin_terminal);
        },
    );
    let owner = cx.weak_entity();
    let close = PanelAction::control(
        "hide-pinned-terminal",
        PanelControl::Close,
        close,
        move |window, cx| {
            let _ = owner.update(cx, |model, cx| model.toggle_pinned_terminal(window, cx));
        },
    );
    let placeholder = || {
        PanelAction::control(
            "pinned-terminal-placeholder",
            PanelControl::Close,
            header.clone(),
            |_, _| {},
        )
    };
    PanelChrome::new(
        title,
        Some(
            IconGlyph::new(Icon::Terminal, m.font_footnote(), model.theme.fg_muted)
                .into_any_element(),
        ),
        header.clone(),
        placeholder(),
        placeholder(),
        close,
        PanelStyle::new(model.theme.clone(), m),
    )
    .without_move_action()
    .without_mode_action()
    .with_trailing_action(unpin)
}

fn body(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let terminal = model
        .state
        .pinned_terminal()
        .and_then(|pane| model.terminal(&pane.id))
        .map(crate::model::PaneView::element);
    div()
        .id("pinned-terminal")
        .debug_selector(|| "pinned-terminal".into())
        .flex()
        .flex_col()
        .flex_1()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .bg(model.theme.bg)
        .on_action(cx.listener(|model, _: &CloseTab, window, cx| {
            model.toggle_pinned_terminal(window, cx);
        }))
        .on_action(cx.listener(|model, _: &ClosePane, window, cx| {
            model.toggle_pinned_terminal(window, cx);
        }))
        .on_action(|_: &SplitRight, _, _| {})
        .on_action(|_: &SplitDown, _, _| {})
        .on_action(|_: &ToggleZoomPane, _, _| {})
        .on_action(|_: &EqualizeSplits, _, _| {})
        .on_action(|_: &DetachTerminal, _, _| {})
        .children(terminal)
        .into_any_element()
}
