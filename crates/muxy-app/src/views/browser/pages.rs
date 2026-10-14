//! What the content area shows instead of a page: the start page and the
//! page that couldn't be opened.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_app_core::browser::SearchEngine;
use muxy_ui::components::{IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

use super::{BrowserView, LoadError};

pub(super) fn start(
    engine: SearchEngine,
    theme: &Theme,
    metrics: Metrics,
    cx: &mut Context<BrowserView>,
) -> AnyElement {
    centered(metrics)
        .id("browser-start")
        .debug_selector(|| "browser-start".into())
        .child(IconGlyph::new(
            Icon::Globe,
            metrics.icon_xxl(),
            theme.fg_muted,
        ))
        .child(
            div()
                .text_size(metrics.font_headline())
                .text_color(theme.fg)
                .child(tr!("New Tab")),
        )
        .child(
            div()
                .text_size(metrics.font_body())
                .text_color(theme.fg_muted)
                .child(tr!(
                    "Search with %@ or enter a website address.",
                    engine.name()
                )),
        )
        .child(
            div()
                .id("browser-start-search")
                .flex()
                .items_center()
                .justify_between()
                .gap(metrics.spacing4())
                .w(metrics.scaled(320.0))
                .h(metrics.control_large())
                .px(metrics.spacing5())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .cursor_text()
                .text_size(metrics.font_body())
                .text_color(theme.fg_dim)
                .on_click(cx.listener(|view, _, window, cx| view.focus_address(window, cx)))
                .child(tr!("Search or enter address"))
                .child(div().text_color(theme.fg_dim).child("⌘L")),
        )
        .into_any_element()
}

pub(super) fn error(
    error: &LoadError,
    theme: &Theme,
    metrics: Metrics,
    cx: &mut Context<BrowserView>,
) -> AnyElement {
    centered(metrics)
        .id("browser-error")
        .debug_selector(|| "browser-error".into())
        .child(
            SymbolGlyph::new(
                "exclamationmark.triangle",
                metrics.icon_xxl(),
                theme.warning,
            )
            .regular(),
        )
        .child(
            div()
                .text_size(metrics.font_headline())
                .text_color(theme.fg)
                .child(tr!("This page can't be opened")),
        )
        .child(
            div()
                .max_w(metrics.scaled(420.0))
                .text_center()
                .text_size(metrics.font_body())
                .text_color(theme.fg_muted)
                .child(error.message.clone()),
        )
        .child(
            div()
                .max_w(metrics.scaled(420.0))
                .truncate()
                .font_family("SF Mono")
                .text_size(metrics.font_caption())
                .text_color(theme.fg_dim)
                .child(error.url.clone()),
        )
        .child(
            div()
                .id("browser-try-again")
                .mt(metrics.spacing4())
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .rounded(metrics.radius_md())
                .bg(theme.accent)
                .text_color(theme.accent_foreground)
                .text_size(metrics.font_body())
                .cursor_pointer()
                .on_click(cx.listener(|view, _, _, cx| view.reload(cx)))
                .child(tr!("Try Again")),
        )
        .into_any_element()
}

fn centered(metrics: Metrics) -> gpui::Div {
    div()
        .absolute()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(metrics.spacing4())
        .p(metrics.spacing8())
        .min_w(px(0.0))
}
