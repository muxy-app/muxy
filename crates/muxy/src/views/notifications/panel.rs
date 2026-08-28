use crate::state::AppState;
use crate::views::window::MainWindow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Bounds, Context, ElementId, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, SharedString, Size,
    StatefulInteractiveElement, Styled, actions, div, px,
};
use muxy_core::notifications::{NotificationRecord, NotificationSource};
use muxy_ui::components::{IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;

pub const KEY_CONTEXT: &str = "NotificationPanel";
actions!(notification_panel, [Dismiss, Activate]);

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let context = Some(KEY_CONTEXT);
    vec![
        gpui::KeyBinding::new("escape", Dismiss, context),
        gpui::KeyBinding::new("enter", Activate, context),
    ]
}

pub const PANEL_WIDTH: f32 = 320.0;
pub const PANEL_HEIGHT: f32 = 400.0;
pub const TITLE_LINES: usize = 1;
pub const BODY_LINES: usize = 2;
pub const UNREAD_DOT_SIZE: f32 = 6.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIcon {
    Terminal,
    Provider(String),
    Network,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowModel {
    pub id: String,
    pub source_icon: SourceIcon,
    pub title: String,
    pub body: Option<String>,
    pub relative_time: String,
    pub unread: bool,
    pub accessibility_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanelModel {
    pub title: &'static str,
    pub clear_all_visible: bool,
    pub empty_message: Option<&'static str>,
    pub rows: Vec<RowModel>,
}

pub fn relative_time(timestamp: f64, now: f64) -> String {
    let elapsed = (now - timestamp).max(0.0);
    if elapsed < 60.0 {
        "now".to_owned()
    } else if elapsed < 3_600.0 {
        format!("{}m", (elapsed / 60.0).floor() as u64)
    } else if elapsed < 86_400.0 {
        format!("{}h", (elapsed / 3_600.0).floor() as u64)
    } else {
        format!("{}d", (elapsed / 86_400.0).floor() as u64)
    }
}

pub fn model(records: &[NotificationRecord], now: f64) -> PanelModel {
    let rows = records
        .iter()
        .map(|record| {
            let body = (!record.body.is_empty()).then(|| record.body.clone());
            let relative_time = relative_time(record.timestamp, now);
            let read_state = if record.is_read { "read" } else { "unread" };
            let accessibility_label = match body.as_deref() {
                Some(body) => format!("{}, {body}, {relative_time}, {read_state}", record.title),
                None => format!("{}, {relative_time}, {read_state}", record.title),
            };
            RowModel {
                id: record.id.clone(),
                source_icon: source_icon(&record.source),
                title: record.title.clone(),
                body,
                relative_time,
                unread: !record.is_read,
                accessibility_label,
            }
        })
        .collect();
    PanelModel {
        title: "Notifications",
        clear_all_visible: !records.is_empty(),
        empty_message: records.is_empty().then_some("No notifications"),
        rows,
    }
}

fn source_icon(source: &NotificationSource) -> SourceIcon {
    match source {
        NotificationSource::Osc => SourceIcon::Terminal,
        NotificationSource::AiProvider { provider_id } => SourceIcon::Provider(provider_id.clone()),
        NotificationSource::Socket => SourceIcon::Network,
    }
}

pub fn panel_origin(
    anchor: Bounds<Pixels>,
    panel_size: Size<Pixels>,
    viewport: Size<Pixels>,
    margin: Pixels,
    gap: Pixels,
) -> Point<Pixels> {
    let x = crate::views::repository::branch::clamp_axis(
        f32::from(anchor.origin.x),
        f32::from(panel_size.width),
        f32::from(viewport.width),
        f32::from(margin),
    );
    let y = crate::views::repository::branch::clamp_axis(
        f32::from(anchor.origin.y - panel_size.height - gap),
        f32::from(panel_size.height),
        f32::from(viewport.height),
        f32::from(margin),
    );
    gpui::point(px(x), px(y))
}

pub fn render(
    panel: PanelModel,
    origin: Point<Pixels>,
    focus: &FocusHandle,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(metrics.spacing5())
        .py(metrics.spacing4())
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(metrics.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg)
                .child(SharedString::from(panel.title)),
        );
    if panel.clear_all_visible {
        header = header.child(
            div()
                .id("notification-clear-all")
                .tab_index(0)
                .cursor_pointer()
                .text_size(metrics.font_footnote())
                .text_color(theme.accent)
                .on_click(cx.listener(|window: &mut MainWindow, _, _, cx| {
                    window.clear_notifications(cx);
                }))
                .child("Clear All"),
        );
    }

    let mut content = div()
        .id("notification-scroll")
        .flex()
        .flex_col()
        .flex_grow()
        .min_h(px(0.0))
        .overflow_y_scroll();
    if let Some(empty_message) = panel.empty_message {
        content = content.child(
            div()
                .flex()
                .flex_col()
                .flex_grow()
                .items_center()
                .justify_center()
                .gap(metrics.spacing4())
                .text_color(theme.fg_muted)
                .child(SymbolGlyph::new(
                    "bell.slash",
                    metrics.icon_xl(),
                    theme.fg_dim,
                ))
                .child(
                    div()
                        .text_size(metrics.font_body())
                        .child(SharedString::from(empty_message)),
                ),
        );
    } else {
        for (index, row) in panel.rows.into_iter().enumerate() {
            content = content.child(notification_row(row, index, state, cx));
        }
    }

    div()
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(metrics.scaled(PANEL_WIDTH))
        .h(metrics.scaled(PANEL_HEIGHT))
        .occlude()
        .key_context(KEY_CONTEXT)
        .track_focus(focus)
        .on_action(cx.listener(|window: &mut MainWindow, _: &Dismiss, _, cx| {
            window.dismiss_overlay(cx);
        }))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(metrics.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(header)
        .child(content)
        .into_any_element()
}

fn notification_row(
    row: RowModel,
    index: usize,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let group = SharedString::from(format!("notification-row-{}", row.id));
    let navigation_id = row.id.clone();
    let action_id = row.id.clone();
    let remove_id = row.id.clone();
    let icon = source_icon_element(&row.source_icon, state);
    let mut copy = div()
        .flex()
        .flex_col()
        .flex_grow()
        .min_w(px(0.0))
        .gap(metrics.spacing1())
        .child(
            div()
                .line_clamp(TITLE_LINES)
                .text_size(metrics.font_body())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.fg)
                .child(SharedString::from(row.title)),
        );
    if let Some(body) = row.body {
        copy = copy.child(
            div()
                .line_clamp(BODY_LINES)
                .text_size(metrics.font_footnote())
                .text_color(theme.fg_muted)
                .child(SharedString::from(body)),
        );
    }
    copy = copy.child(
        div()
            .text_size(metrics.font_caption())
            .text_color(theme.fg_dim)
            .child(SharedString::from(row.relative_time)),
    );

    div()
        .id(ElementId::Name(group.clone()))
        .group(group.clone())
        .tab_index(index as isize + 1)
        .relative()
        .flex()
        .flex_row()
        .items_start()
        .gap(metrics.spacing3())
        .px(metrics.spacing4())
        .py(metrics.spacing3())
        .cursor_pointer()
        .border_b_1()
        .border_color(theme.border)
        .hover(|style| style.bg(theme.hover))
        .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
            window.activate_notification_row(&navigation_id, cx);
        }))
        .on_action(
            cx.listener(move |window: &mut MainWindow, _: &Activate, _, cx| {
                window.activate_notification_row(&action_id, cx);
            }),
        )
        .child(
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(metrics.icon_xl())
                .child(icon)
                .when(row.unread, |element| {
                    element.child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .size(metrics.scaled(UNREAD_DOT_SIZE))
                            .rounded_full()
                            .bg(theme.accent),
                    )
                }),
        )
        .child(copy)
        .child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "notification-remove-{remove_id}"
                ))))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(metrics.icon_lg())
                .opacity(0.0)
                .group_hover(group, |style| style.opacity(1.0))
                .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                    cx.stop_propagation();
                    window.remove_notification(&remove_id, cx);
                }))
                .child(IconGlyph::new(
                    Icon::X,
                    metrics.font_caption(),
                    theme.fg_muted,
                )),
        )
        .child(
            div()
                .absolute()
                .size(px(1.0))
                .opacity(0.0)
                .child(SharedString::from(row.accessibility_label)),
        )
        .into_any_element()
}

fn source_icon_element(icon: &SourceIcon, state: &AppState) -> AnyElement {
    let size = state.metrics.icon_md();
    let color = state.theme.fg_muted;
    match icon {
        SourceIcon::Terminal => IconGlyph::new(Icon::Terminal, size, color).into_any_element(),
        SourceIcon::Network => IconGlyph::new(Icon::Network, size, color).into_any_element(),
        SourceIcon::Provider(provider_id) => {
            let Some(provider) = muxy_core::repository_ai::provider(provider_id) else {
                return IconGlyph::new(Icon::Terminal, size, color).into_any_element();
            };
            gpui::svg()
                .path(SharedString::from(format!(
                    "icons/providers/{}.svg",
                    provider.icon_key
                )))
                .size(size)
                .text_color(color)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::notifications::{NotificationRecord, NotificationTarget};

    fn record(
        id: &str,
        source: NotificationSource,
        title: &str,
        body: &str,
        timestamp: f64,
        is_read: bool,
    ) -> NotificationRecord {
        NotificationRecord::with_id(
            id,
            NotificationTarget::new(
                "11111111-2222-4333-8444-555555555555",
                "22222222-3333-4444-8555-666666666666",
                "33333333-4444-4555-8666-777777777777",
                "44444444-5555-4666-8777-888888888888",
                "55555555-6666-4777-8888-999999999999",
                "/tmp/worktree",
            )
            .expect("target"),
            source,
            title,
            body,
            timestamp,
            is_read,
        )
        .expect("record")
    }

    #[test]
    fn notification_panel_dimensions_and_empty_state_are_exact() {
        assert_eq!((PANEL_WIDTH, PANEL_HEIGHT), (320.0, 400.0));
        assert_eq!((TITLE_LINES, BODY_LINES, UNREAD_DOT_SIZE), (1, 2, 6.0));
        assert_eq!(
            model(&[], 100.0),
            PanelModel {
                title: "Notifications",
                clear_all_visible: false,
                empty_message: Some("No notifications"),
                rows: Vec::new(),
            }
        );
    }

    #[test]
    fn notification_panel_rows_preserve_newest_order_sources_and_accessibility() {
        let records = vec![
            record(
                "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                NotificationSource::Osc,
                "Newest",
                "Terminal body",
                100.0,
                false,
            ),
            record(
                "BBBBBBBB-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                NotificationSource::AiProvider {
                    provider_id: "codex".to_owned(),
                },
                "Provider",
                "",
                40.0,
                true,
            ),
            record(
                "CCCCCCCC-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                NotificationSource::Socket,
                "Socket",
                "Network body",
                -86_400.0,
                false,
            ),
        ];
        let panel = model(&records, 100.0);
        assert_eq!(panel.title, "Notifications");
        assert!(panel.clear_all_visible);
        assert_eq!(panel.empty_message, None);
        assert_eq!(
            panel
                .rows
                .iter()
                .map(|row| row.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Newest", "Provider", "Socket"]
        );
        assert_eq!(panel.rows[0].source_icon, SourceIcon::Terminal);
        assert_eq!(
            panel.rows[1].source_icon,
            SourceIcon::Provider("codex".to_owned())
        );
        assert_eq!(panel.rows[2].source_icon, SourceIcon::Network);
        assert_eq!(panel.rows[0].relative_time, "now");
        assert_eq!(panel.rows[1].relative_time, "1m");
        assert_eq!(panel.rows[2].relative_time, "1d");
        assert_eq!(panel.rows[0].body.as_deref(), Some("Terminal body"));
        assert_eq!(panel.rows[1].body, None);
        assert!(panel.rows[0].unread);
        assert!(!panel.rows[1].unread);
        assert_eq!(
            panel.rows[0].accessibility_label,
            "Newest, Terminal body, now, unread"
        );
        assert_eq!(panel.rows[1].accessibility_label, "Provider, 1m, read");
    }

    #[test]
    fn notification_panel_relative_time_clamps_future_and_uses_minutes_hours_days() {
        assert_eq!(relative_time(101.0, 100.0), "now");
        assert_eq!(relative_time(41.0, 100.0), "now");
        assert_eq!(relative_time(40.0, 100.0), "1m");
        assert_eq!(relative_time(100.0 - 7_200.0, 100.0), "2h");
        assert_eq!(relative_time(100.0 - 259_200.0, 100.0), "3d");
    }

    #[test]
    fn notification_panel_origin_stays_inside_viewport_above_footer_anchor() {
        let origin = panel_origin(
            Bounds::new(
                gpui::point(px(30.0), px(580.0)),
                gpui::size(px(24.0), px(24.0)),
            ),
            gpui::size(px(320.0), px(400.0)),
            gpui::size(px(800.0), px(640.0)),
            px(16.0),
            px(8.0),
        );
        assert_eq!(origin, gpui::point(px(30.0), px(172.0)));
        let clamped = panel_origin(
            Bounds::new(
                gpui::point(px(790.0), px(20.0)),
                gpui::size(px(24.0), px(24.0)),
            ),
            gpui::size(px(320.0), px(400.0)),
            gpui::size(px(800.0), px(640.0)),
            px(16.0),
            px(8.0),
        );
        assert_eq!(clamped, gpui::point(px(464.0), px(16.0)));
    }

    #[test]
    fn notification_panel_source_mapping_is_total() {
        assert_eq!(source_icon(&NotificationSource::Osc), SourceIcon::Terminal);
        assert_eq!(
            source_icon(&NotificationSource::Socket),
            SourceIcon::Network
        );
        assert_eq!(
            source_icon(&NotificationSource::AiProvider {
                provider_id: "xal".to_owned(),
            }),
            SourceIcon::Provider("xal".to_owned())
        );
    }
}
