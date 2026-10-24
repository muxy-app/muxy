//! Client configuration and keymap schema and storage.
//!
//! Server-owned settings and presentation code stay with their owners.

mod appearance;
mod browser;
mod chord;
mod commands;
mod composer;
mod config;
mod error;
mod ghostty;
mod keymap;
mod proxy;
mod quick_terminal;
mod servers;
mod worktrees;

pub use worktrees::{
    DEFAULT_WORKTREE_FOLDER, SUGGESTED_WORKTREE_TEMPLATE, WorktreeLocation, WorktreeSettings,
    sanitized_component,
};

pub use appearance::{
    AgentSessionsPosition, AgentsPanelDock, AgentsPanelGrouping, AgentsPanelIcon, AppLayout,
    Appearance, ProjectOrder, SidebarCollapsedStyle,
};
pub use browser::BrowserSettings;
pub use chord::KeyChord;
pub use commands::CustomCommand;
pub use composer::{ComposerPosition, ComposerPresentation, ComposerSettings};
pub use config::{
    ClipboardSettings, CloseBehavior, CommitChoices, NewPaneDirectory, OpenerSettings,
    PaneSettings, ProjectSettings, Settings, WindowSettings,
};
pub use error::{Error, Result};
pub use ghostty::{
    CellHeight, FontMap, FontOptions, OptionAsAlt, PaddingColor, TerminalAction, TerminalBindings,
    TerminalColor, TerminalEdit, TerminalOptions, TerminalSettings,
};
pub use keymap::Keymap;
pub use proxy::{DEFAULT_PROXY_BYPASS, ProxyKind, ProxySettings};
pub use quick_terminal::QuickTerminalSettings;
pub use servers::ServerEntry;
