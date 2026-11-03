use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use muxy_core::capability::Feature;
use muxy_core::release::Channel;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::{accessibility, controls, form, tr};

use super::{Category, SettingsEvent, SettingsView};

impl SettingsView {
    pub(crate) fn show_about(&mut self, cx: &mut Context<Self>) {
        self.finish_terminal_slider(cx);
        self.recording = None;
        self.cancel_quick_recording(cx);
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

#[cfg(test)]
mod tests {
    use super::Category;
    use crate::model::{AppModel, tests::stub_boot};
    use gpui::TestAppContext;
    use muxy_app_core::AppState;

    #[gpui::test]
    fn about_search_only_offers_configuration_for_preference_results(cx: &mut TestAppContext) {
        for (category, query, editable) in [
            (Category::About, "", false),
            (Category::About, "version", false),
            (Category::About, "updates", false),
            (Category::About, "marketplace", false),
            (Category::General, "marketplace", false),
            (Category::About, "width", true),
            (Category::About, "a", true),
            (Category::About, "no-match", false),
            (Category::General, "", true),
            (Category::General, "version", false),
            (Category::General, "a", true),
        ] {
            let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
            let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
            model.update_in(cx, |model, window, cx| {
                model.open_settings(window, cx);
                model
                    .settings_tab
                    .as_ref()
                    .expect("settings")
                    .view
                    .update(cx, |view, cx| {
                        view.show_about(cx);
                        view.category = category;
                        view.query = query.into();
                        view.search
                            .update(cx, |input, cx| input.set_text(query, cx));
                    });
            });
            cx.run_until_parked();
            assert_eq!(
                cx.debug_bounds("settings-open-configuration").is_some(),
                editable,
                "{query}"
            );
        }
    }

    #[gpui::test]
    fn about_cancels_shortcut_recording(cx: &mut TestAppContext) {
        let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        model.update_in(cx, |model, window, cx| {
            model.open_settings(window, cx);
            model
                .settings_tab
                .as_ref()
                .expect("settings")
                .view
                .update(cx, |view, cx| {
                    let (recorder, _receiver) =
                        muxy_ui::quick_terminal::start_shortcut_recording().expect("recorder");
                    let task = cx.spawn(async |_, _| std::future::pending::<()>().await);
                    view.quick_recording = Some((recorder, task));
                    view.recording = Some("open_settings".into());
                    view.show_about(cx);
                    assert!(view.quick_recording.is_none());
                    assert!(view.recording.is_none());
                });
        });
    }
}
