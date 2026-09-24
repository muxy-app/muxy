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
            tabs: vec![PickerTab::new("items", "Items")],
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
            .map_or_else(|| "Confirm".into(), |action| action.label.clone());
        let message = self
            .confirmation_message
            .clone()
            .unwrap_or_else(|| format!("{label}?").into());
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
                    .child("Cancel")
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
                "No matches",
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
#[allow(
    clippy::float_cmp,
    reason = "These geometry cases use exactly representable values."
)]
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
    fn presentations_respect_requested_dimensions_and_fit_the_viewport() {
        let metrics = PickerMetrics {
            modal_width: 600.0,
            modal_height: 500.0,
            modal_top: 40.0,
            popover_width: 300.0,
            popover_height: 200.0,
            viewport_margin: 10.0,
        };
        let modal = metrics.resolve(PickerPresentation::Modal, 1440.0, 900.0);
        assert_eq!(modal.width, 600.0);
        assert_eq!(modal.height, 500.0);
        assert_eq!(modal.top, 40.0);
        assert!(modal.backdrop);

        let compact = metrics.resolve(PickerPresentation::Modal, 390.0, 400.0);
        assert_eq!(compact.width, 370.0);
        assert_eq!(compact.height, 360.0);
        assert_eq!(compact.top, 20.0);

        let popover = metrics.resolve(PickerPresentation::Popover, 1440.0, 900.0);
        assert_eq!(popover.width, 300.0);
        assert_eq!(popover.height, 200.0);
        assert!(!popover.backdrop);

        let embedded = metrics.resolve(PickerPresentation::Embedded, 900.0, 700.0);
        assert_eq!(embedded.width, 900.0);
        assert_eq!(embedded.height, 700.0);
        assert_eq!(embedded.top, 0.0);
        assert!(!embedded.backdrop);
    }

    #[test]
    fn tabs_preserve_query_selection_and_scroll_without_rebuilding_inactive_state() {
        let mut state = PickerState::new(["branches", "stashes"]);
        state.set_items(rows());
        state.set_query("feat");
        state.select_next();
        assert_eq!(state.selected_row_id(), Some("feature"));

        assert!(state.activate_tab("stashes").is_ok());
        state.set_items(vec![
            PickerItem::section("Stashes"),
            PickerItem::row("stash@{0}"),
            PickerItem::row("stash@{1}"),
        ]);
        state.set_query("wip");
        state.select_last();

        assert!(state.activate_tab("branches").is_ok());
        assert_eq!(state.query(), "feat");
        assert_eq!(state.selected_row_id(), Some("feature"));
        assert_eq!(state.item_count(), 5);

        assert!(state.activate_tab("stashes").is_ok());
        assert_eq!(state.query(), "wip");
        assert_eq!(state.selected_row_id(), Some("stash@{1}"));
    }

    #[test]
    fn keyboard_navigation_skips_sections_and_disabled_rows_and_wraps() {
        let mut state = PickerState::new(["branches"]);
        state.set_items(vec![
            PickerItem::section("Local Branches"),
            PickerItem::row("main"),
            PickerItem::row("busy").disabled(true),
            PickerItem::section("Remote Branches"),
            PickerItem::row("origin/main"),
        ]);

        assert_eq!(state.selected_row_id(), Some("main"));
        state.select_next();
        assert_eq!(state.selected_row_id(), Some("origin/main"));
        state.select_next();
        assert_eq!(state.selected_row_id(), Some("main"));
        state.select_previous();
        assert_eq!(state.selected_row_id(), Some("origin/main"));
    }

    #[test]
    fn status_messages_hide_stale_actions_until_items_are_ready() {
        let mut state = PickerState::new(["main"]);
        state.set_items(vec![PickerItem::row("old"), PickerItem::row("next")]);
        for status in [
            PickerStatus::Loading("Searching".into()),
            PickerStatus::Empty("No results".into()),
            PickerStatus::Error("Failed".into()),
        ] {
            state.set_status(status);
            state.select_next();
            state.select_last();
            assert!(state.selected_row_id().is_none());
            assert!(state.confirm().is_none());
            assert!(state.select_row("old").is_err());
            assert!(state.open_inline_action("old", "delete").is_err());
        }
        state.set_status(PickerStatus::Ready);
        assert_eq!(state.selected_row_id(), Some("old"));
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

    #[test]
    fn escape_closes_nested_actions_before_the_surface_and_confirm_is_identity_based() {
        let mut state = PickerState::new(["branches"]);
        state.set_items(rows());
        assert!(state.open_inline_action("feature", "delete").is_ok());
        assert_eq!(state.escape(), PickerEscape::CloseInlineAction);
        assert_eq!(state.escape(), PickerEscape::Dismiss);

        assert!(state.select_row("feature").is_ok());
        assert_eq!(state.confirm(), Some(PickerSelection::new("feature")));
    }

    #[test]
    fn popover_height_includes_shell_spacing_and_only_inter_item_gaps() {
        let layout = PickerLayout::resolve(PickerPresentation::Popover, true);
        let items = vec![
            PickerItem::row("one"),
            PickerItem::row("two"),
            PickerItem::row("three"),
        ];
        assert_eq!(layout.row_radius, 4.0);
        assert_eq!(layout.item_inset, popover::ROW_PADDING);
        assert_eq!(
            content_height(layout, 1, &items, &PickerStatus::Ready, None, false),
            113.0
        );
        assert_eq!(
            content_height(layout, 2, &items, &PickerStatus::Ready, None, true),
            143.0
        );
        assert_eq!(
            content_height(layout, 1, &[], &PickerStatus::Ready, None, false),
            79.0
        );
        assert_eq!(
            content_height(layout, 1, &items, &PickerStatus::Ready, Some(3), false),
            99.0
        );
        assert_eq!(
            scrollbar_item_heights(layout, 1.0, &items, None),
            [25.0, 25.0, 24.0]
        );
    }

    #[test]
    fn rich_popover_rows_and_sections_share_the_scrollbar_spacing_model() {
        let layout = PickerLayout::resolve(PickerPresentation::Popover, false);
        let mut rich = PickerRow::new("rich", "Rich");
        rich.detail = Some("Detail".into());
        let items = vec![
            PickerItem::section("Group"),
            PickerItem::row("plain"),
            PickerItem::Row(rich),
        ];
        assert_eq!(
            content_height(layout, 1, &items, &PickerStatus::Ready, None, false),
            123.0
        );
        let heights = scrollbar_item_heights(layout, 1.5, &items, None);
        assert_eq!(heights, [34.5, 37.5, 48.0]);
        for target in [0.0, 33.0, 34.0, 34.5, 35.0, 71.0, 72.0, 120.0] {
            assert_eq!(
                scrollbar_offset(&heights, scrollbar_list_offset(&heights, target)),
                target
            );
        }
        assert_eq!(
            scrollbar_item_heights(layout, 1.5, &items, Some(3)),
            [30.0; 3]
        );
    }

    #[test]
    fn other_presentations_keep_their_row_heights_and_list_insets() {
        let items = vec![
            PickerItem::row("one"),
            PickerItem::row("two"),
            PickerItem::row("three"),
        ];
        let modal = PickerLayout::resolve(PickerPresentation::Modal, false);
        let embedded = PickerLayout::resolve(PickerPresentation::Embedded, false);
        for layout in [modal, embedded] {
            assert_eq!(scrollbar_item_heights(layout, 1.0, &items, None), [32.0; 3]);
            assert_eq!(layout.list_vertical_inset, 4.0);
            assert_eq!(layout.row_gap, 0.0);
        }
        assert_eq!(
            content_height(modal, 2, &items, &PickerStatus::Ready, None, false),
            174.0
        );
        assert_eq!(
            content_height(embedded, 1, &items, &PickerStatus::Ready, None, false),
            138.0
        );
    }

    #[test]
    fn scrollbar_maps_the_full_unmeasured_logical_list() {
        let layout = PickerLayout::resolve(PickerPresentation::Popover, false);
        let mut items = vec![PickerItem::section("Providers")];
        items.extend((0..20).map(|index| PickerItem::row(index.to_string())));
        let heights = scrollbar_item_heights(layout, 1.0, &items, None);
        let content = heights.iter().sum::<f32>();
        let visible = 160.0;
        let target = content - visible;
        let offset = scrollbar_list_offset(&heights, target);

        assert!(offset.item_ix > 0);
        assert_eq!(scrollbar_offset(&heights, offset), target);
        assert_eq!(scrollbar_offset(&heights, offset) + visible, content);
    }
}

#[cfg(test)]
mod gpui_regression_tests {
    use super::*;
    use crate::theme::ColorScheme;
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

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

    fn assert_input_focused(popover: &Entity<Picker>, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            assert!(
                popover.read(cx).input.focus_handle(cx).is_focused(window),
                "input lost focus in {:?}",
                popover.read(cx).config.presentation,
            );
        });
    }

    #[gpui::test]
    fn footer_buttons_preserve_disabled_actions_and_support_keyboard_activation(
        cx: &mut TestAppContext,
    ) {
        let (host, cx) = open(cx, PickerPresentation::Popover, false);
        cx.update(|_, cx| {
            let mut registry = crate::shortcuts::Registry::new(&muxy_core::shortcuts::Defaults);
            crate::components::register_shortcuts(&mut registry);
            cx.bind_keys(registry.into_bindings());
        });
        let picker = host.read_with(cx, |host, _| host.popover.clone());
        picker.update(cx, |picker, cx| {
            picker.set_items(vec![PickerItem::row("branch")], cx);
            picker.set_footer_actions(
                vec![
                    PickerAction::new("disabled", "Disabled").disabled(true),
                    PickerAction::new("create", "New Branch…"),
                ],
                cx,
            );
        });
        cx.simulate_resize(gpui::size(px(600.0), px(400.0)));
        cx.run_until_parked();
        let panel = cx.debug_bounds("tested-popover").expect("panel");
        let disabled = cx.debug_bounds("picker-footer-disabled").expect("disabled");
        let create = cx.debug_bounds("picker-footer-create").expect("create");
        assert!(create.bottom() < panel.bottom());
        assert!(disabled.left() > panel.left() && create.right() < panel.right());
        cx.simulate_click(disabled.center(), Modifiers::none());
        cx.run_until_parked();
        host.read_with(cx, |host, _| {
            assert!(
                !host
                    .events
                    .iter()
                    .any(|event| matches!(event, PickerEvent::FooterAction(_)))
            );
        });
        cx.simulate_click(create.center(), Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.focus(&picker.read(cx).input.focus_handle(cx));
            window.focus_next();
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        host.read_with(cx, |host, _| {
            let actions: Vec<_> = host
                .events
                .iter()
                .filter_map(|event| match event {
                    PickerEvent::FooterAction(action) => Some(action.as_ref()),
                    _ => None,
                })
                .collect();
            assert_eq!(actions, ["create", "create"]);
            assert!(
                !host
                    .events
                    .iter()
                    .any(|event| matches!(event, PickerEvent::Confirmed(_)))
            );
        });
    }

    #[gpui::test]
    fn popover_rows_fit_the_shared_insets_and_gaps_at_each_scale(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Popover, false);
        let picker = host.read_with(cx, |host, _| host.popover.clone());
        for (compact, scale, has_footer) in
            [(true, 1.0, false), (false, 1.5, false), (true, 1.5, true)]
        {
            let metrics = Metrics::new(scale);
            picker.update(cx, |picker, cx| {
                picker.config.compact = compact;
                picker.set_items(
                    vec![
                        PickerItem::row("one"),
                        PickerItem::row("two"),
                        PickerItem::row("three"),
                    ],
                    cx,
                );
                picker.set_footer_actions(
                    if has_footer {
                        vec![PickerAction::new("done", "Done")]
                    } else {
                        Vec::new()
                    },
                    cx,
                );
                picker.set_appearance(Theme::from_scheme(&ColorScheme::default()), metrics, cx);
            });
            cx.simulate_resize(gpui::size(px(800.0), px(600.0)));
            cx.run_until_parked();
            let panel = cx.debug_bounds("tested-popover").expect("panel");
            let first = cx.debug_bounds("picker-row-one").expect("first row");
            let second = cx.debug_bounds("picker-row-two").expect("second row");
            let last = cx.debug_bounds("picker-row-three").expect("last row");
            assert_eq!(
                first.left() - panel.left(),
                px(1.0) + metrics.scaled(popover::PADDING)
            );
            assert_eq!(
                panel.right() - first.right(),
                px(1.0) + metrics.scaled(popover::PADDING)
            );
            assert_eq!(first.size.height, metrics.scaled(popover::ROW_HEIGHT));
            assert_eq!(
                second.top() - first.bottom(),
                metrics.scaled(popover::ROW_GAP)
            );
            assert_eq!(
                last.top() - second.bottom(),
                metrics.scaled(popover::ROW_GAP)
            );
            let bottom = if has_footer {
                let footer = cx
                    .debug_bounds("picker-footer-done")
                    .expect("footer button");
                assert_eq!(
                    footer.top() - last.bottom(),
                    metrics.scaled(popover::ROW_GAP + 4.0) + px(1.0)
                );
                footer.bottom()
            } else {
                last.bottom()
            };
            assert_eq!(
                panel.bottom() - bottom,
                metrics.scaled(popover::PADDING) + px(1.0)
            );
            picker.read_with(cx, |picker, _| {
                let heights = scrollbar_item_heights(
                    PickerLayout::resolve(PickerPresentation::Popover, compact),
                    scale,
                    picker.state.items(),
                    None,
                );
                assert_eq!(
                    picker.scroll.viewport_bounds().size.height,
                    px(heights.iter().sum())
                );
                assert!(picker.scrollbar_geometry().is_none());
            });
        }
    }

    #[gpui::test]
    fn popover_scrollbar_stays_in_padded_edge_and_reaches_the_last_row(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Popover, false);
        let picker = host.read_with(cx, |host, _| host.popover.clone());
        picker.update(cx, |picker, cx| {
            picker.set_items(
                (0..40)
                    .map(|index| PickerItem::row(format!("row-{index}")))
                    .collect(),
                cx,
            );
        });
        cx.simulate_resize(gpui::size(px(800.0), px(600.0)));
        cx.run_until_parked();
        let first = cx.debug_bounds("picker-row-row-0").expect("first row");
        let scrollbar = cx.debug_bounds("picker-scrollbar").expect("scrollbar");
        let panel = cx.debug_bounds("tested-popover").expect("panel");
        assert!(scrollbar.left() >= first.right() - px(popover::ROW_PADDING));
        assert_eq!(scrollbar.right(), panel.right() - px(1.0));
        picker.update(cx, |picker, cx| {
            let geometry = picker.scrollbar_geometry().expect("scrollbar geometry");
            picker.scrollbar_drag = Some(px(0.0));
            picker.drag_scrollbar_to(geometry.track_origin + geometry.track_length, geometry);
            picker.scrollbar_drag = None;
            cx.notify();
        });
        cx.run_until_parked();
        let last = cx.debug_bounds("picker-row-row-39").expect("last row");
        assert_eq!(last.bottom(), panel.bottom() - px(1.0 + popover::PADDING));
        picker.read_with(cx, |picker, _| {
            let geometry = picker.scrollbar_geometry().expect("scrollbar geometry");
            let heights = scrollbar_item_heights(
                PickerLayout::resolve(PickerPresentation::Popover, false),
                1.0,
                picker.state.items(),
                None,
            );
            assert_eq!(
                px(scrollbar_offset(
                    &heights,
                    picker.scroll.logical_scroll_top()
                )),
                geometry.maximum_offset
            );
        });
    }

    #[gpui::test]
    fn compact_rows_keep_details_inline_and_fit_scaled_small_windows(cx: &mut TestAppContext) {
        for presentation in [
            PickerPresentation::Modal,
            PickerPresentation::Popover,
            PickerPresentation::Embedded,
        ] {
            let (host, cx) = open(cx, presentation, false);
            let picker = host.read_with(cx, |host, _| host.popover.clone());
            picker.update(cx, |picker, cx| {
                let mut row = PickerRow::new("inline", "Project with a long descriptive name");
                row.detail = Some("~/Projects/another/very/long/directory/name".into());
                row.trailing = Some("Owner: Desktop 0123456789abcdef".into());
                picker.set_items(vec![PickerItem::Row(row)], cx);
                picker.set_appearance(
                    Theme::from_scheme(&ColorScheme::default()),
                    Metrics::new(1.5),
                    cx,
                );
            });
            cx.simulate_resize(gpui::size(px(390.0), px(240.0)));
            cx.run_until_parked();
            let row = cx.debug_bounds("picker-row-inline").expect("row");
            let title = cx.debug_bounds("picker-title-inline").expect("title");
            let detail = cx.debug_bounds("picker-detail-inline").expect("detail");
            assert_eq!(row.size.height, px(48.0));
            assert_eq!(title.center().y, detail.center().y);
            assert!(title.size.width > px(0.0) && detail.size.width > px(0.0));
            assert!(detail.left() >= title.right());
            assert!(row.right() <= px(390.0) && row.bottom() <= px(240.0));
        }
    }

    #[gpui::test]
    fn theme_swatches_fit_beside_left_aligned_names_and_overlap_by_half(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Modal, false);
        let picker = host.read_with(cx, |host, _| host.popover.clone());
        for (width, scale) in [(1000.0, 1.0), (390.0, 1.0), (390.0, 1.5)] {
            let metrics = Metrics::new(scale);
            picker.update(cx, |picker, cx| {
                let mut row = PickerRow::new("theme", "A theme with a very long descriptive name");
                row.selection_style = PickerSelectionStyle::Highlight;
                row.current = true;
                row.swatches = vec![gpui::rgb(0x12_34_56).into(); 16];
                picker.set_items(vec![PickerItem::Row(row)], cx);
                picker.set_appearance(Theme::from_scheme(&ColorScheme::default()), metrics, cx);
            });
            cx.simulate_resize(gpui::size(px(width), px(600.0)));
            cx.run_until_parked();
            let row = cx.debug_bounds("picker-row-theme").expect("theme row");
            let title = cx.debug_bounds("picker-title-theme").expect("theme name");
            assert_eq!(title.left(), row.left() + metrics.scaled(8.0));
            assert!(title.size.width > px(0.0));
            let mut previous = None;
            for index in 0..16 {
                let swatch = cx
                    .debug_bounds(format!("picker-swatch-theme-{index}").leak())
                    .expect("color circle");
                assert_eq!(
                    swatch.size,
                    gpui::size(metrics.scaled(20.0), metrics.scaled(20.0))
                );
                assert!(swatch.left() >= title.right());
                assert!(swatch.right() <= row.right() - metrics.scaled(8.0));
                assert!(swatch.top() >= row.top() && swatch.bottom() <= row.bottom());
                if let Some(left) = previous {
                    assert_eq!(swatch.left() - left, metrics.scaled(10.0));
                }
                previous = Some(swatch.left());
            }
        }
    }

    #[gpui::test]
    fn changing_current_theme_preserves_query_selection_and_scroll(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Modal, false);
        let picker = host.read_with(cx, |host, _| host.popover.clone());
        picker.update(cx, |picker, cx| {
            picker.set_items(
                (0..40)
                    .map(|index| PickerItem::row(format!("theme-{index}")))
                    .collect(),
                cx,
            );
            picker.set_query("theme", cx);
            picker.select_row("theme-15", cx).expect("theme row");
            picker.scroll.scroll_to(ListOffset {
                item_ix: 10,
                offset_in_item: px(5.0),
            });
        });
        cx.run_until_parked();
        let before = picker.read_with(cx, |picker, _| picker.scroll.logical_scroll_top());
        picker.update(cx, |picker, cx| {
            picker.set_current_row("theme-15", cx);
            picker.set_appearance(
                Theme::from_scheme(&ColorScheme::parse(
                    "background = abcdef\nforeground = 123456\n",
                )),
                Metrics::new(1.0),
                cx,
            );
        });
        cx.run_until_parked();
        picker.read_with(cx, |picker, _| {
            assert_eq!(picker.query(), "theme");
            assert_eq!(picker.state.selected_row_id(), Some("theme-15"));
            let after = picker.scroll.logical_scroll_top();
            assert_eq!(after.item_ix, before.item_ix);
            assert_eq!(after.offset_in_item, before.offset_in_item);
            assert!(matches!(&picker.state.items()[15], PickerItem::Row(row) if row.current));
        });
        assert_input_focused(&picker, cx);
    }

    #[gpui::test]
    fn unchanged_refresh_preserves_an_open_row_confirmation(cx: &mut TestAppContext) {
        let (host, cx) = open(cx, PickerPresentation::Popover, false);
        let popover = host.read_with(cx, |host, _| host.popover.clone());
        popover.update(cx, |popover, cx| {
            let mut row = PickerRow::new("feature", "feature");
            row.actions.push(PickerAction::new("delete", "Delete"));
            let items = vec![PickerItem::Row(row)];
            popover.set_items(items.clone(), cx);
            popover
                .open_confirmation_with_message("feature", "delete", Some("Delete?".into()), cx)
                .expect("confirmation");
            popover.set_items(items, cx);
            popover.set_status(PickerStatus::Ready, cx);
            assert_eq!(popover.state.inline_action(), Some(("feature", "delete")));
            assert_eq!(popover.confirmation_message, Some("Delete?".into()));
        });
    }

    #[gpui::test]
    fn nested_detail_escape_restores_input_focus(cx: &mut TestAppContext) {
        for presentation in [
            PickerPresentation::Modal,
            PickerPresentation::Popover,
            PickerPresentation::Embedded,
        ] {
            let (host, cx) = open(cx, presentation, false);
            let popover = host.read_with(cx, |host, _| host.popover.clone());
            popover.update(cx, |popover, cx| {
                popover.set_items(vec![PickerItem::row("first")], cx);
                popover.show_detail("Details", "one\ntwo\nthree", cx);
            });
            cx.run_until_parked();
            cx.update(|window, cx| assert!(popover.read(cx).focus_handle.is_focused(window)));
            cx.simulate_keystrokes("escape");
            assert!(popover.read_with(cx, |popover, _| popover.detail.is_none()));
            assert_eq!(
                popover.read_with(cx, |popover, _| popover.scroll.item_count()),
                1
            );
            assert_input_focused(&popover, cx);
            assert!(
                !host
                    .read_with(cx, |host, _| host.events.clone())
                    .contains(&PickerEvent::Dismissed)
            );
            cx.simulate_keystrokes("escape");
            assert_eq!(
                host.read_with(cx, |host, _| host.events.clone()),
                vec![PickerEvent::Dismissed]
            );
        }
    }

    #[gpui::test]
    fn detail_refresh_and_tab_change_keep_the_visible_content_consistent(cx: &mut TestAppContext) {
        for presentation in [
            PickerPresentation::Modal,
            PickerPresentation::Popover,
            PickerPresentation::Embedded,
        ] {
            let (host, cx) = open(cx, presentation, false);
            let popover = host.read_with(cx, |host, _| host.popover.clone());
            popover.update(cx, |popover, cx| {
                popover.show_detail("Details", "one\ntwo\nthree", cx);
            });
            cx.run_until_parked();
            popover.update(cx, |popover, cx| {
                popover.set_items(vec![PickerItem::row("replacement")], cx);
            });
            cx.run_until_parked();
            assert_eq!(
                popover.read_with(cx, |popover, _| popover.scroll.item_count()),
                3
            );
            cx.simulate_keystrokes("down enter alt-enter tab");
            assert!(host.read_with(cx, |host, _| host.events.clone()).is_empty());
            cx.simulate_keystrokes("ctrl-tab");
            assert_eq!(
                popover.read_with(cx, |popover, _| popover.active_tab().to_owned()),
                "second"
            );
            assert!(popover.read_with(cx, |popover, _| popover.detail.is_none()));
            assert_eq!(
                popover.read_with(cx, |popover, _| popover.scroll.item_count()),
                0
            );
            assert_input_focused(&popover, cx);
            assert!(
                host.read_with(cx, |host, _| host.events.clone())
                    .contains(&PickerEvent::TabChanged("second".into()))
            );
            cx.simulate_keystrokes("ctrl-shift-tab enter");
            assert!(
                host.read_with(cx, |host, _| host.events.clone())
                    .contains(&PickerEvent::Confirmed(PickerSelection::new("replacement")))
            );
        }
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
