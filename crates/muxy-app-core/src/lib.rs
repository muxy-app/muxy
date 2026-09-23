//! Headless client state, configuration, and persistence.
//!
//! Workspaces, cached project views, tabs, panes, and window state live here;
//! server execution and UI toolkit code do not.

pub mod composer;
mod error;
pub mod extensions;
mod home;
mod ids;
mod layout;
pub mod modal;
pub mod opener;
mod pane;
mod project;
pub mod restore;
mod state;
pub mod store;
mod tab;
pub mod title;
pub mod webview;
mod window;
mod workspace;

pub use error::AppError;
pub use ids::{PaneId, ProjectId, ServerId, TabId, WorkspaceId};
pub use layout::{Axis, Branch, Direction, Layout};
pub use pane::{Pane, PaneContent};
pub use project::{Color, PROJECT_COLORS, Project, ProjectKind, ProjectStatus};
pub use state::AppState;
pub use tab::{Tab, TabCloseScope, TabSide};
pub use window::{WindowBounds, WindowState};
pub use workspace::Workspace;

mod catalog;

pub mod settings;

pub mod activity;
