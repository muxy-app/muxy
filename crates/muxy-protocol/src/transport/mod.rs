//! Protocol-free ordered byte streams and their platform adapters: local
//! sockets, TLS, and a child process's stdin and stdout, plus a relay that
//! carries a stream over a process's own stdin and stdout.
//!
//! Framing, messages, sessions, and application policy are deliberately
//! outside this crate.

mod error;
mod stdio;
mod stream;
pub mod tls;
mod unix;

pub use error::BindError;
pub use stdio::{ChildProcess, ChildStream, relay};
pub use stream::{ByteStream, Listener, StreamCancellation};
#[cfg(windows)]
pub use unix::peer_pid;
pub use unix::{
    LocalListener, LocalStream, SocketDirectoryLock, UnixSocketListener, connect, connect_local,
    lock_socket_directory, socket_pair,
};
