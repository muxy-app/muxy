use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use muxy_core::capability::Feature;
use muxy_core::release::Channel;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::{accessibility, controls, form, tr};

use super::{Category, SettingsEvent, SettingsView};

impl SettingsView {
    pub(crate) fn show_about(&mut self, cx: &mut Context<Self>) {
        self.category = Category::About;
        self.section = None;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }
}

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let channel = Channel::current();
    let mut rows = Vec::new();
    for (id, label, value) in [
        ("about-name", tr_key!("App name"), channel.app_name().into()),
        (
            "about-version",
            tr_key!("Version"),
            env!("CARGO_PKG_VERSION").into(),
        ),
        (
            "about-channel",
            tr_key!("Channel"),
            match channel {
                Channel::Beta => tr!("Beta"),
                Channel::Stable => tr!("Stable"),
            },
        ),
    ] {
        if !view.matches(Category::About, label) {
            continue;
        }
        rows.push(
            form::row(
                view.layout_style(),
                &translate(label),
                None,
                accessibility::label(id, value),
                view.compact,
            )
            .border_b_1()
            .border_color(view.theme.border)
            .debug_selector(move || format!("settings-{id}"))
            .into_any_element(),
        );
    }
    if view.snapshot.capabilities.has_backend(Feature::Updates)
        && view.matches(Category::About, tr_key!("Check for Updates…"))
    {
        rows.push(
            div()
                .py(view.layout_metrics.spacing7())
                .child(
                    controls::button(
                        view.style(),
                        "about-check-for-updates",
                        controls::Button::secondary(&tr!("Check for Updates…")),
                        cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::CheckForUpdates)),
                    )
                    .debug_selector(|| "settings-about-check-for-updates".into()),
                )
                .into_any_element(),
        );
    }
    rows
}
