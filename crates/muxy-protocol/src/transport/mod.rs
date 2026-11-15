mod accepting;
mod error;
mod stdio;
mod stream;
pub mod tls;
mod unix;

pub use error::BindError;
pub use stdio::{ChildProcess, ChildStream, relay};
pub use stream::{ByteStream, Listener, StreamCancellation};
#[cfg(unix)]
pub use unix::peer_is_same_user;
#[cfg(windows)]
pub use unix::peer_pid;
pub use unix::{
    LocalListener, LocalStream, SocketDirectoryLock, UnixSocketListener, connect, connect_local,
    lock_socket_directory, socket_pair,
};
