mod auth;
mod handshake;
mod merge;
mod outbox;
mod peer;
mod policy;
mod reader;
mod sink;
mod state;
#[cfg(test)]
mod tests;
mod writer;

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder, MAX_FRAME, WireError};
use muxy_protocol::{CONTROL, Message};

use crate::{Admission, Registry};
pub(crate) use peer::Connection;
pub(crate) use sink::OutboxSink;

const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// Hello and the authentication request are small; nothing larger is read before trust.
const UNAUTHENTICATED_PAYLOAD: usize = 4 * 1024;

pub fn serve(stream: Box<dyn ByteStream>, registry: Arc<Registry>) -> Result<(), WireError> {
    let cancellation: Arc<dyn StreamCancellation> = Arc::from(stream.cancellation()?);
    let (read, write) = stream.split()?;
    let mut decoder = Decoder::new(read);
    let mut encoder = Encoder::new(write);
    if !handshake::accept(&mut decoder, &mut encoder)? {
        return Ok(());
    }
    let connection = Arc::new(Connection::new(Arc::clone(&registry.notifier)));
    registry.notifier.register(&connection);
    run(decoder, encoder, &cancellation, registry, &connection)
}

/// Serves a network peer, which must authenticate as a paired device before
/// the connection gets any resources. One deadline covers TLS, Hello, and auth.
pub fn serve_remote(
    stream: Box<dyn ByteStream>,
    registry: Arc<Registry>,
    admission: Admission,
) -> Result<(), WireError> {
    let cancellation: Arc<dyn StreamCancellation> = Arc::from(stream.cancellation()?);
    let deadline = watchdog(Arc::clone(&cancellation), admission.timeout)?;
    let (read, write) = stream.split()?;
    let mut decoder = Decoder::new(read);
    decoder.set_payload_limit(UNAUTHENTICATED_PAYLOAD);
    let mut encoder = Encoder::new(write);
    if !handshake::accept_remote(&mut decoder, &mut encoder)? {
        return Ok(());
    }
    let Some(admitted) = auth::accept(&mut decoder, &mut encoder, &registry)? else {
        return Ok(());
    };
    let connection = Arc::new(Connection::for_device(
        Arc::clone(&registry.notifier),
        admitted.device,
        admitted.kind,
    ));
    registry.notifier.register(&connection);
    // Registering before this check means a concurrent revoke either sees this
    // connection and closes it, or this check sees the revoke.
    if !registry.remote.authorized(admitted.device) {
        connection.close();
        encoder.send(CONTROL, &auth::unauthorized(admitted.request))?;
        return Ok(());
    }
    connection.push_control(Message::Reply {
        id: admitted.request,
        body: admitted.reply,
    });
    drop(deadline);
    drop(admission);
    decoder.set_payload_limit(MAX_FRAME);
    log::info!(
        "device connected: {} ({:?})",
        admitted.device,
        admitted.kind
    );
    registry.remote.changed();
    let result = run(decoder, encoder, &cancellation, registry, &connection);
    log::info!("device disconnected: {}", admitted.device);
    result
}

/// Cancels the stream unless the returned sender is dropped before the timeout.
fn watchdog(
    cancellation: Arc<dyn StreamCancellation>,
    timeout: Duration,
) -> io::Result<mpsc::Sender<()>> {
    let (disarm, disarmed) = mpsc::channel::<()>();
    thread::Builder::new()
        .name("connection-deadline".into())
        .spawn(move || {
            if let Err(RecvTimeoutError::Timeout) = disarmed.recv_timeout(timeout) {
                cancellation.cancel();
            }
        })?;
    Ok(disarm)
}

fn run(
    mut decoder: Decoder<impl Read>,
    encoder: Encoder<impl Write + Send + 'static>,
    cancellation: &Arc<dyn StreamCancellation>,
    registry: Arc<Registry>,
    connection: &Arc<Connection>,
) -> Result<(), WireError> {
    let output = Arc::clone(connection);
    let cancel = Arc::clone(cancellation);
    let (done, finished) = mpsc::channel();
    let writer = thread::Builder::new()
        .name("connection-writer".into())
        .spawn(move || {
            let result = writer::run(encoder, &output, cancel.as_ref());
            let _ = done.send(());
            cancel.cancel();
            result
        })?;
    let result = reader::run(&mut decoder, &registry, connection);
    connection.close();
    registry.connection_closed(connection);
    drop(registry);
    if !matches!(result, Ok(reader::Exit::Fatal))
        || matches!(
            finished.recv_timeout(CLOSE_TIMEOUT),
            Err(RecvTimeoutError::Timeout)
        )
    {
        cancellation.cancel();
    }
    let written = writer
        .join()
        .map_err(|_| io::Error::other("connection writer panicked"))?;
    written.and(result.map(|_| ()))
}
