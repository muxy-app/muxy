//! Server-owned projects, sessions, capability execution, settings, and client runtime.
//!
//! Project metadata and explicit session membership are shared; client-owned
//! workspaces, tabs, panes, and presentation never enter this crate.

mod archive;
pub mod connection;
mod error;
mod registry;
mod search;
mod session;
mod settings;
mod spawn;

pub use error::ServerError;
pub use registry::{Registry, ServerEvent};
pub use session::{AttachmentEvent, AttachmentId, SessionCommand, SessionHandle};
pub use settings::ServerSettings;

mod shell;
pub use shell::ShellIntegration;

mod catalog;
pub use catalog::LegacyImport;

mod git;

mod activity;
mod detection;
