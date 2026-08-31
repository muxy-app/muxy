mod attach;
mod daemon;
mod process_tree;
mod pty;
mod runtime_paths;
mod shell;
mod transport;

pub use client::{RendererClient, RendererEvent, SessionClient, sibling_helper};
pub use process_tree::{identity_is_alive, process_identity};
pub use runtime_paths::selected_socket_path;

mod client;

use muxy_proto::session::{BuildMode, SessionId};
use std::io;
use std::path::Path;

pub fn current_build_mode() -> BuildMode {
    if muxy_core::build_mode!().is_development() {
        BuildMode::Development
    } else {
        BuildMode::Production
    }
}

pub fn run_daemon(socket_path: impl AsRef<Path>) -> io::Result<()> {
    daemon::run(daemon::DaemonConfig::new(
        socket_path.as_ref(),
        current_build_mode(),
    ))
}

pub fn run_attach(socket_path: &Path, session_id: SessionId) -> io::Result<()> {
    attach::run(socket_path, session_id, current_build_mode())
}
