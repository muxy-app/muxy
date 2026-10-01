use std::collections::BTreeMap;

use muxy_protocol::{FileEntry, ServerPath};
use serde_json::Value;

use crate::{AppError, AppState, Axis, Layout, Pane, PaneContent, PaneId, ProjectId, Tab};

pub const DIRECTORY: &str = ".muxy/layouts";
const MAX_BYTES: usize = 256 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_PANES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub name: String,
    pub path: ServerPath,
}

pub fn discover(entries: Vec<FileEntry>) -> Vec<Descriptor> {
    let mut layouts: Vec<_> = entries
        .into_iter()
        .filter_map(|entry| {
            let name = std::str::from_utf8(&entry.name.0).ok()?;
            if entry.is_directory || name.starts_with('.') || name.contains('/') {
                return None;
            }
            let (name, extension) = name.rsplit_once('.')?;
            if name.is_empty()
                || !["yaml", "yml", "json"].contains(&extension.to_ascii_lowercase().as_str())
            {
                return None;
            }
            Some(Descriptor {
                name: name.to_owned(),
                path: entry.path,
            })
        })
        .collect();
    layouts.sort_by_cached_key(|layout| (layout.name.to_lowercase(), layout.path.0.clone()));
    layouts
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    root: Node,
    extra_tabs: Vec<Terminal>,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Leaf(Terminal),
    Split(Axis, Vec<Self>),
}

#[derive(Clone, Debug, PartialEq)]
struct Terminal {
    name: Option<String>,
    command: Option<String>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("Layout exceeds the 256 KiB limit".into());
        }
        let options = serde_saphyr::options! {
            budget: serde_saphyr::budget! {
                max_depth: MAX_DEPTH * 2 + 8,
                max_events: 16_384,
                max_total_scalar_bytes: MAX_BYTES,
            },
            alias_limits: serde_saphyr::alias_limits! {
                max_total_replayed_events: 16_384,
            },
        };
        let value: Value = serde_saphyr::from_str_with_options(text, options)
            .map_err(|error| error.to_string())?;
        let mut extra_tabs = Vec::new();
        let mut count = 0;
        let root = Node::parse(&value, 0, &mut count, &mut extra_tabs)?;
        Ok(Self { root, extra_tabs })
    }

    fn build(&self) -> (Vec<Tab>, BTreeMap<PaneId, String>) {
        let mut panes = Vec::new();
        let mut commands = BTreeMap::new();
        let layout = self.root.build(&mut panes, &mut commands);
        let mut tab = Tab::terminal();
        tab.custom_title = panes.first().map(|pane| pane.title.clone());
        tab.panes = panes;
        tab.layout = layout;
        let mut tabs = vec![tab];
        for terminal in &self.extra_tabs {
            let pane = terminal.build(&mut commands);
            let mut tab = Tab::terminal();
            tab.custom_title = Some(pane.title.clone());
            tab.layout = Layout::Leaf(pane.id);
            tab.panes = vec![pane];
            tabs.push(tab);
        }
        (tabs, commands)
    }
}

impl Node {
    fn parse(
        value: &Value,
        depth: usize,
        count: &mut usize,
        extra_tabs: &mut Vec<Terminal>,
    ) -> Result<Self, String> {
        if depth > MAX_DEPTH {
            return Err("Layout nesting exceeds 32 levels".into());
        }
        let object = value.as_object().ok_or("Each pane must be an object")?;
        if let Some(panes) = object.get("panes") {
            let panes = panes
                .as_array()
                .filter(|panes| !panes.is_empty())
                .ok_or("panes must be a non-empty list")?;
            let axis = if object
                .get("layout")
                .and_then(Value::as_str)
                .is_some_and(|layout| layout.eq_ignore_ascii_case("vertical"))
            {
                Axis::Vertical
            } else {
                Axis::Horizontal
            };
            let children = panes
                .iter()
                .map(|pane| Self::parse(pane, depth + 1, count, extra_tabs))
                .collect::<Result<_, _>>()?;
            return Ok(Self::Split(axis, children));
        }
        if let Some(tab) = object.get("tab") {
            return Terminal::parse(tab, count).map(Self::Leaf);
        }
        let tabs = object
            .get("tabs")
            .and_then(Value::as_array)
            .filter(|tabs| !tabs.is_empty())
            .ok_or("Each leaf must contain tab or a non-empty tabs list")?;
        let first = Terminal::parse(&tabs[0], count)?;
        for tab in &tabs[1..] {
            extra_tabs.push(Terminal::parse(tab, count)?);
        }
        Ok(Self::Leaf(first))
    }

    fn build(&self, panes: &mut Vec<Pane>, commands: &mut BTreeMap<PaneId, String>) -> Layout {
        match self {
            Self::Leaf(terminal) => {
                let pane = terminal.build(commands);
                let layout = Layout::Leaf(pane.id);
                panes.push(pane);
                layout
            }
            Self::Split(axis, children) => build_children(children, *axis, panes, commands),
        }
    }
}

fn build_children(
    children: &[Node],
    axis: Axis,
    panes: &mut Vec<Pane>,
    commands: &mut BTreeMap<PaneId, String>,
) -> Layout {
    if children.len() == 1 {
        return children[0].build(panes, commands);
    }
    let middle = children.len() / 2;
    #[allow(
        clippy::cast_precision_loss,
        reason = "Layouts contain at most 64 panes"
    )]
    let ratio = middle as f32 / children.len() as f32;
    Layout::Split {
        axis,
        ratio,
        first: Box::new(build_children(&children[..middle], axis, panes, commands)),
        second: Box::new(build_children(&children[middle..], axis, panes, commands)),
    }
}

impl Terminal {
    fn parse(value: &Value, count: &mut usize) -> Result<Self, String> {
        *count += 1;
        if *count > MAX_PANES {
            return Err("Layout exceeds 64 terminals".into());
        }
        if let Some(command) = value.as_str() {
            return Ok(Self {
                name: None,
                command: Some(trimmed(command).ok_or("A bare tab command cannot be empty")?),
            });
        }
        let object = value
            .as_object()
            .ok_or("tab must be an object or a command")?;
        let name = object.get("name").and_then(Value::as_str).and_then(trimmed);
        let command = match object.get("command") {
            None | Some(Value::Null) => None,
            Some(Value::String(command)) => trimmed(command),
            Some(Value::Array(commands)) => {
                let commands = commands
                    .iter()
                    .map(|command| command.as_str().ok_or("Commands must be strings"))
                    .collect::<Result<Vec<_>, _>>()?;
                trimmed(
                    &commands
                        .into_iter()
                        .filter_map(trimmed)
                        .collect::<Vec<_>>()
                        .join(" && "),
                )
            }
            Some(_) => return Err("command must be a string or list of strings".into()),
        };
        Ok(Self { name, command })
    }

    fn build(&self, commands: &mut BTreeMap<PaneId, String>) -> Pane {
        let id = PaneId::new();
        if let Some(command) = &self.command {
            commands.insert(id, command.clone());
        }
        let title = self
            .name
            .as_deref()
            .or_else(|| {
                self.command
                    .as_deref()
                    .and_then(|command| command.split_whitespace().next())
            })
            .unwrap_or("Terminal");
        Pane {
            id,
            title: title.into(),
            content: PaneContent::Terminal { session: None },
        }
    }
}

fn trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

impl AppState {
    pub fn apply_project_layout(
        &mut self,
        project: ProjectId,
        config: &Config,
    ) -> Result<Vec<PaneId>, AppError> {
        let target = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?;
        target.require_available()?;
        let (tabs, commands) = config.build();
        let selected = tabs[0].id;
        let panes: Vec<_> = tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .collect();
        let old_tabs: Vec<_> = target.tabs.iter().map(|tab| tab.id).collect();
        let closing: Vec<_> = target
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .filter_map(|pane| match pane.content {
                PaneContent::Terminal { session } => session,
                _ => None,
            })
            .collect();
        for tab in old_tabs {
            self.close_tab(project, tab)?;
        }
        self.project_mut(project)?.tabs = tabs;
        self.startup_commands.extend(commands);
        self.window.current_project = project;
        self.window.selected_tab.insert(project, selected);
        self.window.activate(panes.first().copied());
        let references = self.session_references();
        for session in closing {
            if !references.contains(&session) {
                self.queue_discard(session);
            }
        }
        Ok(panes)
    }

    pub fn startup_command(&self, pane: PaneId) -> Option<&str> {
        self.startup_commands.get(&pane).map(String::as_str)
    }

    pub fn set_startup_command(&mut self, pane: PaneId, command: &str) -> Result<(), AppError> {
        if !matches!(self.pane_mut(pane)?.content, PaneContent::Terminal { .. }) {
            return Err(AppError::NotTerminal(pane));
        }
        if let Some(command) = trimmed(command) {
            self.startup_commands.insert(pane, command);
        }
        Ok(())
    }

    pub fn take_startup_command(&mut self, pane: PaneId) -> Option<String> {
        self.startup_commands.remove(&pane)
    }
}
