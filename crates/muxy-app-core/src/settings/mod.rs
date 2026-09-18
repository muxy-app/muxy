//! Client configuration and keymap schema and storage.
//!
//! Server-owned settings and presentation code stay with their owners.

mod appearance;
mod chord;
mod composer;
mod config;
mod error;
mod ghostty;
mod keymap;
mod quick_terminal;

pub use appearance::{AppLayout, Appearance, ProjectOrder, SidebarCollapsedStyle};
pub use chord::KeyChord;
pub use composer::{ComposerPosition, ComposerPresentation, ComposerSettings};
pub use config::{
    ClipboardSettings, CloseBehavior, NewPaneDirectory, OpenerSettings, PaneSettings,
    ProjectSettings, Settings, WindowSettings,
};
pub use error::{Error, Result};
pub use ghostty::{
    CellHeight, FontMap, FontOptions, OptionAsAlt, PaddingColor, TerminalAction, TerminalBindings,
    TerminalColor, TerminalOptions, TerminalSettings,
};
pub use keymap::Keymap;
pub use quick_terminal::QuickTerminalSettings;
