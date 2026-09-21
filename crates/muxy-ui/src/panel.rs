use crate::components::{IconGlyph, SymbolGlyph, Tooltip};
use crate::icon::Icon;
use crate::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, DispatchPhase, ElementId, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    canvas, div, px,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelId(String);

impl PanelId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PanelId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PanelId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub enum PanelPosition {
    Right,
    Bottom,
}

impl PanelPosition {
    pub fn moved(self) -> Self {
        match self {
            Self::Right => Self::Bottom,
            Self::Bottom => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[must_use]
pub enum PanelMode {
    Pinned,
    Floating,
}

impl PanelMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Pinned => Self::Floating,
            Self::Floating => Self::Pinned,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelPlacement {
    pub id: PanelId,
    pub position: PanelPosition,
    pub mode: PanelMode,
}

impl PanelPlacement {
    pub fn new(id: impl Into<PanelId>, position: PanelPosition, mode: PanelMode) -> Self {
        Self {
            id: id.into(),
            position,
            mode,
        }
    }

    pub fn slot(&self) -> PanelSlot {
        PanelSlot {
            position: self.position,
            mode: self.mode,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelSlot {
    pub position: PanelPosition,
    pub mode: PanelMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelDisplacement {
    pub displaced: PanelPlacement,
    pub replacement: PanelPlacement,
}

#[derive(Debug, Default)]
pub struct PanelHost {
    slots: BTreeMap<PanelSlot, PanelId>,
    placements: BTreeMap<PanelId, PanelPlacement>,
}

impl PanelHost {
    pub fn place(&mut self, placement: PanelPlacement) -> Option<PanelDisplacement> {
        self.placements.remove(&placement.id);
        let displaced = self.placements.pop_first().map(|(_, panel)| panel);
        self.slots.clear();
        self.slots.insert(placement.slot(), placement.id.clone());
        self.placements
            .insert(placement.id.clone(), placement.clone());

        displaced.map(|displaced| PanelDisplacement {
            displaced,
            replacement: placement,
        })
    }

    pub fn remove(&mut self, id: &PanelId) -> Option<PanelPlacement> {
        let placement = self.placements.remove(id)?;
        self.slots.remove(&placement.slot());
        Some(placement)
    }

    pub fn placement(&self, id: &PanelId) -> Option<&PanelPlacement> {
        self.placements.get(id)
    }

    pub fn occupant(&self, slot: PanelSlot) -> Option<&PanelId> {
        self.slots.get(&slot)
    }

    pub fn placements(&self) -> impl Iterator<Item = &PanelPlacement> {
        self.placements.values()
    }

    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }

    pub fn len(&self) -> usize {
        self.placements.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelSizeBounds {
    pub minimum: f32,
    pub maximum: f32,
}

impl PanelSizeBounds {
    pub fn new(minimum: f32, maximum: f32) -> Self {
        let minimum = finite_or_zero(minimum).max(0.0);
        let maximum = finite_or_zero(maximum).max(minimum);
        Self { minimum, maximum }
    }

    pub fn clamp(self, dimension: f32) -> f32 {
        if dimension.is_finite() {
            dimension.clamp(self.minimum, self.maximum)
        } else {
            self.minimum
        }
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLayout {
    pub position: PanelPosition,
    pub mode: PanelMode,
    dimension: f32,
}

impl PanelLayout {
    pub fn new(
        position: PanelPosition,
        mode: PanelMode,
        dimension: f32,
        bounds: PanelSizeBounds,
    ) -> Self {
        Self {
            position,
            mode,
            dimension: bounds.clamp(dimension),
        }
    }

    pub fn dimension(self) -> f32 {
        self.dimension
    }

    pub fn consumed_width(self) -> f32 {
        if self.mode == PanelMode::Pinned && self.position == PanelPosition::Right {
            self.dimension
        } else {
            0.0
        }
    }

    pub fn consumed_height(self) -> f32 {
        if self.mode == PanelMode::Pinned && self.position == PanelPosition::Bottom {
            self.dimension
        } else {
            0.0
        }
    }

    pub fn overlays_workspace(self) -> bool {
        self.mode == PanelMode::Floating
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelResize {
    position: PanelPosition,
    start_dimension: f32,
    start_pointer: Point<f32>,
    bounds: PanelSizeBounds,
}

impl PanelResize {
    pub fn new(
        position: PanelPosition,
        start_dimension: f32,
        start_pointer: Point<f32>,
        bounds: PanelSizeBounds,
    ) -> Self {
        Self {
            position,
            start_dimension: bounds.clamp(start_dimension),
            start_pointer,
            bounds,
        }
    }

    pub fn dimension_at(self, pointer: Point<f32>) -> f32 {
        let delta = match self.position {
            PanelPosition::Right => self.start_pointer.x - pointer.x,
            PanelPosition::Bottom => self.start_pointer.y - pointer.y,
        };
        self.bounds.clamp(self.start_dimension + delta)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PanelResizeState {
    active: Rc<RefCell<Option<PanelResize>>>,
}

impl PanelResizeState {
    pub fn begin(&self, resize: PanelResize) {
        self.active.borrow_mut().replace(resize);
    }

    pub fn dimension_at(&self, pointer: Point<f32>) -> Option<f32> {
        self.active
            .borrow()
            .as_ref()
            .map(|resize| resize.dimension_at(pointer))
    }

    pub fn end(&self) -> bool {
        self.active.borrow_mut().take().is_some()
    }

    pub fn is_active(&self) -> bool {
        self.active.borrow().is_some()
    }
}

#[derive(Debug, Clone)]
pub struct PanelSizing {
    layout: PanelLayout,
    bounds: PanelSizeBounds,
    resize_state: PanelResizeState,
}

impl PanelSizing {
    pub fn new(
        placement: &PanelPlacement,
        dimension: f32,
        bounds: PanelSizeBounds,
        resize_state: PanelResizeState,
    ) -> Self {
        Self {
            layout: PanelLayout::new(placement.position, placement.mode, dimension, bounds),
            bounds,
            resize_state,
        }
    }

    pub fn layout(&self) -> PanelLayout {
        self.layout
    }

    pub fn resize_state(&self) -> &PanelResizeState {
        &self.resize_state
    }
}

#[derive(Debug, Clone)]
pub struct PanelStyle {
    pub theme: Theme,
    pub metrics: Metrics,
}

impl PanelStyle {
    pub fn new(theme: Theme, metrics: Metrics) -> Self {
        Self { theme, metrics }
    }
}

type PanelActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

enum PanelActionContent {
    Glyph(SharedString),
    Symbol(SharedString),
    Icon(Icon),
    Element(AnyElement),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelControl {
    Move(PanelPosition),
    Mode(PanelMode),
    Close,
}

impl PanelControl {
    fn appearance(self) -> (&'static str, &'static str) {
        match self {
            Self::Move(PanelPosition::Right) => {
                ("Move to Bottom", "rectangle.bottomhalf.inset.filled")
            }
            Self::Move(PanelPosition::Bottom) => {
                ("Move to Right", "rectangle.righthalf.inset.filled")
            }
            Self::Mode(PanelMode::Pinned) => ("Float Panel", "pin.slash"),
            Self::Mode(PanelMode::Floating) => ("Dock Panel", "pin"),
            Self::Close => ("Close", "xmark"),
        }
    }
}

#[must_use]
pub struct PanelAction {
    id: ElementId,
    label: SharedString,
    content: PanelActionContent,
    focus_handle: FocusHandle,
    handler: PanelActionHandler,
    selected: bool,
}

impl PanelAction {
    pub fn control(
        id: impl Into<ElementId>,
        control: PanelControl,
        focus_handle: FocusHandle,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        let (label, symbol) = control.appearance();
        Self::symbol(id, label, symbol, focus_handle, handler)
    }

    pub fn symbol(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        symbol: impl Into<SharedString>,
        focus_handle: FocusHandle,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: PanelActionContent::Symbol(symbol.into()),
            focus_handle: focus_handle.tab_stop(true),
            handler: Rc::new(handler),
            selected: false,
        }
    }

    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        glyph: impl Into<SharedString>,
        focus_handle: FocusHandle,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: PanelActionContent::Glyph(glyph.into()),
            focus_handle: focus_handle.tab_stop(true),
            handler: Rc::new(handler),
            selected: false,
        }
    }

    pub fn icon(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: Icon,
        focus_handle: FocusHandle,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: PanelActionContent::Icon(icon),
            focus_handle: focus_handle.tab_stop(true),
            handler: Rc::new(handler),
            selected: false,
        }
    }

    pub fn element(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        element: impl IntoElement,
        focus_handle: FocusHandle,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: PanelActionContent::Element(element.into_any_element()),
            focus_handle: focus_handle.tab_stop(true),
            handler: Rc::new(handler),
            selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }
}

#[derive(IntoElement)]
#[must_use]
pub struct PanelChrome {
    title: SharedString,
    icon: Option<AnyElement>,
    focus_handle: FocusHandle,
    move_action: Option<PanelAction>,
    mode_action: Option<PanelAction>,
    close_action: Option<PanelAction>,
    trailing_actions: Vec<PanelAction>,
    theme: Theme,
    metrics: Metrics,
}

impl PanelChrome {
    #[allow(
        clippy::similar_names,
        reason = "Move and mode are distinct named panel actions."
    )]
    pub fn new(
        title: impl Into<SharedString>,
        icon: Option<AnyElement>,
        focus_handle: FocusHandle,
        move_action: PanelAction,
        mode_action: PanelAction,
        close_action: PanelAction,
        style: PanelStyle,
    ) -> Self {
        Self {
            title: title.into(),
            icon,
            focus_handle,
            move_action: Some(move_action),
            mode_action: Some(mode_action),
            close_action: Some(close_action),
            trailing_actions: Vec::new(),
            theme: style.theme,
            metrics: style.metrics,
        }
    }

    pub fn with_trailing_action(mut self, action: PanelAction) -> Self {
        self.trailing_actions.push(action);
        self
    }

    pub fn without_move_action(mut self) -> Self {
        self.move_action = None;
        self
    }

    pub fn without_mode_action(mut self) -> Self {
        self.mode_action = None;
        self
    }

    pub fn without_close_action(mut self) -> Self {
        self.close_action = None;
        self
    }
}

impl RenderOnce for PanelChrome {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(self.metrics.spacing2())
            .min_w(px(0.0))
            .flex_grow();
        if let Some(icon) = self.icon {
            title = title.child(div().flex_none().child(icon));
        }
        title = title.child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(self.metrics.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(self.theme.fg)
                .child(self.title),
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .h(self.metrics.scaled(33.0))
            .pl(self.metrics.spacing4())
            .pr(self.metrics.spacing2())
            .border_b_1()
            .border_color(self.theme.border)
            .track_focus(&self.focus_handle)
            .child(title)
            .children(
                self.trailing_actions
                    .into_iter()
                    .map(|action| panel_action(action, &self.theme, self.metrics, window)),
            )
            .children(
                self.move_action
                    .map(|action| panel_action(action, &self.theme, self.metrics, window)),
            )
            .children(
                self.mode_action
                    .map(|action| panel_action(action, &self.theme, self.metrics, window)),
            )
            .children(
                self.close_action
                    .map(|action| panel_action(action, &self.theme, self.metrics, window)),
            )
    }
}

fn panel_action(
    action: PanelAction,
    theme: &Theme,
    metrics: Metrics,
    window: &Window,
) -> AnyElement {
    let PanelAction {
        id,
        label,
        content,
        focus_handle,
        handler,
        selected,
    } = action;
    let selector = SharedString::from(id.to_string());
    let group = SharedString::from(format!("panel-action-{id}"));
    let color = if selected {
        theme.accent
    } else {
        theme.fg_muted
    };
    let glyph = match content {
        PanelActionContent::Icon(icon) => IconGlyph::new(icon, metrics.font_emphasis(), color)
            .when(!selected, |glyph| {
                glyph.hover_in_group(group.clone(), theme.fg)
            })
            .into_any_element(),
        PanelActionContent::Symbol(symbol) => {
            SymbolGlyph::new(symbol, metrics.font_emphasis(), color)
                .when(!selected, |glyph| {
                    glyph.hover_in_group(group.clone(), theme.fg)
                })
                .into_any_element()
        }
        PanelActionContent::Element(element) => element,
        PanelActionContent::Glyph(glyph) => div().child(glyph).into_any_element(),
    };
    let click_handler = handler.clone();
    let key_handler = handler;
    let focus_for_mouse = focus_handle.clone();
    let focused = focus_handle.is_focused(window);
    let tooltip_background = theme.raised();
    let tooltip_foreground = theme.fg;
    let tooltip_border = theme.border;
    div()
        .id(id)
        .group(group)
        .debug_selector(move || selector.to_string())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.control_medium())
        .rounded(metrics.radius_xl())
        .cursor_pointer()
        .track_focus(&focus_handle)
        .text_size(metrics.font_emphasis())
        .text_color(color)
        .when(!selected, |button| {
            button.hover(|style| style.text_color(theme.fg))
        })
        .when(focused, |style| style.bg(theme.accent_soft))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.focus(&focus_for_mouse);
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| click_handler(window, cx))
        .key_context("Button")
        .on_action(move |_: &crate::components::ActivateButton, window, cx| {
            key_handler(window, cx);
            cx.stop_propagation();
        })
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    label.clone(),
                    tooltip_background,
                    tooltip_foreground,
                    tooltip_border,
                )
            })
            .into()
        })
        .child(glyph)
        .into_any_element()
}

type PanelResizeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PanelFrame {
    placement: PanelPlacement,
    sizing: PanelSizing,
    chrome: AnyElement,
    content: AnyElement,
    on_resize: PanelResizeHandler,
    theme: Theme,
    metrics: Metrics,
}

impl PanelFrame {
    pub fn new(
        placement: PanelPlacement,
        sizing: PanelSizing,
        chrome: impl IntoElement,
        content: impl IntoElement,
        on_resize: impl Fn(f32, &mut Window, &mut App) + 'static,
        style: PanelStyle,
    ) -> Self {
        Self {
            placement,
            sizing,
            chrome: chrome.into_any_element(),
            content: content.into_any_element(),
            on_resize: Rc::new(on_resize),
            theme: style.theme,
            metrics: style.metrics,
        }
    }

    pub fn layout(&self) -> PanelLayout {
        self.sizing.layout()
    }
}

fn panel_resize_listener(
    resize_state: PanelResizeState,
    handler: PanelResizeHandler,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let move_state = resize_state.clone();
            let move_handler = handler.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || !move_state.is_active() {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    move_state.end();
                    window.refresh();
                    return;
                }
                let Some(dimension) = move_state.dimension_at(Point::new(
                    f32::from(event.position.x),
                    f32::from(event.position.y),
                )) else {
                    return;
                };
                move_handler(dimension, window, cx);
                cx.stop_propagation();
            });
            let end_state = resize_state.clone();
            let end_handler = handler.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let Some(dimension) = end_state.dimension_at(Point::new(
                    f32::from(event.position.x),
                    f32::from(event.position.y),
                )) else {
                    return;
                };
                end_state.end();
                end_handler(dimension, window, cx);
                window.refresh();
                cx.stop_propagation();
            });
        },
    )
    .absolute()
    .size_full()
}

impl RenderOnce for PanelFrame {
    #[allow(
        clippy::too_many_lines,
        reason = "Panel chrome and its overlaid resize grip share one layout"
    )]
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let position = self.placement.position;
        let dimension = self.sizing.layout().dimension();
        let bounds = self.sizing.bounds;
        let resize_state = self.sizing.resize_state.clone();
        let down_resize_state = resize_state.clone();
        let group = SharedString::from(format!("panel-resize-{}", self.placement.id.as_str()));
        let selector = group.clone();
        let separator = div()
            .absolute()
            .bg(if resize_state.is_active() {
                self.theme.accent
            } else {
                self.theme.border
            })
            .group_hover(group.clone(), |style| style.bg(self.theme.accent));
        let resize_handle = div()
            .id(group.clone())
            .group(group)
            .debug_selector(move || selector.to_string())
            .absolute()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    down_resize_state.begin(PanelResize::new(
                        position,
                        dimension,
                        Point::new(f32::from(event.position.x), f32::from(event.position.y)),
                        bounds,
                    ));
                    window.prevent_default();
                    window.refresh();
                    cx.stop_propagation();
                },
            );
        let resize_handle = match position {
            PanelPosition::Right => resize_handle
                .left(px(-1.0))
                .top_0()
                .w(self.metrics.resize_handle_hit_area())
                .h_full()
                .child(separator.left_0().top_0().bottom_0().w(px(1.0)))
                .cursor_ew_resize(),
            PanelPosition::Bottom => resize_handle
                .left_0()
                .top(px(-1.0))
                .w_full()
                .h(self.metrics.resize_handle_hit_area())
                .child(separator.left_0().top_0().right_0().h(px(1.0)))
                .cursor_ns_resize(),
        };
        let resize_listener = panel_resize_listener(resize_state, self.on_resize);

        let frame = div()
            .id(SharedString::from(format!(
                "panel-frame-{}",
                self.placement.id.as_str()
            )))
            .max_w_full()
            .max_h_full()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .occlude()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(self.theme.bg)
            .border_color(self.theme.border)
            .child(self.chrome)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .child(self.content),
            )
            .child(resize_handle)
            .child(resize_listener);

        match (self.placement.position, self.placement.mode) {
            (PanelPosition::Right, PanelMode::Pinned) => {
                frame.w(px(dimension + 1.0)).h_full().border_l_1()
            }
            (PanelPosition::Bottom, PanelMode::Pinned) => {
                frame.w_full().h(px(dimension + 1.0)).border_t_1()
            }
            (PanelPosition::Right, PanelMode::Floating) => frame
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(dimension + 1.0))
                .border_l_1(),
            (PanelPosition::Bottom, PanelMode::Floating) => frame
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(dimension + 1.0))
                .border_t_1(),
        }
    }
}

impl std::fmt::Debug for PanelAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelAction")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for PanelChrome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelChrome")
            .field("title", &self.title)
            .field("trailing_actions", &self.trailing_actions)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for PanelFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanelFrame")
            .field("placement", &self.placement)
            .field("sizing", &self.sizing)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod interaction_tests;

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "These geometry cases use exactly representable values."
)]
mod tests {
    use super::{
        PanelHost, PanelId, PanelLayout, PanelMode, PanelPlacement, PanelPosition, PanelResize,
        PanelResizeState, PanelSizeBounds, PanelSlot,
    };
    use gpui::Point;

    #[test]
    fn every_placement_displaces_the_previous_panel() -> Result<(), Box<dyn std::error::Error>> {
        let slots = [PanelPosition::Right, PanelPosition::Bottom]
            .into_iter()
            .flat_map(|position| {
                [PanelMode::Pinned, PanelMode::Floating].map(|mode| PanelSlot { position, mode })
            })
            .collect::<Vec<_>>();
        for first_slot in &slots {
            for second_slot in &slots {
                let mut host = PanelHost::default();
                let first = PanelPlacement::new("first", first_slot.position, first_slot.mode);
                let second = PanelPlacement::new("second", second_slot.position, second_slot.mode);
                assert!(host.place(first.clone()).is_none());
                let displacement = host.place(second.clone()).ok_or("missing displacement")?;
                assert_eq!(displacement.displaced, first);
                assert_eq!(displacement.replacement, second);
                assert_eq!(host.placements().collect::<Vec<_>>(), vec![&second]);
                assert!(host.placement(&first.id).is_none());
                assert!(host.remove(&first.id).is_none());
                for slot in &slots {
                    assert_eq!(
                        host.occupant(*slot),
                        (slot == second_slot).then_some(&second.id)
                    );
                }
                assert_eq!(host.remove(&second.id), Some(second));
                assert!(host.is_empty());
                assert!(slots.iter().all(|slot| host.occupant(*slot).is_none()));
            }
        }
        Ok(())
    }

    #[test]
    fn moving_and_changing_mode_preserves_the_panel_id() {
        let mut host = PanelHost::default();
        let id = PanelId::from("stable");
        host.place(PanelPlacement::new(
            id.clone(),
            PanelPosition::Right,
            PanelMode::Floating,
        ));
        host.place(PanelPlacement::new(
            id.clone(),
            PanelPosition::Bottom,
            PanelMode::Pinned,
        ));
        assert_eq!(host.len(), 1);
        assert_eq!(
            host.placement(&id),
            Some(&PanelPlacement::new(
                id,
                PanelPosition::Bottom,
                PanelMode::Pinned,
            ))
        );
    }

    #[test]
    fn pinned_layout_consumes_only_its_axis_and_floating_layout_overlays() {
        let bounds = PanelSizeBounds::new(100.0, 500.0);
        let right = PanelLayout::new(PanelPosition::Right, PanelMode::Pinned, 320.0, bounds);
        assert_eq!(right.consumed_width(), 320.0);
        assert_eq!(right.consumed_height(), 0.0);
        assert!(!right.overlays_workspace());

        let bottom = PanelLayout::new(PanelPosition::Bottom, PanelMode::Pinned, 220.0, bounds);
        assert_eq!(bottom.consumed_width(), 0.0);
        assert_eq!(bottom.consumed_height(), 220.0);

        let floating = PanelLayout::new(PanelPosition::Right, PanelMode::Floating, 320.0, bounds);
        assert_eq!(floating.consumed_width(), 0.0);
        assert_eq!(floating.consumed_height(), 0.0);
        assert!(floating.overlays_workspace());
    }

    #[test]
    fn right_and_bottom_resize_clamp_to_caller_bounds() {
        let bounds = PanelSizeBounds::new(100.0, 500.0);
        let right = PanelResize::new(
            PanelPosition::Right,
            300.0,
            Point::new(600.0, 400.0),
            bounds,
        );
        assert_eq!(right.dimension_at(Point::new(500.0, 400.0)), 400.0);
        assert_eq!(right.dimension_at(Point::new(900.0, 400.0)), 100.0);

        let bottom = PanelResize::new(
            PanelPosition::Bottom,
            200.0,
            Point::new(600.0, 400.0),
            bounds,
        );
        assert_eq!(bottom.dimension_at(Point::new(600.0, 250.0)), 350.0);
        assert_eq!(bottom.dimension_at(Point::new(600.0, -500.0)), 500.0);
    }

    #[test]
    fn persistent_resize_state_tracks_moves_until_end() {
        let state = PanelResizeState::default();
        state.begin(PanelResize::new(
            PanelPosition::Right,
            300.0,
            Point::new(600.0, 400.0),
            PanelSizeBounds::new(100.0, 500.0),
        ));
        assert!(state.is_active());
        assert_eq!(state.dimension_at(Point::new(550.0, 400.0)), Some(350.0));
        assert_eq!(state.dimension_at(Point::new(0.0, 400.0)), Some(500.0));
        assert_eq!(state.dimension_at(Point::new(900.0, 400.0)), Some(100.0));
        assert!(state.end());
        assert!(!state.is_active());
        assert_eq!(state.dimension_at(Point::new(550.0, 400.0)), None);
        assert!(!state.end());
    }

    #[test]
    fn invalid_bounds_and_dimensions_fail_to_finite_values() {
        let bounds = PanelSizeBounds::new(f32::NAN, f32::INFINITY);
        assert_eq!(bounds.minimum, 0.0);
        assert_eq!(bounds.maximum, 0.0);
        assert_eq!(bounds.clamp(f32::NAN), 0.0);
    }
}
