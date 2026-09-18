use std::fmt;
use std::rc::Rc;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla, IntoElement,
    Render, SharedString, Subscription, Window,
};

use crate::picker::{
    Picker, PickerConfig, PickerEvent, PickerItem, PickerRow, PickerSelectionStyle, PickerStatus,
};
use crate::theme::{Metrics, Theme};

type ListProvider<A> = Rc<dyn Fn(&App) -> Registry<A>>;

#[derive(Clone)]
enum Target<A> {
    Action(A),
    List(ListProvider<A>),
}

#[derive(Clone)]
pub struct Command<A> {
    id: SharedString,
    title: SharedString,
    shortcut: Option<SharedString>,
    keywords: String,
    disabled: bool,
    swatches: Vec<Hsla>,
    current: bool,
    keep_open: bool,
    target: Target<A>,
}

impl<A> fmt::Debug for Command<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl<A> Command<A> {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>, action: A) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: None,
            keywords: String::new(),
            disabled: false,
            swatches: Vec::new(),
            current: false,
            keep_open: false,
            target: Target::Action(action),
        }
    }

    pub fn list(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        provider: impl Fn(&App) -> Registry<A> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: None,
            keywords: String::new(),
            disabled: false,
            swatches: Vec::new(),
            current: false,
            keep_open: false,
            target: Target::List(Rc::new(provider)),
        }
    }

    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    #[must_use]
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = keywords.into();
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[must_use]
    pub fn swatches(mut self, colors: Vec<Hsla>) -> Self {
        self.swatches = colors;
        self
    }

    #[must_use]
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    #[must_use]
    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }

    fn matches(&self, query: &str) -> bool {
        let searchable = format!("{} {}", self.title, self.keywords).to_lowercase();
        query
            .split_whitespace()
            .all(|word| searchable.contains(word))
    }

    fn row(&self) -> PickerItem {
        let mut row = PickerRow::new(self.id.clone(), self.title.clone());
        row.selection_style = PickerSelectionStyle::Highlight;
        row.disabled = self.disabled;
        row.swatches.clone_from(&self.swatches);
        row.current = self.current;
        row.trailing = match self.target {
            Target::Action(_) => self.shortcut.clone(),
            Target::List(_) => Some("›".into()),
        };
        PickerItem::Row(row)
    }
}

#[derive(Clone)]
pub struct Registry<A> {
    commands: Vec<Command<A>>,
}

impl<A> fmt::Debug for Registry<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("commands", &self.commands)
            .finish()
    }
}

impl<A> Default for Registry<A> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

impl<A> Registry<A> {
    pub fn register(&mut self, command: Command<A>) -> Option<Command<A>> {
        if let Some(existing) = self
            .commands
            .iter_mut()
            .find(|entry| entry.id == command.id)
        {
            return Some(std::mem::replace(existing, command));
        }
        self.commands.push(command);
        None
    }

    fn items(&self, query: &str) -> Vec<PickerItem> {
        let query = query.to_lowercase();
        self.commands
            .iter()
            .filter(|command| command.matches(&query))
            .map(Command::row)
            .collect()
    }
}

struct Page<A> {
    id: Option<SharedString>,
    registry: Registry<A>,
    title: SharedString,
    query: String,
    selected: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub enum CommandPaletteEvent<A> {
    Selected(A),
    Applied(A),
    Dismissed,
}

pub struct CommandPalette<A> {
    picker: Entity<Picker>,
    page: Page<A>,
    parents: Vec<Page<A>>,
    _subscription: Subscription,
}

impl<A> fmt::Debug for CommandPalette<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandPalette")
            .field("commands", &self.page.registry)
            .field("depth", &self.parents.len())
            .finish_non_exhaustive()
    }
}

impl<A: Clone + 'static> EventEmitter<CommandPaletteEvent<A>> for CommandPalette<A> {}

impl<A: Clone + 'static> Focusable for CommandPalette<A> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.read(cx).input().focus_handle(cx)
    }
}

impl<A: Clone + 'static> CommandPalette<A> {
    pub fn new(
        commands: Registry<A>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::new("command-palette", "Search commands…"),
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&picker, |palette: &mut Self, _, event, cx| match event {
            PickerEvent::QueryChanged { query, .. } => {
                palette.page.query = query.to_string();
                palette.refresh(cx);
            }
            PickerEvent::SelectionChanged(selection) => {
                palette.page.selected = Some(selection.id.clone());
            }
            PickerEvent::Confirmed(selection) => palette.confirm(&selection.id, cx),
            PickerEvent::NavigateBackRequested => palette.back(cx),
            PickerEvent::Dismissed if !palette.parents.is_empty() => palette.back(cx),
            PickerEvent::Dismissed => cx.emit(CommandPaletteEvent::Dismissed),
            _ => {}
        });
        let mut palette = Self {
            picker,
            page: Page {
                id: None,
                registry: commands,
                title: "Search commands…".into(),
                query: String::new(),
                selected: None,
            },
            parents: Vec::new(),
            _subscription: subscription,
        };
        palette.refresh(cx);
        palette
    }

    pub fn set_appearance(&self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
    }

    pub fn set_current(&mut self, id: &str, cx: &mut Context<Self>) {
        for command in &mut self.page.registry.commands {
            command.current = command.id == id;
        }
        self.picker
            .update(cx, |picker, cx| picker.set_current_row(id, cx));
    }

    pub fn is_page(&self, id: &str) -> bool {
        self.page.id.as_ref().is_some_and(|page| page == id)
    }

    pub fn current_id(&self) -> Option<&str> {
        self.page
            .registry
            .commands
            .iter()
            .find(|command| command.current)
            .map(|command| command.id.as_ref())
    }

    pub fn open_root_page(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(command) = self
            .page
            .registry
            .commands
            .iter()
            .find(|command| command.id == id && !command.disabled)
            .cloned()
        else {
            return;
        };
        let Target::List(provider) = command.target else {
            return;
        };
        self.page = Page {
            id: Some(command.id),
            registry: provider(cx),
            title: command.title,
            query: String::new(),
            selected: None,
        };
        self.parents.clear();
        self.show_page(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let items = self.page.registry.items(&self.page.query);
        let status = if items.is_empty() {
            PickerStatus::Empty("No matching commands".into())
        } else {
            PickerStatus::Ready
        };
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }

    fn confirm(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(command) = self
            .page
            .registry
            .commands
            .iter()
            .find(|command| {
                command.id == id
                    && !command.disabled
                    && command.matches(&self.page.query.to_lowercase())
            })
            .cloned()
        else {
            return;
        };
        match command.target {
            Target::Action(action) if command.keep_open => {
                self.page.selected = Some(command.id);
                cx.emit(CommandPaletteEvent::Applied(action));
            }
            Target::Action(action) => cx.emit(CommandPaletteEvent::Selected(action)),
            Target::List(provider) => {
                self.page.selected = Some(command.id.clone());
                let page = Page {
                    id: Some(command.id),
                    registry: provider(cx),
                    title: command.title,
                    query: String::new(),
                    selected: None,
                };
                self.parents.push(std::mem::replace(&mut self.page, page));
                self.show_page(cx);
            }
        }
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.parents.pop() {
            self.page = parent;
            self.show_page(cx);
        }
    }

    fn show_page(&mut self, cx: &mut Context<Self>) {
        self.picker.update(cx, |picker, cx| {
            picker.set_query(self.page.query.clone(), cx);
            picker.set_placeholder(self.page.title.clone(), cx);
            picker.set_can_navigate_back(!self.parents.is_empty(), cx);
        });
        self.refresh(cx);
        if let Some(selected) = &self.page.selected {
            self.picker.update(cx, |picker, cx| {
                let _ = picker.select_row(selected, cx);
            });
        }
    }
}

impl<A: Clone + 'static> Render for CommandPalette<A> {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}

#[cfg(test)]
mod tests;
