use super::items::{Item, ItemAction, Scope};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyBinding, Render, Subscription,
};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverConfig, CommandPopoverDensity, CommandPopoverEvent,
    CommandPopoverHint, CommandPopoverItem, CommandPopoverLeading, CommandPopoverPresentation,
    CommandPopoverRow, CommandPopoverStatus, CommandPopoverTab,
};
use muxy_ui::theme::{Metrics, Theme};

pub fn key_bindings() -> Vec<KeyBinding> {
    Vec::new()
}

pub enum OmniboxEvent {
    QueryChanged,
    Confirm(ItemAction),
    Dismiss,
}

pub struct Omnibox {
    scope: Scope,
    rows: Vec<Item>,
    picker: Entity<CommandPopover>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<OmniboxEvent> for Omnibox {}

impl Focusable for Omnibox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}

impl Omnibox {
    pub fn new(scope: Scope, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| {
            CommandPopover::new(
                CommandPopoverConfig {
                    id: "omnibox".into(),
                    presentation: CommandPopoverPresentation::Modal,
                    density: CommandPopoverDensity::Comfortable,
                    tabs: vec![CommandPopoverTab::new("results", scope_title(scope))],
                    placeholder: scope.placeholder().into(),
                    footer_actions: Vec::new(),
                    footer_hints: vec![
                        CommandPopoverHint::new("↩", scope.return_label()),
                        CommandPopoverHint::new("Tab / ⇧Tab", "Navigate"),
                        CommandPopoverHint::new("Esc", "Close"),
                    ],
                    width: Some(720.0),
                    height: Some(460.0),
                    max_height: None,
                    completion_on_tab: false,
                    confirm_on_click: true,
                },
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&picker, |omnibox: &mut Self, _, event, cx| match event {
            CommandPopoverEvent::QueryChanged { .. } => cx.emit(OmniboxEvent::QueryChanged),
            CommandPopoverEvent::Confirmed(selection)
            | CommandPopoverEvent::SecondaryConfirmed(selection) => {
                let Some(id) = selection.id.strip_prefix("omnibox-") else {
                    return;
                };
                if let Some(item) = omnibox.rows.iter().find(|item| item.id == id) {
                    cx.emit(OmniboxEvent::Confirm(item.action.clone()));
                }
            }
            CommandPopoverEvent::Dismissed => cx.emit(OmniboxEvent::Dismiss),
            _ => {}
        });
        Self {
            scope,
            rows: Vec::new(),
            picker,
            _subscriptions: vec![subscription],
        }
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn query(&self, cx: &App) -> String {
        self.picker.read(cx).query().to_owned()
    }

    pub fn apply_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.scope = scope;
        self.rows.clear();
        self.picker.update(cx, |picker, cx| {
            picker.set_placeholder(scope.placeholder(), cx);
            picker.set_query(String::new(), cx);
            picker.set_items(Vec::new(), cx);
            picker.set_status(CommandPopoverStatus::Empty(scope.empty_state().into()), cx);
        });
    }

    pub fn set_rows(&mut self, rows: Vec<Item>, reset_highlight: bool, cx: &mut Context<Self>) {
        self.rows = rows;
        let mut items = Vec::new();
        let mut section = None;
        for item in &self.rows {
            if section.as_deref() != Some(item.section.as_str()) {
                section = Some(item.section.clone());
                items.push(CommandPopoverItem::section(item.section.clone()));
            }
            let mut row =
                CommandPopoverRow::new(format!("omnibox-{}", item.id), item.title.clone());
            row.subtitle = item.subtitle.clone().map(Into::into);
            row.leading = Some(CommandPopoverLeading::Symbol(item.symbol.clone().into()));
            items.push(CommandPopoverItem::Row(row));
        }
        let first = self.rows.first().map(|item| format!("omnibox-{}", item.id));
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            if self.rows.is_empty() {
                picker.set_status(
                    CommandPopoverStatus::Empty(self.scope.empty_state().into()),
                    cx,
                );
            } else if reset_highlight && let Some(first) = first.as_deref() {
                let _ = picker.select_row(first, cx);
            }
        });
    }
}

impl Render for Omnibox {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}

fn scope_title(scope: Scope) -> &'static str {
    match scope {
        Scope::OpenTabs => "Open Tabs",
        Scope::Projects => "Projects",
        Scope::RecentlyRemovedProjects => "Recently Removed",
        Scope::Worktrees => "Worktrees",
        Scope::Workspaces => "Workspaces",
        Scope::CommandShortcuts => "Commands",
    }
}
