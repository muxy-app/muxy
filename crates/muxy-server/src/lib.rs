mod archive;
pub mod connection;
mod error;
mod exec;
mod locks;
mod platform;
mod process;
mod references;
mod registry;
mod search;
mod session;
mod settings;
mod spawn;

pub use error::ServerError;
pub use registry::{Registry, ServerEvent};
pub use session::{AttachmentEvent, AttachmentId, AttachmentSink, SessionCommand, SessionHandle};
pub use settings::ServerSettings;

mod shell;
pub use shell::ShellIntegration;

mod catalog;
pub use catalog::LegacyImport;

mod files;
mod git;
mod notifier;
mod unreadable;
mod uploads;
mod watch;

mod activity;
mod detection;

mod push;
#[cfg(test)]
mod test_support;

mod remote;
pub use remote::{Admission, RemoteAccess};
