use super::{Category, Change, SettingsEvent, SettingsView};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use muxy_app_core::composer::submission::ImageSubmissionStrategy;
use muxy_ui::components::ButtonInteraction;
use muxy_ui::controls::{self, Choice};

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.composer;
    let mut rows = Vec::new();
    for (id, label, value) in [
        (
            "voice-auto-send",
            "Send after recording",
            settings.voice_auto_send,
        ),
        (
            "composer-clear-after",
            "Clear after sending",
            settings.clear_after_sending,
        ),
        (
            "composer-clear-close",
            "Clear on close",
            settings.clear_on_close,
        ),
    ] {
        if view.matches(Category::Composer, label) {
            rows.push(view.row(
                id,
                label,
                view.toggle(id, value, Change::Composer(id, !value), cx),
            ));
        }
    }
    for (id, label) in [
        ("composer-font", "Composer font"),
        ("composer-line-height", "Composer line height"),
    ] {
        if view.matches(Category::Composer, label) {
            rows.push(view.row(id, label, view.field(id)));
        }
    }
    if view.matches(Category::Composer, "Image submission") {
        rows.push(view.row(
            "composer-images",
            "Image submission",
            controls::segmented(
                view.style(),
                "composer-images",
                &[
                    Choice::new("clipboard", "Clipboard paste"),
                    Choice::new("path", "Inline file path"),
                ],
                if settings.image_strategy == ImageSubmissionStrategy::Clipboard {
                    "clipboard"
                } else {
                    "path"
                },
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::Composer(
                        "composer-images",
                        selected.as_ref() == "clipboard",
                    )));
                }),
            ),
        ));
    }
    if view.matches(Category::Composer, "Dictation language") {
        rows.push(
            view.row(
                "composer-language",
                "Dictation language",
                div()
                    .id("composer-language")
                    .px(view.metrics.spacing4())
                    .py(view.metrics.spacing2())
                    .rounded(view.metrics.radius_md())
                    .bg(view.theme.surface)
                    .child(if settings.language.is_empty() {
                        "System language".to_owned()
                    } else {
                        settings.language.clone()
                    })
                    .button_interaction(
                        cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::DictationLanguage)),
                    )
                    .into_any_element(),
            ),
        );
    }
    rows
}
