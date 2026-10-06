use super::{Category, Change, PickerKind, SettingsEvent, SettingsView};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
};
use muxy_app_core::settings::NewPaneDirectory;
use muxy_ui::controls::{self, Choice};
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

pub(super) fn rows(
    pane: &SettingsView,
    _: &Window,
    cx: &mut Context<SettingsView>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if pane.matches(Category::Terminal, tr_key!("Font family")) {
        let default = tr!("Default");
        rows.push(
            pane.row(
                "font-family",
                tr_key!("Font family"),
                pane.picker(
                    PickerKind::FontFamily,
                    pane.snapshot
                        .terminal
                        .font_families
                        .first()
                        .map_or(default.as_str(), String::as_str),
                    cx,
                ),
            ),
        );
    }
    for (id, label) in [
        ("font-size", tr_key!("Font size (points)")),
        (
            "adjust-cell-height",
            tr_key!("Cell height adjustment (pixels or %%)"),
        ),
    ] {
        if pane.matches(Category::Terminal, label) {
            rows.push(pane.row(id, label, pane.field(id)));
        }
    }
    let clipboard = pane.snapshot.settings.clipboard.copy_on_select;
    if pane.matches(Category::Terminal, tr_key!("Copy on select")) {
        rows.push(pane.row(
            "copy-on-select",
            tr_key!("Copy on select"),
            pane.toggle(
                "copy-on-select",
                clipboard,
                Change::CopyOnSelect(!clipboard),
                cx,
            ),
        ));
    }
    if pane.matches(Category::Terminal, tr_key!("New pane directory")) {
        let selected = match pane.snapshot.settings.panes.new_pane_directory {
            NewPaneDirectory::Project => "project",
            NewPaneDirectory::Current => "current",
        };
        rows.push(pane.row(
            "directory",
            tr_key!("New pane directory"),
            controls::segmented(
                pane.style(),
                "directory",
                &[
                    Choice::new("project", tr!("Project")),
                    Choice::new("current", tr!("Current pane")),
                ],
                selected,
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::Directory(
                        if selected.as_ref() == "current" {
                            NewPaneDirectory::Current
                        } else {
                            NewPaneDirectory::Project
                        },
                    )));
                }),
            ),
        ));
    }
    if pane.matches(Category::Terminal, tr_key!("Ghostty configuration")) {
        rows.push(configuration(pane, cx));
    }
    if !pane.snapshot.terminal.diagnostics.is_empty()
        && pane.matches(Category::Terminal, tr_key!("Configuration warnings"))
    {
        rows.push(
            pane.note(
                &tr!(
                    "Muxy ignored the following unsupported Ghostty settings:\n\n%@",
                    pane.snapshot.terminal.diagnostics.join("\n")
                ),
                false,
            )
            .debug_selector(|| "settings-terminal-configuration-warnings".into())
            .into_any_element(),
        );
    }
    rows
}

fn configuration(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    pane.row(
        "ghostty-configuration",
        tr_key!("Ghostty configuration"),
        div()
            .flex()
            .flex_wrap()
            .gap(pane.metrics.spacing2())
            .child(
                controls::button(
                    pane.style(),
                    "edit-ghostty-configuration",
                    &tr!("Edit ghostty.conf"),
                    true,
                    cx.listener(|_, _, _, cx| {
                        cx.emit(SettingsEvent::OpenConfiguration("ghostty.conf"));
                    }),
                )
                .debug_selector(|| "settings-edit-ghostty-configuration".into()),
            )
            .child(
                controls::button(
                    pane.style(),
                    "reload-ghostty-configuration",
                    &tr!("Reload"),
                    true,
                    cx.listener(|_, _, _, cx| {
                        cx.emit(SettingsEvent::ReloadConfiguration);
                    }),
                )
                .debug_selector(|| "settings-reload-ghostty-configuration".into()),
            )
            .into_any_element(),
    )
}
