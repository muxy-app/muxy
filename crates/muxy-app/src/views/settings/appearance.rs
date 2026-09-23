use super::{Category, Change, PickerKind, SettingsEvent, SettingsView};
use gpui::{AnyElement, Context};
use muxy_app_core::settings::{CloseBehavior, SidebarCollapsedStyle};
use muxy_ui::controls::{self, Choice};

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
    if category == Category::General && pane.matches(category, "When closing tabs or panes") {
        let selected = match pane.snapshot.settings.window.close_behavior {
            CloseBehavior::CloseSession => "close",
            CloseBehavior::Detach => "detach",
        };
        rows.push(pane.row(
            "close-behavior",
            "When closing tabs or panes",
            controls::segmented(
                pane.style(),
                "close-behavior",
                &[
                    Choice::new("close", "Close sessions"),
                    Choice::new("detach", "Detach"),
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
    let appearance = &pane.snapshot.settings.appearance;
    for (dark, label, value) in [
        (false, "Light theme", &appearance.light_theme),
        (true, "Dark theme", &appearance.dark_theme),
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
            "Expand sidebar",
            appearance.sidebar_expanded,
            Change::Sidebar(!appearance.sidebar_expanded),
        ),
        (
            "status-bar",
            "Show status bar",
            appearance.status_bar_visible,
            Change::StatusBar(!appearance.status_bar_visible),
        ),
        (
            "confirm-process",
            "Confirm before closing a running process",
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
    if category == Category::Appearance && pane.matches(category, "Collapsed sidebar style") {
        rows.push(collapsed_sidebar_style(pane, cx));
    }
    if category == Category::Appearance && pane.matches(category, "Sidebar vibrancy") {
        rows.push(pane.row(
            "sidebar-vibrancy",
            "Sidebar vibrancy",
            pane.toggle(
                "sidebar-vibrancy",
                appearance.sidebar_vibrancy,
                Change::SidebarVibrancy(!appearance.sidebar_vibrancy),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, "Sidebar vibrancy level") {
        rows.push(pane.row(
            "sidebar-vibrancy-level",
            "Sidebar vibrancy level",
            pane.field("sidebar-vibrancy-level"),
        ));
    }
    for (id, label, value) in [
        (
            "auto-expand-worktrees",
            "Expand worktrees when switching projects",
            appearance.auto_expand_worktrees,
        ),
        (
            "worktree-order",
            "Order worktrees by recent use",
            appearance.worktree_order_by_mru,
        ),
        (
            "worktree-unread",
            "Show unread worktree indicators",
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
        && pane.matches(category, "Active sidebar")
    {
        let selected = sidebars
            .iter()
            .find(|(id, _)| *id == appearance.extension_sidebar)
            .map_or("Built-in", |(_, label)| label.as_str());
        rows.push(pane.row(
            "extension-sidebar",
            "Active sidebar",
            pane.picker(PickerKind::ExtensionSidebar, selected, cx),
        ));
    }
    for (id, label) in [
        ("width", "Default window width"),
        ("height", "Default window height"),
    ] {
        if category == Category::General && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.field(id)));
        }
    }
    rows
}

fn collapsed_sidebar_style(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let appearance = &pane.snapshot.settings.appearance;
    pane.row(
        "sidebar-collapsed-style",
        "Collapsed sidebar style",
        controls::segmented(
            pane.style(),
            "sidebar-collapsed-style",
            &[
                Choice::new("icons", "Icons"),
                Choice::new("hidden", "Hidden"),
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
