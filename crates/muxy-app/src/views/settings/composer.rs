use super::slider::{Commit, Slider};
use super::{Category, SettingsEvent, SettingsView};
use crate::model::preferences::{Change, ComposerToggle};
use gpui::{AnyElement, Context, IntoElement};
use muxy_app_core::composer::submission::ImageSubmissionStrategy;
use muxy_core::capability::Feature;
use muxy_ui::controls::{self, Choice};
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.composer;
    let mut rows = Vec::new();
    for (toggle, label, value) in [
        (
            ComposerToggle::ClearAfter,
            tr_key!("Clear after sending"),
            settings.clear_after_sending,
        ),
        (
            ComposerToggle::ClearClose,
            tr_key!("Clear on close"),
            settings.clear_on_close,
        ),
    ] {
        if !view.matches(Category::Composer, label) {
            continue;
        }
        let id = toggle.id();
        rows.push(view.row(
            id,
            label,
            view.toggle(id, value, Change::Composer(toggle, !value), cx),
        ));
    }
    if view.matches(Category::Composer, tr_key!("Composer font")) {
        rows.push(view.row(
            "composer-font",
            tr_key!("Composer font"),
            view.field("composer-font"),
        ));
    }
    if view.matches(Category::Composer, tr_key!("Composer line height")) {
        rows.push(view.row(
            "composer-line-height",
            tr_key!("Composer line height"),
            view.slider(
                Slider::new("composer-line-height", (1.1, 2.0), 0.05, "", Commit::Field),
                view.snapshot.settings.composer.line_height,
                cx,
            ),
        ));
    }
    rows.extend(image_rows(view, cx));
    rows
}

fn image_rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let settings = &view.snapshot.settings.composer;
    let clipboard = view
        .snapshot
        .capabilities
        .unsupported(Feature::ClipboardImages);
    let mut rows = Vec::new();
    if view.matches(Category::Composer, tr_key!("Image submission")) {
        rows.push(view.row(
            ComposerToggle::Images.id(),
            tr_key!("Image submission"),
            controls::segmented(
                view.style(),
                ComposerToggle::Images.id(),
                &tr!("Image submission"),
                &[
                    Choice {
                        enabled: clipboard.is_none(),
                        ..Choice::new("clipboard", tr!("Clipboard paste"))
                    },
                    Choice::new("path", tr!("Inline file path")),
                ],
                if settings.image_strategy == ImageSubmissionStrategy::Clipboard {
                    "clipboard"
                } else {
                    "path"
                },
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::Composer(
                        ComposerToggle::Images,
                        selected.as_ref() == "clipboard",
                    )));
                }),
            ),
        ));
        if let Some(reason) = &clipboard {
            rows.push(
                view.note(
                    &tr!("%@ Images are sent as inline file paths.", reason.as_ref()),
                    false,
                )
                .into_any_element(),
            );
        }
    }
    rows
}
