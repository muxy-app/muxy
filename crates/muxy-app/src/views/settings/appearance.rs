use super::{Category, Change, PickerKind, SettingsEvent, SettingsView, Snapshot};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement};
use muxy_app_core::localization;
use muxy_app_core::settings::{CloseBehavior, SidebarCollapsedStyle};
use muxy_ui::controls::{self, Choice};
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;

#[allow(
    clippy::too_many_lines,
    reason = "Declarative appearance settings rows"
)]
pub(super) fn rows(
    pane: &SettingsView,
    category: Category,
    cx: &mut Context<SettingsView>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if category == Category::General
        && pane.matches(category, tr_key!("When closing tabs or panes"))
    {
        let selected = match pane.snapshot.settings.window.close_behavior {
            CloseBehavior::CloseSession => "close",
            CloseBehavior::Detach => "detach",
        };
        rows.push(pane.row(
            "close-behavior",
            tr_key!("When closing tabs or panes"),
            controls::segmented(
                pane.style(),
                "close-behavior",
                &[
                    Choice::new("close", tr!("Close sessions")),
                    Choice::new("detach", tr!("Detach")),
                ],
                selected,
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::CloseBehavior(
                        if selected.as_ref() == "detach" {
                            CloseBehavior::Detach
                        } else {
                            CloseBehavior::CloseSession
                        },
                    )));
                }),
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("App language")) {
        rows.push(app_language(pane, cx));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("More languages")) {
        rows.push(
            pane.row(
                "more-languages",
                tr_key!("More languages"),
                controls::button(
                    pane.style(),
                    "browse-languages",
                    &tr!("Browse language extensions…"),
                    true,
                    cx.listener(|pane, _, _, cx| pane.browse_languages(cx)),
                )
                .debug_selector(|| "settings-browse-languages".into())
                .into_any_element(),
            ),
        );
    }
    let appearance = &pane.snapshot.settings.appearance;
    for (dark, label, value) in [
        (false, tr_key!("Light theme"), &appearance.light_theme),
        (true, tr_key!("Dark theme"), &appearance.dark_theme),
    ] {
        if category == Category::Appearance && pane.matches(category, label) {
            rows.push(pane.row(
                if dark { "dark-theme" } else { "light-theme" },
                label,
                pane.picker(PickerKind::Theme(dark), value, cx),
            ));
        }
    }
    for (id, label, value, change) in [
        (
            "sidebar",
            tr_key!("Expand sidebar"),
            appearance.sidebar_expanded,
            Change::Sidebar(!appearance.sidebar_expanded),
        ),
        (
            "status-bar",
            tr_key!("Show status bar"),
            appearance.status_bar_visible,
            Change::StatusBar(!appearance.status_bar_visible),
        ),
        (
            "confirm-process",
            tr_key!("Confirm before closing a running process"),
            pane.snapshot.settings.window.confirm_running_process,
            Change::ConfirmProcess(!pane.snapshot.settings.window.confirm_running_process),
        ),
    ] {
        let target = if id == "confirm-process" {
            Category::General
        } else {
            Category::Appearance
        };
        if category == target && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.toggle(id, value, change, cx)));
        }
    }
    if category == Category::Appearance
        && pane.matches(category, tr_key!("Collapsed sidebar style"))
    {
        rows.push(collapsed_sidebar_style(pane, cx));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Sidebar vibrancy")) {
        rows.push(pane.row(
            "sidebar-vibrancy",
            tr_key!("Sidebar vibrancy"),
            pane.toggle(
                "sidebar-vibrancy",
                appearance.sidebar_vibrancy,
                Change::SidebarVibrancy(!appearance.sidebar_vibrancy),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Sidebar vibrancy level"))
    {
        rows.push(pane.row(
            "sidebar-vibrancy-level",
            tr_key!("Sidebar vibrancy level"),
            pane.field("sidebar-vibrancy-level"),
        ));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Show tips")) {
        rows.push(pane.row(
            "tips",
            tr_key!("Show tips"),
            pane.toggle(
                "tips",
                appearance.tips_visible,
                Change::Tips(!appearance.tips_visible),
                cx,
            ),
        ));
    }
    for (id, label, value) in [
        (
            "auto-expand-worktrees",
            tr_key!("Expand worktrees when switching projects"),
            appearance.auto_expand_worktrees,
        ),
        (
            "worktree-order",
            tr_key!("Order worktrees by recent use"),
            appearance.worktree_order_by_mru,
        ),
        (
            "worktree-unread",
            tr_key!("Show unread worktree indicators"),
            appearance.worktree_show_unread,
        ),
    ] {
        if category == Category::Appearance && pane.matches(category, label) {
            rows.push(pane.row(
                id,
                label,
                pane.toggle(id, value, Change::Worktrees(id, !value), cx),
            ));
        }
    }
    let sidebars = &pane.snapshot.sidebars;
    if category == Category::Appearance
        && !sidebars.is_empty()
        && pane.matches(category, tr_key!("Active sidebar"))
    {
        let built_in = tr!("Built-in");
        let selected = sidebars
            .iter()
            .find(|(id, _)| *id == appearance.extension_sidebar)
            .map_or(built_in.as_str(), |(_, label)| label.as_str());
        rows.push(pane.row(
            "extension-sidebar",
            tr_key!("Active sidebar"),
            pane.picker(PickerKind::ExtensionSidebar, selected, cx),
        ));
    }
    if category == Category::General && pane.matches(category, tr_key!("Open files with")) {
        rows.push(pane.row(
            "file-opener",
            tr_key!("Open files with"),
            pane.picker(PickerKind::FileOpener, &file_opener(&pane.snapshot), cx),
        ));
    }
    for (id, label) in [
        ("width", tr_key!("Default window width")),
        ("height", tr_key!("Default window height")),
        (
            "worktree-template",
            tr_key!("Default worktree path template"),
        ),
        ("worktree-folder", tr_key!("Default worktree folder")),
    ] {
        if category == Category::General && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.field(id)));
        }
    }
    rows
}

/// Built-in openers for files clicked in a terminal, as `(setting value, label key)`.
pub(super) const FILE_OPENERS: [(&str, &str); 3] = [
    ("system.editor", tr_key!("Project editor")),
    ("system.finder", "Finder"),
    ("system.application", tr_key!("Default application")),
];

/// The chosen file opener's label. A chosen extension opener that is no
/// longer enabled stays chosen, marked unavailable.
fn file_opener(snapshot: &Snapshot) -> String {
    let value = &snapshot.settings.openers.file;
    FILE_OPENERS
        .iter()
        .map(|&(id, label)| (id, translate(label)))
        .chain(
            snapshot
                .file_openers
                .iter()
                .map(|(id, label)| (id.as_str(), label.clone().into())),
        )
        .find(|(id, _)| id == value)
        .map_or_else(
            || unavailable_file_opener(value),
            |(_, label)| label.to_string(),
        )
}

pub(super) fn unavailable_file_opener(value: &str) -> String {
    match value.split_once(':') {
        Some((extension, opener)) => tr!("%@ (%@, unavailable)", extension, opener).to_string(),
        None => tr!("%@ (unavailable)", value).to_string(),
    }
}

/// The app language. While the chosen language's extension is unavailable,
/// the row says English shows meanwhile.
fn app_language(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let selected = &pane.snapshot.settings.appearance.language;
    let unavailable =
        !selected.is_empty() && !pane.snapshot.languages.iter().any(|(id, _)| id == selected);
    let note = unavailable.then(|| {
        tr!("The selected language extension is unavailable, so Muxy is temporarily using English.")
    });
    pane.row_with_description(
        "app-language",
        tr_key!("App language"),
        note.as_ref().map(AsRef::as_ref),
        pane.picker(
            PickerKind::AppLanguage,
            &language_label(&pane.snapshot, selected),
            cx,
        ),
        pane.compact,
    )
}

/// The label of a language selection: English, a provider, or an
/// unavailable one.
pub(super) fn language_label(snapshot: &Snapshot, selected: &str) -> String {
    if selected.is_empty() {
        return tr!("English").to_string();
    }
    snapshot
        .languages
        .iter()
        .find(|(id, _)| id == selected)
        .map_or_else(
            || match localization::parse_selection(selected) {
                Some((extension, id)) => tr!("%@ (%@, unavailable)", extension, id).to_string(),
                None => tr!("Unavailable language").to_string(),
            },
            |(_, label)| label.clone(),
        )
}

fn collapsed_sidebar_style(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let appearance = &pane.snapshot.settings.appearance;
    pane.row(
        "sidebar-collapsed-style",
        tr_key!("Collapsed sidebar style"),
        controls::segmented(
            pane.style(),
            "sidebar-collapsed-style",
            &[
                Choice::new("icons", tr!("Icons")),
                Choice::new("hidden", tr!("Hidden")),
            ],
            match appearance.sidebar_collapsed_style {
                SidebarCollapsedStyle::Icons => "icons",
                SidebarCollapsedStyle::Hidden => "hidden",
            },
            cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                cx.emit(SettingsEvent::Change(Change::SidebarCollapsedStyle(
                    if selected.as_ref() == "hidden" {
                        SidebarCollapsedStyle::Hidden
                    } else {
                        SidebarCollapsedStyle::Icons
                    },
                )));
            }),
        ),
    )
}
