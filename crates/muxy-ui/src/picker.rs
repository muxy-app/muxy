use muxy_core::shortcuts::ShortcutId;

use crate::components::IconGlyph;
#[cfg(target_os = "macos")]
use crate::components::SymbolGlyph;
use crate::controls::{self, Style};
use crate::icon::Icon;
use crate::popover;
use crate::scrollbar::{MINIMUM_THUMB_LENGTH, ThumbGeometry};
use crate::text_input::{self, InputEvent, InputStyle, TextInput};
use crate::theme::{Metrics, Theme};
use crate::tr;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    Hsla, InteractiveElement, IntoElement, ListAlignment, ListOffset, ListState, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, px, relative,
};
use std::collections::HashMap;

const KEY_CONTEXT: &str = "Picker";

actions!(
    picker,
    [
        SelectNext,
        SelectPrevious,
        Confirm,
        SecondaryConfirm,
        Dismiss,
        NextTab,
        PreviousTab,
        TabPressed,
        NavigateBack,
    ]
);

pub fn register_shortcuts(registry: &mut crate::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::PopoverSelectNext, &SelectNext);
    registry.register(ShortcutId::PopoverSelectPrevious, &SelectPrevious);
    registry.register(ShortcutId::PopoverConfirm, &Confirm);
    registry.register(ShortcutId::PopoverSecondaryConfirm, &SecondaryConfirm);
    registry.register(ShortcutId::PopoverTabPressed, &TabPressed);
    registry.register(ShortcutId::PopoverNavigateBack, &NavigateBack);
    registry.register(ShortcutId::PopoverDismiss, &Dismiss);
    registry.register(ShortcutId::PopoverNextTab, &NextTab);
    registry.register(ShortcutId::PopoverPreviousTab, &PreviousTab);
}

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let mut registry = crate::shortcuts::Registry::new(&muxy_core::shortcuts::Defaults);
    register_shortcuts(&mut registry);
    registry.into_bindings()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerPresentation {
    Modal,
    Popover,
    Embedded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PickerLayout {
    inline_tabs: bool,
    header_height: f32,
    row_height: f32,
    section_height: f32,
    tab_height: f32,
    panel_radius: f32,
    row_radius: f32,
    horizontal_inset: f32,
    outer_item_inset: f32,
    item_inset: f32,
    item_gap: f32,
    list_vertical_inset: f32,
    status_height: f32,
    shell_padding: f32,
    shell_gap: f32,
    row_gap: f32,
    footer_height: f32,
}

impl PickerLayout {
    fn resolve(presentation: PickerPresentation, compact: bool) -> Self {
        let mut layout = Self {
            inline_tabs: presentation == PickerPresentation::Popover,
            header_height: 32.0,
            row_height: 32.0,
            section_height: 22.0,
            tab_height: 24.0,
            panel_radius: 6.0,
            row_radius: 4.0,
            horizontal_inset: 8.0,
            outer_item_inset: 4.0,
            item_inset: 4.0,
            item_gap: 6.0,
            list_vertical_inset: 4.0,
            status_height: 48.0,
            shell_padding: if presentation == PickerPresentation::Popover {
                popover::PADDING
            } else {
                0.0
            },
            shell_gap: if presentation == PickerPresentation::Popover {
                popover::ROW_GAP
            } else {
                0.0
            },
            row_gap: 0.0,
            footer_height: 37.0,
        };
        if presentation == PickerPresentation::Popover {
            layout.item_inset = popover::ROW_PADDING;
            layout.horizontal_inset = popover::ROW_PADDING;
            layout.outer_item_inset = 0.0;
            layout.list_vertical_inset = 0.0;
            layout.row_gap = popover::ROW_GAP;
            layout.footer_height = popover::ROW_HEIGHT + 5.0;
        }
        if compact {
            layout.header_height = 28.0;
            layout.row_height = 24.0;
            layout.section_height = 20.0;
            layout.status_height = 40.0;
        }
        layout
    }

    fn row_height(self, row: &PickerRow) -> f32 {
        if self.inline_tabs && row.detail.is_none() && row.swatches.is_empty() {
            popover::ROW_HEIGHT
        } else {
            self.row_height
        }
    }

    fn item_extent(self, item: &PickerItem, index: usize, count: usize) -> f32 {
        self.item_height(item) + if index + 1 < count { self.row_gap } else { 0.0 }
    }

    fn item_height(self, item: &PickerItem) -> f32 {
        match item {
            PickerItem::Section(_) => self.section_height,
            PickerItem::Row(row) => self.row_height(row),
        }
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "GPUI measures row heights with f32 coordinates."
)]
fn content_height(
    layout: PickerLayout,
    tab_count: usize,
    items: &[PickerItem],
    status: &PickerStatus,
    detail_line_count: Option<usize>,
    has_footer: bool,
) -> f32 {
    let tabs = if tab_count > 1 && !layout.inline_tabs {
        layout.tab_height + 12.0
    } else {
        0.0
    };
    let body = if let Some(line_count) = detail_line_count {
        line_count.max(1) as f32 * 20.0
    } else if matches!(status, PickerStatus::Ready) && !items.is_empty() {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| layout.item_extent(item, index, items.len()))
            .sum::<f32>()
            + layout.list_vertical_inset * 2.0
    } else {
        layout.status_height
    };
    let footer = if has_footer {
        layout.footer_height
    } else {
        0.0
    };
    let gaps = 1 + usize::from(tabs > 0.0) + usize::from(has_footer);
    let border = 2.0;
    tabs + layout.header_height
        + body
        + footer
        + border
        + layout.shell_padding * 2.0
        + layout.shell_gap * gaps as f32
}

fn scrollbar_item_heights(
    layout: PickerLayout,
    scale: f32,
    items: &[PickerItem],
    detail_line_count: Option<usize>,
) -> Vec<f32> {
    if let Some(line_count) = detail_line_count {
        return vec![20.0 * scale; line_count];
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| layout.item_extent(item, index, items.len()) * scale)
        .collect()
}

fn scrollbar_offset(heights: &[f32], offset: ListOffset) -> f32 {
    heights.iter().take(offset.item_ix).sum::<f32>() + f32::from(offset.offset_in_item)
}

fn scrollbar_list_offset(heights: &[f32], target: f32) -> ListOffset {
    let mut remaining = target.max(0.0);
    for (item_ix, height) in heights.iter().copied().enumerate() {
        if remaining < height {
            return ListOffset {
                item_ix,
                offset_in_item: px(remaining),
            };
        }
        remaining -= height;
    }
    ListOffset {
        item_ix: heights.len(),
        offset_in_item: px(0.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickerGeometry {
    pub width: f32,
    pub height: f32,
    pub top: f32,
    pub backdrop: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickerMetrics {
    pub modal_width: f32,
    pub modal_height: f32,
    pub modal_top: f32,
    pub popover_width: f32,
    pub popover_height: f32,
    pub viewport_margin: f32,
}

impl Default for PickerMetrics {
    fn default() -> Self {
        Self {
            modal_width: 480.0,
            modal_height: 360.0,
            modal_top: 48.0,
            popover_width: 340.0,
            popover_height: 360.0,
            viewport_margin: 8.0,
        }
    }
}

impl PickerMetrics {
    fn scaled(self, metrics: Metrics) -> Self {
        Self {
            modal_width: f32::from(metrics.scaled(self.modal_width)),
            modal_height: f32::from(metrics.scaled(self.modal_height)),
            popover_width: f32::from(metrics.scaled(self.popover_width)),
            popover_height: f32::from(metrics.scaled(self.popover_height)),
            ..self
        }
    }

    pub fn resolve(
        self,
        presentation: PickerPresentation,
        viewport_width: f32,
        viewport_height: f32,
    ) -> PickerGeometry {
        match presentation {
            PickerPresentation::Modal => {
                let width = self
                    .modal_width
                    .min((viewport_width - self.viewport_margin * 2.0).max(0.0));
                let top = if viewport_width < self.modal_width {
                    self.viewport_margin * 2.0
                } else {
                    self.modal_top
                }
                .min((viewport_height - self.viewport_margin).max(0.0));
                PickerGeometry {
                    width,
                    height: self
                        .modal_height
                        .min((viewport_height - top - self.viewport_margin * 2.0).max(0.0)),
                    top,
                    backdrop: true,
                }
            }
            PickerPresentation::Popover => PickerGeometry {
                width: self
                    .popover_width
                    .min((viewport_width - self.viewport_margin * 2.0).max(0.0)),
                height: self
                    .popover_height
                    .min((viewport_height - self.viewport_margin * 2.0).max(0.0)),
                top: 0.0,
                backdrop: false,
            },
            PickerPresentation::Embedded => PickerGeometry {
                width: viewport_width,
                height: viewport_height,
                top: 0.0,
                backdrop: false,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickerLeading {
    Icon(Icon),
    Symbol(SharedString),
    Text(SharedString),
    Asset(SharedString),
    Swatch(Hsla),
}

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct PickerAction {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Option<PickerLeading>,
    pub destructive: bool,
    pub disabled: bool,
}

impl PickerAction {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            destructive: false,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: PickerLeading) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerSelectionStyle {
    Checkmark,
    Highlight,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickerRow {
    pub id: SharedString,
    pub title: SharedString,
    pub detail: Option<SharedString>,
    pub leading: Option<PickerLeading>,
    pub trailing: Option<SharedString>,
    /// A short label outlined at the row's right edge.
    pub badge: Option<SharedString>,
    pub actions: Vec<PickerAction>,
    pub swatches: Vec<Hsla>,
    pub current: bool,
    pub selected: bool,
    pub selection_style: PickerSelectionStyle,
    pub disabled: bool,
}

impl PickerRow {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            leading: None,
            trailing: None,
            badge: None,
            actions: Vec::new(),
            swatches: Vec::new(),
            current: false,
            selected: false,
            selection_style: PickerSelectionStyle::Checkmark,
            disabled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub enum PickerItem {
    Section(SharedString),
    Row(PickerRow),
}

impl PickerItem {
    pub fn section(label: impl Into<SharedString>) -> Self {
        Self::Section(label.into())
    }

    pub fn row(id: impl Into<SharedString>) -> Self {
        let id = id.into();
        Self::Row(PickerRow::new(id.clone(), id))
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        if let Self::Row(row) = &mut self {
            row.disabled = disabled;
        }
        self
    }

    pub fn row_data(self) -> Option<PickerRow> {
        match self {
            Self::Row(row) => Some(row),
            Self::Section(_) => None,
        }
    }

    fn selectable_id(&self) -> Option<&SharedString> {
        match self {
            Self::Row(row) if !row.disabled => Some(&row.id),
            Self::Section(_) | Self::Row(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PickerSelection {
    pub id: SharedString,
}

impl PickerSelection {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerEscape {
    CloseInlineAction,
    Dismiss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownPickerTab;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownPickerRow;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PickerStatus {
    #[default]
    Ready,
    Loading(SharedString),
    Empty(SharedString),
    Error(SharedString),
}

#[derive(Clone, Debug, Default)]
struct PickerTabState {
    query: String,
    items: Vec<PickerItem>,
    selected_id: Option<SharedString>,
    inline_action: Option<(SharedString, SharedString)>,
    status: PickerStatus,
}

#[derive(Clone, Debug)]
pub struct PickerState {
    tabs: Vec<SharedString>,
    active_tab: SharedString,
    active: PickerTabState,
    tab_states: HashMap<SharedString, PickerTabState>,
}

impl PickerState {
    pub fn new(tabs: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        let tabs = tabs.into_iter().map(Into::into).collect::<Vec<_>>();
        assert!(!tabs.is_empty());
        let active_tab = tabs[0].clone();
        let tab_states = tabs
            .iter()
            .filter(|tab| **tab != active_tab)
            .cloned()
            .map(|tab| (tab, PickerTabState::default()))
            .collect();
        Self {
            tabs,
            active_tab,
            active: PickerTabState::default(),
            tab_states,
        }
    }

    pub fn active_tab(&self) -> &str {
        self.active_tab.as_ref()
    }

    pub fn tabs(&self) -> &[SharedString] {
        &self.tabs
    }

    pub fn query(&self) -> &str {
        &self.active_state().query
    }

    pub fn item_count(&self) -> usize {
        self.active_state().items.len()
    }

    pub fn items(&self) -> &[PickerItem] {
        &self.active_state().items
    }

    pub fn status(&self) -> &PickerStatus {
        &self.active_state().status
    }

    pub fn set_status(&mut self, status: PickerStatus) {
        let state = self.active_state_mut();
        state.status = status;
        if state.status != PickerStatus::Ready {
            state.inline_action = None;
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.active_state_mut().query = query.into();
    }

    pub fn set_items(&mut self, items: Vec<PickerItem>) {
        let previous = self.active_state().selected_id.clone();
        let selected_id = previous
            .filter(|selected| {
                items
                    .iter()
                    .any(|item| item.selectable_id() == Some(selected))
            })
            .or_else(|| items.iter().find_map(PickerItem::selectable_id).cloned());
        let state = self.active_state_mut();
        state.items = items;
        state.selected_id = selected_id;
        state.inline_action = None;
        state.status = PickerStatus::Ready;
    }

    pub fn activate_tab(&mut self, tab: &str) -> Result<(), UnknownPickerTab> {
        if self.active_tab.as_ref() == tab {
            return Ok(());
        }
        let Some(tab) = self.tabs.iter().find(|candidate| candidate.as_ref() == tab) else {
            return Err(UnknownPickerTab);
        };
        let next = self.tab_states.remove(tab).ok_or(UnknownPickerTab)?;
        self.tab_states.insert(
            self.active_tab.clone(),
            std::mem::replace(&mut self.active, next),
        );
        self.active_tab = tab.clone();
        Ok(())
    }

    pub fn selected_row_id(&self) -> Option<&str> {
        if *self.status() != PickerStatus::Ready {
            return None;
        }
        self.active_state().selected_id.as_ref().map(AsRef::as_ref)
    }

    pub fn select_next(&mut self) {
        self.move_selection(1);
    }

    pub fn select_previous(&mut self) {
        self.move_selection(-1);
    }

    pub fn select_last(&mut self) {
        if *self.status() != PickerStatus::Ready {
            return;
        }
        let selected = self
            .active_state()
            .items
            .iter()
            .rev()
            .find_map(PickerItem::selectable_id)
            .cloned();
        self.active_state_mut().selected_id = selected;
    }

    pub fn select_row(&mut self, id: &str) -> Result<(), UnknownPickerRow> {
        if *self.status() != PickerStatus::Ready
            || !self.active_state().items.iter().any(|item| {
                item.selectable_id()
                    .is_some_and(|candidate| candidate.as_ref() == id)
            })
        {
            return Err(UnknownPickerRow);
        }
        self.active_state_mut().selected_id = Some(SharedString::from(id.to_owned()));
        Ok(())
    }

    pub fn open_inline_action(&mut self, id: &str, action: &str) -> Result<(), UnknownPickerRow> {
        self.select_row(id)?;
        self.active_state_mut().inline_action =
            Some((id.to_owned().into(), action.to_owned().into()));
        Ok(())
    }

    pub fn escape(&mut self) -> PickerEscape {
        if self.active_state_mut().inline_action.take().is_some() {
            PickerEscape::CloseInlineAction
        } else {
            PickerEscape::Dismiss
        }
    }

    pub fn confirm(&self) -> Option<PickerSelection> {
        self.selected_row_id()
            .map(|id| PickerSelection::new(id.to_owned()))
    }

    fn inline_action(&self) -> Option<(&str, &str)> {
        self.active_state()
            .inline_action
            .as_ref()
            .map(|(row, action)| (row.as_ref(), action.as_ref()))
    }

    fn active_state(&self) -> &PickerTabState {
        &self.active
    }

    fn active_state_mut(&mut self) -> &mut PickerTabState {
        &mut self.active
    }

    fn move_selection(&mut self, delta: isize) {
        if *self.status() != PickerStatus::Ready {
            return;
        }
        let selectable = self
            .active_state()
            .items
            .iter()
            .filter_map(PickerItem::selectable_id)
            .cloned()
            .collect::<Vec<_>>();
        if selectable.is_empty() {
            self.active_state_mut().selected_id = None;
            return;
        }
        let current = self
            .selected_row_id()
            .and_then(|selected| selectable.iter().position(|id| id.as_ref() == selected))
            .unwrap_or(if delta > 0 { selectable.len() - 1 } else { 0 });
        let next = if delta > 0 {
            (current + 1) % selectable.len()
        } else {
            (current + selectable.len() - 1) % selectable.len()
        };
        self.active_state_mut().selected_id = Some(selectable[next].clone());
    }
}

#[derive(Clone, Debug)]
pub struct PickerTab {
    pub id: SharedString,
    pub label: SharedString,
}

impl PickerTab {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PickerConfig {
    pub id: SharedString,
    pub presentation: PickerPresentation,
    pub tabs: Vec<PickerTab>,
    pub placeholder: SharedString,
    pub footer_actions: Vec<PickerAction>,
    pub width: Option<f32>,
    pub compact: bool,
    pub completion_on_tab: bool,
    pub confirm_on_click: bool,
}

impl PickerConfig {
    pub fn new(id: impl Into<SharedString>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            presentation: PickerPresentation::Modal,
            tabs: vec![PickerTab::new("items", tr!("Items"))],
            placeholder: placeholder.into(),
            footer_actions: Vec::new(),
            width: None,
            compact: false,
            completion_on_tab: false,
            confirm_on_click: true,
        }
    }

    pub fn popover(id: impl Into<SharedString>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            presentation: PickerPresentation::Popover,
            ..Self::new(id, placeholder)
        }
    }

    pub fn dropdown(id: impl Into<SharedString>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            compact: true,
            width: Some(260.0),
            ..Self::popover(id, placeholder)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickerEvent {
    QueryChanged {
        tab: SharedString,
        query: SharedString,
    },
    TabChanged(SharedString),
    SelectionChanged(PickerSelection),
    RowClicked {
        row: SharedString,
        shift: bool,
        platform: bool,
    },
    Confirmed(PickerSelection),
    SecondaryConfirmed(PickerSelection),
    Submitted {
        secondary: bool,
    },
    CompletionRequested,
    NavigateBackRequested,
    RowAction {
        row: SharedString,
        action: SharedString,
    },
    FooterAction(SharedString),
    InlineActionDismissed,
    Dismissed,
}

pub struct Picker {
    config: PickerConfig,
    state: PickerState,
    input: Entity<TextInput>,
    focus_handle: FocusHandle,
    scroll: ListState,
    theme: Theme,
    metrics: Metrics,
    focused: bool,
    can_navigate_back: bool,
    detail: Option<(SharedString, Vec<SharedString>)>,
    confirmation_message: Option<SharedString>,
    scrollbar_drag: Option<Pixels>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PickerEvent> for Picker {}

impl Focusable for Picker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Picker {
    pub fn new(
        config: PickerConfig,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        assert!(!config.tabs.is_empty());
        let style = InputStyle::compact(&theme, &metrics);
        let input = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder(config.placeholder.clone())
        });
        let subscription = cx.subscribe(
            &input,
            |popover: &mut Self, input: Entity<TextInput>, event, cx| {
                if !matches!(event, InputEvent::Changed) {
                    return;
                }
                let query = input.read(cx).text().to_owned();
                popover.state.set_query(query.clone());
                cx.emit(PickerEvent::QueryChanged {
                    tab: popover.state.active_tab.clone(),
                    query: query.into(),
                });
                cx.notify();
            },
        );
        let state = PickerState::new(config.tabs.iter().map(|tab| tab.id.clone()));
        let row_height = PickerLayout::resolve(config.presentation, config.compact).row_height;
        Self {
            config,
            state,
            input,
            focus_handle: cx.focus_handle(),
            scroll: ListState::new(0, ListAlignment::Top, metrics.scaled(row_height)),
            theme,
            metrics,
            focused: false,
            can_navigate_back: false,
            detail: None,
            confirmation_message: None,
            scrollbar_drag: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn input(&self) -> Entity<TextInput> {
        self.input.clone()
    }

    pub fn active_tab(&self) -> &str {
        self.state.active_tab()
    }

    pub fn query(&self) -> &str {
        self.state.query()
    }

    pub fn set_placeholder(&self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, _| input.set_placeholder(placeholder.into()));
    }

    pub fn set_can_navigate_back(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.can_navigate_back = enabled;
        cx.notify();
    }

    pub fn set_query(&mut self, query: impl Into<String>, cx: &mut Context<Self>) {
        let query = query.into();
        self.state.set_query(query.clone());
        self.input.update(cx, |input, cx| input.set_text(query, cx));
    }

    pub fn set_items(&mut self, items: Vec<PickerItem>, cx: &mut Context<Self>) {
        if self.state.items() == items {
            return;
        }
        self.state.set_items(items);
        self.confirmation_message = None;
        if self.detail.is_none() {
            self.scroll.reset(self.state.item_count());
            self.scroll_to_selection();
        }
        cx.notify();
    }

    pub fn set_status(&mut self, status: PickerStatus, cx: &mut Context<Self>) {
        if self.state.status() == &status {
            return;
        }
        self.state.set_status(status);
        cx.notify();
    }

    pub fn set_current_row(&mut self, id: &str, cx: &mut Context<Self>) {
        for item in &mut self.state.active.items {
            if let PickerItem::Row(row) = item {
                row.current = row.id == id;
            }
        }
        cx.notify();
    }

    pub fn set_footer_actions(&mut self, actions: Vec<PickerAction>, cx: &mut Context<Self>) {
        if self.config.footer_actions == actions {
            return;
        }
        self.config.footer_actions = actions;
        cx.notify();
    }

    pub fn select_row(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), UnknownPickerRow> {
        self.state.select_row(id)?;
        self.scroll_to_selection();
        cx.notify();
        Ok(())
    }

    pub fn set_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        let style = InputStyle::compact(&theme, &metrics);
        self.theme = theme;
        self.metrics = metrics;
        self.input
            .update(cx, |input, cx| input.set_style(style, cx));
        cx.notify();
    }

    pub fn show_detail(
        &mut self,
        title: impl Into<SharedString>,
        body: &str,
        cx: &mut Context<Self>,
    ) {
        let lines = body
            .lines()
            .map(|line| SharedString::from(line.to_owned()))
            .collect::<Vec<_>>();
        self.detail = Some((title.into(), lines));
        self.focused = false;
        self.scroll
            .reset(self.detail.as_ref().map_or(0, |(_, lines)| lines.len()));
        cx.notify();
    }

    pub fn open_confirmation(
        &mut self,
        row: &str,
        action: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), UnknownPickerRow> {
        self.open_confirmation_with_message(row, action, None, cx)
    }

    pub fn open_confirmation_with_message(
        &mut self,
        row: &str,
        action: &str,
        message: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> Result<(), UnknownPickerRow> {
        self.state.open_inline_action(row, action)?;
        self.confirmation_message = message;
        cx.notify();
        Ok(())
    }

    pub fn activate_tab(
        &mut self,
        tab: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), UnknownPickerTab> {
        self.state.activate_tab(tab)?;
        self.detail = None;
        self.focused = false;
        let query = self.state.query().to_owned();
        self.input.update(cx, |input, cx| input.set_text(query, cx));
        cx.emit(PickerEvent::TabChanged(self.state.active_tab.clone()));
        self.scroll.reset(self.state.item_count());
        self.scroll_to_selection();
        cx.notify();
        Ok(())
    }

    fn move_selection(&mut self, direction: isize, cx: &mut Context<Self>) {
        if self.detail.is_some() {
            return;
        }
        if direction > 0 {
            self.state.select_next();
        } else {
            self.state.select_previous();
        }
        self.scroll_to_selection();
        if let Some(selection) = self.state.confirm() {
            cx.emit(PickerEvent::SelectionChanged(selection));
        }
        cx.notify();
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if !self.accepts_submission() {
            return;
        }
        if let Some(selection) = self.state.confirm() {
            cx.emit(PickerEvent::Confirmed(selection));
        } else {
            cx.emit(PickerEvent::Submitted { secondary: false });
        }
    }

    fn secondary_confirm(&mut self, _: &SecondaryConfirm, _: &mut Window, cx: &mut Context<Self>) {
        if !self.accepts_submission() {
            return;
        }
        if let Some(selection) = self.state.confirm() {
            cx.emit(PickerEvent::SecondaryConfirmed(selection));
        } else {
            cx.emit(PickerEvent::Submitted { secondary: true });
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.detail.take().is_some() {
            self.scroll.reset(self.state.item_count());
            window.focus(&self.input.focus_handle(cx));
            self.focused = true;
            cx.notify();
            return;
        }
        match self.state.escape() {
            PickerEscape::CloseInlineAction => {
                self.confirmation_message = None;
                cx.emit(PickerEvent::InlineActionDismissed);
                cx.notify();
            }
            PickerEscape::Dismiss => cx.emit(PickerEvent::Dismissed),
        }
    }

    fn next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(1, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(-1, cx);
    }

    fn tab_pressed(&mut self, _: &TabPressed, _: &mut Window, cx: &mut Context<Self>) {
        if !self.accepts_submission() {
            return;
        }
        if self.config.completion_on_tab {
            cx.emit(PickerEvent::CompletionRequested);
        } else {
            self.move_selection(1, cx);
        }
    }

    #[allow(
        clippy::unused_self,
        reason = "GPUI action listeners use the entity receiver."
    )]
    fn navigate_back(&mut self, _: &NavigateBack, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PickerEvent::NavigateBackRequested);
    }

    fn accepts_submission(&self) -> bool {
        self.detail.is_none()
            && matches!(
                self.state.status(),
                PickerStatus::Ready | PickerStatus::Empty(_)
            )
    }

    fn cycle_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let current = self
            .config
            .tabs
            .iter()
            .position(|tab| tab.id.as_ref() == self.state.active_tab())
            .unwrap_or(0);
        if self.config.tabs.is_empty() {
            return;
        }
        let next = if delta > 0 {
            (current + 1) % self.config.tabs.len()
        } else {
            (current + self.config.tabs.len() - 1) % self.config.tabs.len()
        };
        let tab = self.config.tabs[next].id.clone();
        let _ = self.activate_tab(&tab, cx);
    }

    fn scroll_to_selection(&self) {
        if let Some(selected) = self.state.selected_row_id()
            && let Some(index) = self.state.items().iter().position(|item| {
                item.selectable_id()
                    .is_some_and(|candidate| candidate.as_ref() == selected)
            })
        {
            self.scroll.scroll_to_reveal_item(index);
        }
    }

    fn fitted_height(&self, layout: PickerLayout) -> f32 {
        let logical_height = content_height(
            layout,
            self.config.tabs.len(),
            self.state.items(),
            self.state.status(),
            self.detail.as_ref().map(|(_, lines)| lines.len()),
            !self.config.footer_actions.is_empty(),
        );
        let borders = if self.config.footer_actions.is_empty() {
            2.0
        } else {
            3.0
        };
        f32::from(self.metrics.scaled(logical_height - borders)) + borders
    }

    fn render_leading(leading: &PickerLeading, color: Hsla, metrics: Metrics) -> AnyElement {
        match leading {
            PickerLeading::Icon(icon) => {
                IconGlyph::new(*icon, metrics.icon_md(), color).into_any_element()
            }
            PickerLeading::Symbol(symbol) => {
                #[cfg(target_os = "macos")]
                {
                    SymbolGlyph::new(symbol.clone(), metrics.icon_md(), color).into_any_element()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    IconGlyph::new(
                        Icon::from_symbol(symbol).unwrap_or(Icon::Puzzle),
                        metrics.icon_md(),
                        color,
                    )
                    .into_any_element()
                }
            }
            PickerLeading::Text(text) => div()
                .w(metrics.icon_md())
                .flex_none()
                .text_size(metrics.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(text.clone())
                .into_any_element(),
            PickerLeading::Asset(path) => gpui::svg()
                .path(path.clone())
                .size(metrics.icon_md())
                .flex_none()
                .text_color(color)
                .into_any_element(),
            PickerLeading::Swatch(color) => div()
                .size(metrics.icon_md())
                .rounded(metrics.radius_sm())
                .bg(*color)
                .into_any_element(),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "The declarative row tree keeps its layout and event handlers together."
    )]
    fn render_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(item) = self.state.items().get(index).cloned() else {
            return div().into_any_element();
        };
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        match item {
            PickerItem::Section(label) => {
                let section = if self.config.presentation == PickerPresentation::Popover {
                    popover::header(&self.theme, self.metrics)
                } else {
                    div()
                        .flex()
                        .items_end()
                        .px(self.metrics.scaled(layout.horizontal_inset))
                        .pb(self.metrics.spacing2())
                        .border_b_1()
                        .border_color(self.theme.border)
                        .text_size(self.metrics.font_footnote())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(self.theme.fg_muted)
                };
                section
                    .w_full()
                    .h(self.metrics.scaled(layout.section_height))
                    .child(label)
                    .into_any_element()
            }
            PickerItem::Row(row) => {
                if self
                    .state
                    .inline_action()
                    .is_some_and(|(candidate, _)| candidate == row.id.as_ref())
                {
                    return self.render_confirmation(&row, cx);
                }
                let highlighted = self.state.selected_row_id() == Some(row.id.as_ref());
                let row_height = layout.row_height(&row);
                let row_inset =
                    self.metrics
                        .scaled(if self.config.presentation == PickerPresentation::Modal {
                            layout.horizontal_inset
                        } else {
                            layout.item_inset
                        });
                let action_padding = self.metrics.spacing3();
                let id = row.id.clone();
                let hover_id = row.id.clone();
                let group = SharedString::from(format!("command-row-{}", row.id));
                let swatches = row.swatches.clone();
                let selected_background =
                    if self.config.presentation == PickerPresentation::Embedded {
                        self.theme.hover
                    } else {
                        self.theme.surface
                    };
                let element_id = SharedString::from(format!("picker-row-{}", row.id));
                let content = if self.config.presentation == PickerPresentation::Popover {
                    popover::row(
                        &self.theme,
                        self.metrics,
                        element_id,
                        !row.disabled,
                        highlighted || row.selected,
                    )
                } else {
                    div()
                        .id(element_id)
                        .pl(row_inset)
                        .pr(row_inset)
                        .when(
                            self.config.presentation != PickerPresentation::Modal,
                            |element| element.rounded(self.metrics.scaled(layout.row_radius)),
                        )
                        .flex()
                        .items_center()
                        .gap(self.metrics.scaled(layout.item_gap))
                        .text_color(if row.disabled {
                            self.theme.fg_dim
                        } else {
                            self.theme.fg
                        })
                        .when(highlighted || row.selected, |element| {
                            element.bg(selected_background)
                        })
                        .when(!row.disabled, |element| {
                            element
                                .cursor_pointer()
                                .hover(|style| style.bg(self.theme.hover))
                        })
                };
                let mut content = content
                    .debug_selector({
                        let id = row.id.clone();
                        move || format!("picker-row-{id}")
                    })
                    .group(group.clone())
                    .relative()
                    .w_full()
                    .h(self.metrics.scaled(row_height))
                    .when(!row.disabled, |element| {
                        element
                            .on_hover(cx.listener(move |popover, hovered: &bool, _, cx| {
                                if *hovered {
                                    if popover.detail.is_some()
                                        || popover.state.select_row(&hover_id).is_err()
                                    {
                                        return;
                                    }
                                    if let Some(selection) = popover.state.confirm() {
                                        cx.emit(PickerEvent::SelectionChanged(selection));
                                    }
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(
                                move |popover, event: &gpui::ClickEvent, _, cx| {
                                    if popover.detail.is_some()
                                        || popover.state.select_row(&id).is_err()
                                    {
                                        return;
                                    }
                                    cx.emit(PickerEvent::RowClicked {
                                        row: id.clone(),
                                        shift: event.modifiers().shift,
                                        platform: event.modifiers().platform,
                                    });
                                    if popover.config.confirm_on_click
                                        && let Some(selection) = popover.state.confirm()
                                    {
                                        cx.emit(PickerEvent::Confirmed(selection));
                                    }
                                },
                            ))
                    });
                let show_checkmark = row.selection_style == PickerSelectionStyle::Checkmark;
                if show_checkmark && (row.current || row.selected) {
                    content = content.child(
                        IconGlyph::new(Icon::Check, self.metrics.icon_sm(), self.theme.accent)
                            .into_any_element(),
                    );
                } else if let Some(leading) = &row.leading {
                    content = content.child(Self::render_leading(
                        leading,
                        self.theme.fg_muted,
                        self.metrics,
                    ));
                } else if show_checkmark {
                    content = content.child(div().w(self.metrics.icon_sm()));
                }
                content = content.child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap(self.metrics.spacing4())
                        .child(
                            div()
                                .debug_selector({
                                    let id = row.id.clone();
                                    move || format!("picker-title-{id}")
                                })
                                .min_w(px(0.0))
                                .when(row.detail.is_some(), |element| element.max_w(relative(0.6)))
                                .truncate()
                                .text_size(self.metrics.font_body())
                                .font_weight(if row.current || row.selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(row.title),
                        )
                        .when_some(row.detail, |element, detail| {
                            element.child(
                                div()
                                    .debug_selector({
                                        let id = row.id.clone();
                                        move || format!("picker-detail-{id}")
                                    })
                                    .min_w(px(0.0))
                                    .flex_1()
                                    .truncate()
                                    .text_size(self.metrics.font_footnote())
                                    .text_color(self.theme.fg_muted)
                                    .child(detail),
                            )
                        }),
                );
                if !swatches.is_empty() {
                    let diameter =
                        self.metrics
                            .scaled(if self.config.compact { 12.0 } else { 20.0 });
                    let step = diameter / 2.0;
                    let width = swatches
                        .iter()
                        .skip(1)
                        .fold(diameter, |width, _| width + step);
                    let mut preview = div().relative().w(width).h(diameter).flex_none();
                    let mut left = px(0.0);
                    for (index, color) in swatches.into_iter().enumerate() {
                        preview = preview.child(
                            div()
                                .debug_selector({
                                    let id = row.id.clone();
                                    move || format!("picker-swatch-{id}-{index}")
                                })
                                .absolute()
                                .left(left)
                                .top_0()
                                .size(diameter)
                                .rounded_full()
                                .shadow(crate::theme::Elevation::Elevated.shadow(self.theme.bg))
                                .bg(color),
                        );
                        left += step;
                    }
                    content = content.child(preview);
                }
                if let Some(trailing) = row.trailing {
                    content = content.child(
                        div()
                            .flex_none()
                            .max_w(relative(0.4))
                            .truncate()
                            .text_size(self.metrics.font_footnote())
                            .text_color(self.theme.fg_muted)
                            .child(trailing),
                    );
                }
                if let Some(badge) = row.badge {
                    content = content.child(
                        div()
                            .debug_selector({
                                let id = row.id.clone();
                                move || format!("picker-badge-{id}")
                            })
                            .flex_none()
                            .max_w(relative(0.4))
                            .truncate()
                            .px(self.metrics.spacing2())
                            .rounded(self.metrics.radius_sm())
                            .border_1()
                            .border_color(self.theme.border)
                            .text_size(self.metrics.font_caption())
                            .text_color(self.theme.fg_muted)
                            .child(badge),
                    );
                }
                if !row.actions.is_empty() {
                    let mut actions = div()
                        .absolute()
                        .top_0()
                        .right(row_inset)
                        .h_full()
                        .pl(self.metrics.spacing2())
                        .flex()
                        .items_center()
                        .gap(self.metrics.spacing1())
                        .bg(if self.config.presentation == PickerPresentation::Popover {
                            self.theme.bg.blend(self.theme.hover)
                        } else {
                            self.theme.raised().blend(self.theme.hover)
                        })
                        .invisible()
                        .group_hover(group, Styled::visible);
                    for action in row.actions {
                        let row_id = row.id.clone();
                        let action_id = action.id.clone();
                        let disabled = action.disabled;
                        let color = if action.destructive {
                            self.theme.danger
                        } else {
                            self.theme.fg_muted
                        };
                        let icon_only = action.icon.is_some();
                        actions = actions.child(
                            div()
                                .id(SharedString::from(format!(
                                    "picker-action-{}-{}",
                                    row_id, action.id
                                )))
                                .h(if self.config.presentation == PickerPresentation::Popover {
                                    self.metrics.control_small()
                                } else {
                                    self.metrics.control_medium()
                                })
                                .when(icon_only, |element| {
                                    element.w(
                                        if self.config.presentation == PickerPresentation::Popover {
                                            self.metrics.control_small()
                                        } else {
                                            self.metrics.control_medium()
                                        },
                                    )
                                })
                                .when(!icon_only, |element| element.px(action_padding))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.metrics.radius_sm())
                                .when(disabled, |element| element.opacity(0.4))
                                .when(!disabled, |element| {
                                    element
                                        .cursor_pointer()
                                        .hover(|style| style.bg(self.theme.hover))
                                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(PickerEvent::RowAction {
                                                row: row_id.clone(),
                                                action: action_id.clone(),
                                            });
                                        }))
                                })
                                .when_some(action.icon, |element, icon| {
                                    element.child(Self::render_leading(&icon, color, self.metrics))
                                })
                                .when(!icon_only, |element| {
                                    element
                                        .text_size(self.metrics.font_footnote())
                                        .text_color(color)
                                        .child(action.label.clone())
                                }),
                        );
                    }
                    content = content.child(actions);
                }
                let _ = window;
                div()
                    .w_full()
                    .h(self.metrics.scaled(row_height))
                    .when(
                        self.config.presentation != PickerPresentation::Modal,
                        |element| element.px(self.metrics.scaled(layout.outer_item_inset)),
                    )
                    .child(content)
                    .into_any_element()
            }
        }
    }

    fn render_confirmation(&mut self, row: &PickerRow, cx: &mut Context<Self>) -> AnyElement {
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        let action_id = self
            .state
            .inline_action()
            .map(|(_, action)| SharedString::from(action.to_owned()))
            .unwrap_or_default();
        let label = row
            .actions
            .iter()
            .find(|action| action.id == action_id)
            .map_or_else(|| tr!("Confirm"), |action| action.label.clone());
        let message = self
            .confirmation_message
            .clone()
            .unwrap_or_else(|| tr!("%@?", &label));
        let row_id = row.id.clone();
        let confirmed_action = SharedString::from(format!("confirm:{action_id}"));
        let content = if self.config.presentation == PickerPresentation::Popover {
            popover::row(
                &self.theme,
                self.metrics,
                "picker-confirmation",
                false,
                false,
            )
        } else {
            div()
                .id("picker-confirmation")
                .px(self.metrics.scaled(layout.item_inset))
                .rounded(self.metrics.scaled(layout.row_radius))
                .flex()
                .items_center()
                .gap(self.metrics.spacing3())
                .bg(self.theme.danger.opacity(0.12))
        };
        let content = content
            .w_full()
            .h_full()
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_grow()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg)
                    .truncate()
                    .child(message),
            )
            .child(
                div()
                    .id("picker-confirm-cancel")
                    .text_color(self.theme.fg)
                    .px(self.metrics.spacing3())
                    .h(self.metrics.control_small())
                    .flex()
                    .items_center()
                    .rounded(self.metrics.radius_sm())
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .child(tr!("Cancel"))
                    .on_click(cx.listener(|popover, _, _, cx| {
                        let _ = popover.state.escape();
                        popover.confirmation_message = None;
                        cx.emit(PickerEvent::InlineActionDismissed);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("picker-confirm-action")
                    .px(self.metrics.spacing3())
                    .h(self.metrics.control_small())
                    .flex()
                    .items_center()
                    .rounded(self.metrics.radius_sm())
                    .cursor_pointer()
                    .text_color(self.theme.danger)
                    .hover(|style| style.bg(self.theme.hover))
                    .child(label)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(PickerEvent::RowAction {
                            row: row_id.clone(),
                            action: confirmed_action.clone(),
                        });
                    })),
            );
        div()
            .w_full()
            .h(self.metrics.scaled(layout.row_height(row)))
            .px(self.metrics.scaled(layout.outer_item_inset))
            .child(content)
            .into_any_element()
    }

    fn render_detail_item(&self, index: usize) -> AnyElement {
        let line = self
            .detail
            .as_ref()
            .and_then(|(_, lines)| lines.get(index))
            .cloned()
            .unwrap_or_default();
        div()
            .w_full()
            .min_h(self.metrics.scaled(20.0))
            .px(self.metrics.spacing5())
            .text_size(self.metrics.font_caption())
            .font_family("monospace")
            .text_color(self.theme.fg)
            .child(line)
            .into_any_element()
    }

    fn render_tab_strip(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.config.tabs.len() <= 1 {
            return div().into_any_element();
        }
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        let mut strip = div()
            .flex()
            .items_center()
            .border_1()
            .border_color(self.theme.border)
            .rounded(self.metrics.scaled(layout.row_radius))
            .overflow_hidden();
        for tab in self.config.tabs.clone() {
            let selected = tab.id.as_ref() == self.state.active_tab();
            let id = tab.id.clone();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("picker-tab-{}", tab.id)))
                    .px(self.metrics.spacing5())
                    .h(self.metrics.scaled(layout.tab_height))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(self.metrics.font_body())
                    .font_weight(if selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(if selected {
                        self.theme.accent
                    } else {
                        self.theme.fg_muted
                    })
                    .when(selected, |element| element.bg(self.theme.accent_soft))
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(self.theme.hover))
                    })
                    .on_click(cx.listener(move |popover, _, _, cx| {
                        let _ = popover.activate_tab(&id, cx);
                    }))
                    .child(tab.label),
            );
        }
        strip.into_any_element()
    }

    fn render_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let strip = self.render_tab_strip(cx);
        div()
            .flex()
            .items_center()
            .px(self.metrics.spacing5())
            .pt(self.metrics.spacing4())
            .pb(self.metrics.spacing2())
            .child(strip)
            .into_any_element()
    }

    fn render_header(&self) -> gpui::Div {
        if self.config.presentation == PickerPresentation::Popover {
            popover::header(&self.theme, self.metrics)
        } else {
            div()
                .flex()
                .flex_none()
                .items_center()
                .px(self.metrics.spacing4())
                .gap(self.metrics.spacing3())
                .border_b_1()
                .border_color(self.theme.border)
        }
    }

    fn render_footer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.config.footer_actions.is_empty() {
            return None;
        }
        let mut footer = if self.config.presentation == PickerPresentation::Popover {
            popover::footer(&self.theme, self.metrics)
        } else {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_end()
                .p(self.metrics.spacing3())
                .gap(self.metrics.spacing2())
                .border_t_1()
                .border_color(self.theme.border)
        };
        for action in self.config.footer_actions.clone() {
            let action_id = action.id.clone();
            let id = format!("picker-footer-{}", action.id);
            let button = controls::button(
                Style {
                    theme: &self.theme,
                    metrics: &self.metrics,
                },
                &id,
                &action.label,
                !action.disabled,
                cx.listener(move |_, _, _, cx| {
                    cx.emit(PickerEvent::FooterAction(action_id.clone()));
                }),
            )
            .debug_selector(move || id.clone())
            .gap(self.metrics.spacing2())
            .text_color(if action.destructive {
                self.theme.danger
            } else {
                self.theme.fg
            })
            .when_some(action.icon, |element, icon| {
                element.child(Self::render_leading(
                    &icon,
                    self.theme.fg_muted,
                    self.metrics,
                ))
            });
            footer = footer.child(button);
        }
        Some(footer.into_any_element())
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "Scrollbar calculations return to GPUI f32 pixel coordinates."
    )]
    fn scrollbar_geometry(&self) -> Option<PickerScrollbarGeometry> {
        let viewport = self.scroll.viewport_bounds();
        let visible = f64::from(viewport.size.height);
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        let scale = f32::from(self.metrics.scaled(1.0));
        let heights = scrollbar_item_heights(
            layout,
            scale,
            self.state.items(),
            self.detail.as_ref().map(|(_, lines)| lines.len()),
        );
        let vertical_inset = if self.detail.is_none() {
            f32::from(self.metrics.scaled(layout.list_vertical_inset)) * 2.0
        } else {
            0.0
        };
        let content = heights.iter().sum::<f32>() + vertical_inset;
        let maximum = f64::from((content - f32::from(viewport.size.height)).max(0.0));
        let inset = f64::from(self.metrics.spacing2());
        let track = (visible - inset * 2.0).max(0.0);
        let offset = f64::from(
            scrollbar_offset(&heights, self.scroll.logical_scroll_top()).clamp(0.0, maximum as f32),
        );
        let thumb = ThumbGeometry::from_lengths(
            visible + maximum,
            visible,
            offset,
            track,
            MINIMUM_THUMB_LENGTH,
        )?;
        Some(PickerScrollbarGeometry {
            track_origin: viewport.origin.y + self.metrics.spacing2(),
            track_length: px(track as f32),
            thumb_origin: px(thumb.origin as f32),
            thumb_length: px(thumb.length as f32),
            maximum_offset: px(maximum as f32),
        })
    }

    fn begin_scrollbar_drag(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(geometry) = self.scrollbar_geometry() else {
            return;
        };
        let thumb_top = geometry.track_origin + geometry.thumb_origin;
        let thumb_bottom = thumb_top + geometry.thumb_length;
        let grab = if event.position.y >= thumb_top && event.position.y <= thumb_bottom {
            event.position.y - thumb_top
        } else {
            geometry.thumb_length / 2.0
        };
        self.scroll.scrollbar_drag_started();
        self.scrollbar_drag = Some(grab);
        self.drag_scrollbar_to(event.position.y, geometry);
        cx.stop_propagation();
        cx.notify();
    }

    fn drag_scrollbar(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() || self.scrollbar_drag.is_none() {
            return;
        }
        let Some(geometry) = self.scrollbar_geometry() else {
            return;
        };
        self.drag_scrollbar_to(event.position.y, geometry);
        cx.stop_propagation();
        cx.notify();
    }

    fn drag_scrollbar_to(&self, pointer_y: Pixels, geometry: PickerScrollbarGeometry) {
        let travel = geometry.track_length - geometry.thumb_length;
        if travel <= px(0.0) {
            return;
        }
        let grab = self.scrollbar_drag.unwrap_or(geometry.thumb_length / 2.0);
        let origin = (pointer_y - geometry.track_origin - grab).clamp(px(0.0), travel);
        let offset = geometry.maximum_offset * (origin / travel);
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        let heights = scrollbar_item_heights(
            layout,
            f32::from(self.metrics.scaled(1.0)),
            self.state.items(),
            self.detail.as_ref().map(|(_, lines)| lines.len()),
        );
        self.scroll
            .scroll_to(scrollbar_list_offset(&heights, f32::from(offset)));
    }

    fn end_scrollbar_drag(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.scrollbar_drag.take().is_some() {
            self.scroll.scrollbar_drag_ended();
            cx.notify();
        }
    }

    fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let geometry = self.scrollbar_geometry()?;
        let dragging = self.scrollbar_drag.is_some();
        Some(
            div()
                .id("picker-scrollbar")
                .debug_selector(|| "picker-scrollbar".into())
                .absolute()
                .right(self.metrics.spacing1())
                .when(
                    self.config.presentation == PickerPresentation::Popover,
                    |element| element.right(-self.metrics.scaled(popover::PADDING)),
                )
                .top(self.metrics.spacing2())
                .w(self.metrics.scaled(8.0))
                .h(geometry.track_length)
                .cursor_pointer()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(Self::begin_scrollbar_drag),
                )
                .child(
                    div()
                        .absolute()
                        .right(self.metrics.spacing1())
                        .top(geometry.thumb_origin)
                        .w(self.metrics.scaled(if dragging { 5.0 } else { 4.0 }))
                        .h(geometry.thumb_length)
                        .rounded(self.metrics.radius_sm())
                        .bg(self
                            .theme
                            .fg_muted
                            .opacity(if dragging { 0.72 } else { 0.46 })),
                )
                .into_any_element(),
        )
    }
}

#[derive(Clone, Copy)]
struct PickerScrollbarGeometry {
    track_origin: Pixels,
    track_length: Pixels,
    thumb_origin: Pixels,
    thumb_length: Pixels,
    maximum_offset: Pixels,
}

impl Render for Picker {
    #[allow(
        clippy::too_many_lines,
        reason = "The declarative popup tree keeps its focus and layout behavior together."
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused
            && (self.detail.is_some()
                || self.focus_handle.is_focused(window)
                || self.config.presentation != PickerPresentation::Embedded)
        {
            self.focused = true;
            if self.detail.is_some() {
                window.focus(&self.focus_handle);
            } else {
                window.focus(&self.input.focus_handle(cx));
            }
        }
        let viewport = window.viewport_size();
        let layout = PickerLayout::resolve(self.config.presentation, self.config.compact);
        let mut geometry = PickerMetrics::default().scaled(self.metrics).resolve(
            self.config.presentation,
            f32::from(viewport.width),
            f32::from(viewport.height),
        );
        if let Some(width) = self.config.width {
            geometry.width = f32::from(self.metrics.scaled(width)).min(
                (f32::from(viewport.width) - PickerMetrics::default().viewport_margin * 2.0)
                    .max(0.0),
            );
        }
        if self.config.compact {
            geometry.height = geometry.height.min(f32::from(self.metrics.scaled(260.0)));
        }
        geometry.height = self.fitted_height(layout).min(geometry.height);
        let surface = if self.config.presentation == PickerPresentation::Embedded {
            div()
                .occlude()
                .flex()
                .flex_col()
                .bg(self.theme.surface)
                .rounded(self.metrics.scaled(layout.panel_radius))
                .border_1()
                .border_color(self.theme.border)
        } else {
            popover::surface(&self.theme, self.metrics).when(
                self.config.presentation == PickerPresentation::Modal,
                |surface| {
                    surface
                        .p_0()
                        .gap_0()
                        .shadow(crate::theme::Elevation::Modal.shadow(self.theme.bg))
                },
            )
        };
        let mut panel = surface
            .id(self.config.id.clone())
            .debug_selector(|| self.config.id.to_string())
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::secondary_confirm))
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::tab_pressed))
            .on_action(cx.listener(Self::navigate_back))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(cx.listener(Self::drag_scrollbar))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(Self::end_scrollbar_drag),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(Self::end_scrollbar_drag),
            )
            .when(
                self.config.presentation == PickerPresentation::Embedded,
                Styled::w_full,
            )
            .when(
                self.config.presentation != PickerPresentation::Embedded,
                |element| element.w(px(geometry.width)),
            )
            .h(px(geometry.height))
            .overflow_hidden();
        if self.config.tabs.len() > 1 && !layout.inline_tabs {
            panel = panel.child(self.render_tabs(cx));
        }
        panel = if let Some((title, _)) = &self.detail {
            let title = title.clone();
            panel.child(
                self.render_header()
                    .id("picker-detail-header")
                    .h(self.metrics.scaled(layout.header_height))
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .on_click(cx.listener(|popover, _, window, cx| {
                        popover.dismiss(&Dismiss, window, cx);
                    }))
                    .child(IconGlyph::new(
                        Icon::ChevronLeft,
                        self.metrics.icon_md(),
                        self.theme.fg_muted,
                    ))
                    .child(title),
            )
        } else {
            let has_inline_tabs = self.config.tabs.len() > 1 && layout.inline_tabs;
            let inline_tabs = has_inline_tabs.then(|| self.render_tab_strip(cx));
            panel.child(
                self.render_header()
                    .h(self.metrics.scaled(layout.header_height))
                    .when(
                        !self.can_navigate_back
                            && self.config.presentation != PickerPresentation::Popover,
                        |element| {
                            element.child(IconGlyph::new(
                                Icon::Search,
                                self.metrics.icon_md(),
                                self.theme.fg_muted,
                            ))
                        },
                    )
                    .when(self.can_navigate_back, |element| {
                        element.child(
                            div()
                                .id("picker-back")
                                .debug_selector(|| "picker-back".into())
                                .size(self.metrics.control_small())
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(self.metrics.radius_sm())
                                .cursor_pointer()
                                .hover(|style| style.bg(self.theme.hover))
                                .on_click(cx.listener(|popover, _, window, cx| {
                                    popover.navigate_back(&NavigateBack, window, cx);
                                }))
                                .child(IconGlyph::new(
                                    Icon::ChevronLeft,
                                    self.metrics.icon_md(),
                                    self.theme.fg_muted,
                                )),
                        )
                    })
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_grow()
                            .when(has_inline_tabs, |element| {
                                element.pr(self.metrics.spacing5())
                            })
                            .child(self.input.clone()),
                    )
                    .when_some(inline_tabs, ParentElement::child),
            )
        };
        let list = gpui::list(
            self.scroll.clone(),
            cx.processor(|popover, index: usize, window, cx| {
                if popover.detail.is_some() {
                    popover.render_detail_item(index)
                } else if popover.config.presentation == PickerPresentation::Popover {
                    let layout =
                        PickerLayout::resolve(popover.config.presentation, popover.config.compact);
                    let height = popover.state.items().get(index).map_or(0.0, |item| {
                        layout.item_extent(item, index, popover.state.item_count())
                    });
                    div()
                        .w_full()
                        .h(popover.metrics.scaled(height))
                        .child(popover.render_item(index, window, cx))
                        .into_any_element()
                } else {
                    popover.render_item(index, window, cx)
                }
            }),
        )
        .w_full()
        .when(
            self.config.presentation == PickerPresentation::Embedded || self.detail.is_some(),
            |element| element.pr(self.metrics.spacing5()),
        )
        .flex_grow()
        .min_h(px(0.0))
        .when(self.detail.is_none(), |element| {
            element.py(self.metrics.scaled(layout.list_vertical_inset))
        });
        let list = div()
            .relative()
            .min_h(px(0.0))
            .flex_grow()
            .flex()
            .flex_col()
            .child(list)
            .when_some(self.render_scrollbar(cx), |element, scrollbar| {
                element.child(scrollbar)
            });
        panel = match (self.detail.is_some(), self.state.status()) {
            (true, _) => panel.child(list),
            (false, PickerStatus::Ready) if self.state.item_count() > 0 => panel.child(list),
            (false, PickerStatus::Ready) => panel.child(status_message(
                tr!("No matches"),
                self.theme.fg_muted,
                self.metrics,
            )),
            (false, PickerStatus::Loading(message) | PickerStatus::Empty(message)) => {
                panel.child(status_message(message, self.theme.fg_muted, self.metrics))
            }
            (false, PickerStatus::Error(message)) => {
                panel.child(status_message(message, self.theme.danger, self.metrics))
            }
        };
        if let Some(footer) = self.render_footer(cx) {
            panel = panel.child(footer);
        }
        match self.config.presentation {
            PickerPresentation::Modal => div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .when(geometry.backdrop, |element| {
                    element.bg(gpui::hsla(0.0, 0.0, 0.0, 0.3))
                })
                .pt(px(geometry.top))
                .child(panel)
                .into_any_element(),
            PickerPresentation::Popover | PickerPresentation::Embedded => panel.into_any_element(),
        }
    }
}

fn status_message(message: impl Into<SharedString>, color: Hsla, metrics: Metrics) -> AnyElement {
    div()
        .flex_grow()
        .flex()
        .items_center()
        .justify_center()
        .text_size(metrics.font_body())
        .text_color(color)
        .child(message.into())
        .into_any_element()
}

impl std::fmt::Debug for Picker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picker")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("detail", &self.detail)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<PickerItem> {
        vec![
            PickerItem::section("Local Branches"),
            PickerItem::row("main"),
            PickerItem::row("feature"),
            PickerItem::section("Remote Branches"),
            PickerItem::row("origin/main"),
        ]
    }

    #[test]
    fn replacing_items_retains_identity_or_selects_the_first_actionable_row() {
        let mut state = PickerState::new(["branches"]);
        state.set_items(rows());
        state.select_last();
        assert_eq!(state.selected_row_id(), Some("origin/main"));

        state.set_items(vec![
            PickerItem::section("Local Branches"),
            PickerItem::row("main"),
            PickerItem::row("origin/main"),
        ]);
        assert_eq!(state.selected_row_id(), Some("origin/main"));

        state.set_items(vec![
            PickerItem::section("Local Branches"),
            PickerItem::row("main"),
        ]);
        assert_eq!(state.selected_row_id(), Some("main"));
    }
}

#[cfg(test)]
mod gpui_regression_tests {
    use super::*;
    use crate::theme::ColorScheme;
    use gpui::{TestAppContext, VisualTestContext};

    struct Host {
        popover: Entity<Picker>,
        events: Vec<PickerEvent>,
        _subscription: Subscription,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("popover-test-host")
                .size_full()
                .child(self.popover.clone())
        }
    }

    fn open(
        cx: &mut TestAppContext,
        presentation: PickerPresentation,
        completion_on_tab: bool,
    ) -> (Entity<Host>, &mut VisualTestContext) {
        cx.update(|cx| {
            cx.bind_keys(text_input::key_bindings());
            cx.bind_keys(key_bindings());
        });
        let (host, cx) = cx.add_window_view(|_, cx| {
            let popover = cx.new(|cx| {
                Picker::new(
                    PickerConfig {
                        id: "tested-popover".into(),
                        presentation,

                        tabs: vec![
                            PickerTab::new("first", "First"),
                            PickerTab::new("second", "Second"),
                        ],
                        placeholder: "Search".into(),
                        footer_actions: Vec::new(),

                        width: None,
                        compact: false,
                        completion_on_tab,
                        confirm_on_click: false,
                    },
                    Theme::from_scheme(&ColorScheme::default()),
                    Metrics::new(1.0),
                    cx,
                )
            });
            let subscription = cx.subscribe(&popover, |host: &mut Host, _, event, _| {
                host.events.push(event.clone());
            });
            Host {
                popover,
                events: Vec::new(),
                _subscription: subscription,
            }
        });
        let popover = host.read_with(cx, |host, _| host.popover.clone());
        cx.update(|window, cx| window.focus(&popover.read(cx).input.focus_handle(cx)));
        cx.run_until_parked();
        (host, cx)
    }

    #[gpui::test]
    fn hidden_status_rows_do_not_receive_keyboard_actions(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Popover, true);
        let popover = host.read_with(cx, |host, _| host.popover.clone());
        popover.update(cx, |popover, cx| {
            popover.set_items(vec![PickerItem::row("old"), PickerItem::row("next")], cx);
        });
        for status in [
            PickerStatus::Loading("Searching".into()),
            PickerStatus::Error("Failed".into()),
        ] {
            popover.update(cx, |popover, cx| popover.set_status(status, cx));
            cx.run_until_parked();
            cx.simulate_keystrokes("down up enter alt-enter cmd-enter tab shift-tab");
            assert!(host.read_with(cx, |host, _| host.events.clone()).is_empty());
        }
        popover.update(cx, |popover, cx| {
            popover.set_status(PickerStatus::Empty("No results".into()), cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("down up enter alt-enter cmd-enter tab shift-tab");
        assert_eq!(
            host.read_with(cx, |host, _| host.events.clone()),
            vec![
                PickerEvent::Submitted { secondary: false },
                PickerEvent::Submitted { secondary: true },
                PickerEvent::Submitted { secondary: true },
                PickerEvent::CompletionRequested,
            ]
        );
        host.update(cx, |host, _| host.events.clear());
        popover.update(cx, |popover, cx| {
            popover.set_status(PickerStatus::Ready, cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        assert_eq!(
            host.read_with(cx, |host, _| host.events.clone()),
            vec![PickerEvent::Confirmed(PickerSelection::new("old"))]
        );
    }
}
