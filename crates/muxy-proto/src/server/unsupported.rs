use super::{IncomingRequest, ServerError, SocketServer, SocketServerHandle};

pub(super) fn unsupported() -> Result<
    (
        SocketServer,
        SocketServerHandle,
        async_channel::Receiver<IncomingRequest>,
    ),
    ServerError,
> {
    Err(ServerError::UnsupportedPlatform)
}
