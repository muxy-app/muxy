use crate::command::Command;
use crate::state::AppState;
use crate::views::swatches::{icon_color, icon_foreground};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, px,
};
use muxy_core::store::Project;
use muxy_core::store::worktrees::{Source, Worktree};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

const MONOSPACE_FONT: &str = "Menlo";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeRowKind {
    Primary,
    Managed,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeRowModel {
    pub id: String,
    pub label: String,
    pub branch: Option<String>,
    pub kind: WorktreeRowKind,
    pub active: bool,
}

pub fn worktree_row_models(
    worktrees: &[Worktree],
    active_worktree_id: Option<&str>,
    expanded: bool,
) -> Vec<WorktreeRowModel> {
    if !expanded {
        return Vec::new();
    }
    worktrees
        .iter()
        .map(|worktree| WorktreeRowModel {
            id: worktree.id.clone(),
            label: worktree.name.clone(),
            branch: worktree.branch.clone(),
            kind: if worktree.is_primary {
                WorktreeRowKind::Primary
            } else if worktree.source == Source::Muxy {
                WorktreeRowKind::Managed
            } else {
                WorktreeRowKind::External
            },
            active: active_worktree_id.is_some_and(|id| worktree.id.eq_ignore_ascii_case(id)),
        })
        .collect()
}

pub fn worktree_header_command(project_id: &str, active: bool, has_worktrees: bool) -> Command {
    if active && has_worktrees {
        Command::ToggleWorktreeExpansion(project_id.to_owned())
    } else {
        Command::SelectProject(project_id.to_owned())
    }
}

pub fn project_tile(state: &AppState, project: &Project, group: SharedString) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let size = metrics.icon_xxl();

    let (background, hover_background): (Hsla, Hsla) = if project.is_home() {
        (theme.accent, with_opacity(theme.accent, 0.85))
    } else if let Some(tint) = icon_color(project.icon_color.as_deref()) {
        let tint: Hsla = tint.into();
        (tint, with_opacity(tint, 0.85))
    } else {
        (theme.fg_alpha(0.18), theme.fg_alpha(0.22))
    };

    let foreground: Hsla = if project.is_home() {
        theme.accent_foreground
    } else if let Some(tint) = icon_foreground(project.icon_color.as_deref()) {
        tint.into()
    } else if state.is_active(project) {
        theme.fg
    } else {
        theme.fg_muted
    };

    let logo = project
        .logo
        .as_deref()
        .and_then(muxy_core::store::logo::logo_path)
        .filter(|path| path.exists());

    if let Some(path) = logo {
        return div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(metrics.radius_md())
            .overflow_hidden()
            .child(gpui::img(path).size(size))
            .into_any_element();
    }

    let glyph = project
        .icon
        .clone()
        .map(|symbol| {
            muxy_ui::components::SymbolGlyph::new(symbol, metrics.font_title_large(), foreground)
                .into_any_element()
        })
        .unwrap_or_else(|| {
            div()
                .text_size(metrics.font_emphasis())
                .font_weight(FontWeight::BOLD)
                .text_color(foreground)
                .child(SharedString::from(project.display_letter()))
                .into_any_element()
        });

    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(metrics.radius_md())
        .bg(background)
        .group_hover(group, |style| style.bg(hover_background))
        .child(glyph)
        .into_any_element()
}

fn with_opacity(color: Hsla, opacity: f32) -> Hsla {
    Hsla {
        a: color.a * opacity,
        ..color
    }
}

pub fn collapsed_row(state: &AppState, project: &Project) -> AnyElement {
    let metrics = &state.metrics;
    let inset = metrics.scaled(3.0);
    let is_active = state.is_active(project);
    let group = SharedString::from(format!("project-{}", project.id));

    let outer = metrics.icon_xxl() + inset + inset;

    let ring = div()
        .absolute()
        .top_0()
        .left_0()
        .w(outer)
        .h(outer)
        .rounded(metrics.radius_md() + inset)
        .border(px(1.5))
        .border_color(if is_active {
            state.theme.accent
        } else {
            gpui::transparent_black()
        });

    div()
        .id(gpui::ElementId::Name(group.clone()))
        .group(group.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(outer)
        .h(outer)
        .cursor_pointer()
        .child(project_tile(state, project, group))
        .child(ring)
        .into_any_element()
}

pub fn expanded_row(state: &AppState, project: &Project, worktrees_expanded: bool) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let is_active = state.is_active(project);
    let group = SharedString::from(format!("project-{}", project.id));

    let mut label = div()
        .flex()
        .flex_col()
        .flex_grow()
        .min_w(px(0.0))
        .gap(metrics.scaled(1.0))
        .child(
            div()
                .text_size(metrics.font_emphasis())
                .font_weight(if is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(theme.fg)
                .truncate()
                .child(SharedString::from(project.name.clone())),
        );

    if project.has_worktree_ui()
        && let Some(worktree) = project.worktree_label.clone()
    {
        label = label.child(
            div()
                .font_family(MONOSPACE_FONT)
                .text_size(metrics.font_footnote())
                .text_color(theme.fg)
                .truncate()
                .child(SharedString::from(worktree)),
        );
    }

    let mut row = div()
        .id(gpui::ElementId::Name(group.clone()))
        .group(group.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .p(metrics.spacing2())
        .rounded(metrics.radius_lg())
        .cursor_pointer()
        .child(project_tile(state, project, group))
        .child(label);

    if project.has_worktree_ui() {
        row = row.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(metrics.scaled(18.0))
                .child(IconGlyph::new(
                    if worktrees_expanded {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    },
                    metrics.font_xs(),
                    theme.fg,
                )),
        );
    }

    if is_active {
        row = row.bg(theme.surface);
    } else {
        row = row.hover(|style| style.bg(theme.hover));
    }

    row.into_any_element()
}

pub fn worktree_row(state: &AppState, model: &WorktreeRowModel) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let dot = match model.kind {
        WorktreeRowKind::Primary => theme.accent,
        WorktreeRowKind::Managed => theme.fg,
        WorktreeRowKind::External => theme.fg_dim,
    };
    let mut label = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing2())
        .min_w(px(0.0))
        .child(
            div()
                .truncate()
                .text_size(metrics.font_body())
                .text_color(theme.fg)
                .child(SharedString::from(model.label.clone())),
        );
    if model.kind == WorktreeRowKind::Primary {
        label = label.child(
            div()
                .px(metrics.spacing2())
                .rounded(metrics.radius_sm())
                .bg(theme.surface)
                .text_size(metrics.font_micro())
                .font_weight(FontWeight::BOLD)
                .text_color(theme.fg)
                .child("PRIMARY"),
        );
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .px(metrics.spacing2())
        .py(metrics.scaled(7.0))
        .rounded(metrics.radius_md())
        .cursor_pointer()
        .when(model.active, |row| row.bg(theme.surface))
        .when(!model.active, |row| {
            row.hover(|style| style.bg(theme.hover))
        })
        .child(div().size(metrics.scaled(7.0)).rounded_full().bg(dot))
        .child(label)
        .into_any_element()
}

pub fn new_worktree_row(state: &AppState) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .px(metrics.spacing2())
        .py(metrics.scaled(5.0))
        .rounded(metrics.radius_md())
        .cursor_pointer()
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg)
        .hover(|style| style.bg(theme.hover).text_color(theme.accent))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(metrics.icon_xxl())
                .child(IconGlyph::new(Icon::Plus, metrics.font_caption(), theme.fg)),
        )
        .child("New Worktree")
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::store::worktrees::{Source, Worktree};

    fn worktree(id: &str, name: &str, source: Source, primary: bool) -> Worktree {
        Worktree {
            id: id.into(),
            name: name.into(),
            path: format!("/{name}"),
            branch: Some(name.into()),
            source,
            is_primary: primary,
            created_at: 1.0,
            last_active_at: None,
        }
    }

    #[test]
    fn worktree_rows_model_primary_managed_external_active_and_expanded_states() {
        let list = vec![
            worktree("PRIMARY", "Repo", Source::Muxy, true),
            worktree("MANAGED", "Feature", Source::Muxy, false),
            worktree("EXTERNAL", "Review", Source::External, false),
        ];
        assert!(worktree_row_models(&list, Some("MANAGED"), false).is_empty());
        let rows = worktree_row_models(&list, Some("MANAGED"), true);
        assert_eq!(rows[0].kind, WorktreeRowKind::Primary);
        assert_eq!(rows[0].label, "Repo");
        assert_eq!(rows[1].kind, WorktreeRowKind::Managed);
        assert!(rows[1].active);
        assert_eq!(rows[2].kind, WorktreeRowKind::External);
        assert!(!rows[2].active);
    }

    #[test]
    fn worktree_rows_active_header_toggles_and_inactive_header_selects() {
        assert_eq!(
            worktree_header_command("PROJECT", true, true),
            Command::ToggleWorktreeExpansion("PROJECT".into())
        );
        assert_eq!(
            worktree_header_command("PROJECT", false, true),
            Command::SelectProject("PROJECT".into())
        );
    }
}
