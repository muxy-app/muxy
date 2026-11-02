use gpui::prelude::FluentBuilder;
use gpui::{
    App, ClickEvent, Div, ElementId, InteractiveElement, ParentElement, Role, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window,
};

use crate::components::{ButtonInteraction, SymbolGlyph};
use crate::controls::Style;

/// A page in a list of pages. Clients read it as a tab, so the list holding it
/// needs `Role::TabList`.
pub fn item(
    style: Style,
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let Style { theme, metrics } = style;
    let id = id.into();
    gpui::div()
        .id(id.clone())
        .role(Role::Tab)
        .accessibility_id(id)
        .aria_label(name)
        .aria_selected(selected)
        .button_interaction(on_click)
        .flex()
        .items_center()
        .gap(metrics.spacing6())
        .px(metrics.spacing5())
        .h(metrics.spacing10())
        .rounded(metrics.scaled(5.0))
        .cursor_pointer()
        .text_size(metrics.font_headline())
        .text_color(if selected { theme.fg } else { theme.fg_muted })
        .when(selected, |row| row.bg(theme.surface))
        .hover(|row| row.bg(theme.hover))
        .focus(|row| row.bg(theme.accent_soft))
}

pub fn subitem(
    style: Style,
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let Style { theme, metrics } = style;
    item(style, id, name, false, on_click)
        .aria_selected(selected)
        .h_auto()
        .pl(metrics.scaled(34.0))
        .py(metrics.scaled(7.0))
        .rounded(metrics.radius_sm())
        .text_size(metrics.font_body())
        .when(selected, |row| {
            row.bg(theme.accent_soft).text_color(theme.accent)
        })
}

/// The chevron inside an [`item`]. A client can't reach a control nested in a
/// tab, so it stays out of the tree: give clients a hidden button beside the tab.
pub fn disclosure(
    style: Style,
    id: impl Into<ElementId>,
    expanded: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let Style { theme, metrics } = style;
    gpui::div()
        .id(id)
        .flex()
        .items_center()
        .h_full()
        .child(SymbolGlyph::new(
            if expanded {
                "chevron.down"
            } else {
                "chevron.right"
            },
            metrics.icon_xs(),
            theme.fg_muted,
        ))
        .on_click(move |event, window, cx| {
            on_click(event, window, cx);
            cx.stop_propagation();
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorScheme, Metrics, Theme};
    use gpui::Element;
    use gpui::accesskit::Node;

    #[test]
    fn a_subitem_publishes_its_own_selection_though_it_never_draws_as_an_item() {
        let theme = Theme::from_scheme(&ColorScheme::default());
        let metrics = Metrics::new(1.0);
        let style = Style {
            theme: &theme,
            metrics: &metrics,
        };
        let row = subitem(style, "settings-subcategory", "Fonts", true, |_, _, _| {});
        let mut node = Node::new(Role::Unknown);
        row.write_a11y_info(&mut node);
        assert_eq!(row.a11y_role(), Some(Role::Tab));
        assert_eq!(node.label(), Some("Fonts"));
        assert_eq!(node.author_id(), Some("settings-subcategory"));
        assert_eq!(node.is_selected(), Some(true));
    }
}
