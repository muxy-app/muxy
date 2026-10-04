//! Protocol-free ordered byte streams and their platform adapters: Unix
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
pub use unix::{UnixSocketListener, connect, socket_pair};
