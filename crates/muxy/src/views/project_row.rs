use crate::state::AppState;
use crate::views::swatches::{icon_color, icon_foreground};
use gpui::{
    AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, px,
};
use muxy_core::store::Project;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

const MONOSPACE_FONT: &str = "Menlo";

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

pub fn expanded_row(state: &AppState, project: &Project) -> AnyElement {
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
                    Icon::ChevronRight,
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
