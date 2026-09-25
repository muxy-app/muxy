mod auth;
mod handshake;
mod merge;
mod outbox;
mod policy;
mod reader;
#[cfg(test)]
mod tests;
mod writer;

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder, MAX_FRAME, WireError};
use muxy_protocol::{CONTROL, Message, Version};

use crate::{Admission, Registry, ServerEvent};
pub(crate) use outbox::{Outbox, References};

const POLL: Duration = Duration::from_millis(50);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// Hello and the authentication request are small; nothing larger is read before trust.
const UNAUTHENTICATED_PAYLOAD: usize = 4 * 1024;

pub fn serve(
    stream: Box<dyn ByteStream>,
    registry: Arc<Registry>,
    events: Receiver<ServerEvent>,
) -> Result<(), WireError> {
    let cancellation: Arc<dyn StreamCancellation> = Arc::from(stream.cancellation()?);
    let (read, write) = stream.split()?;
    let mut decoder = Decoder::new(read);
    let mut encoder = Encoder::new(write);
    let Some(version) = handshake::accept(&mut decoder, &mut encoder)? else {
        return Ok(());
    };
    let outbox = Arc::new(Outbox::new(
        version,
        Arc::clone(&registry.attachment_changes),
    ));
    registry.register_connection(&outbox);
    run(
        decoder,
        encoder,
        &cancellation,
        registry,
        events,
        &outbox,
        version,
    )
}

/// Serves a network peer, which must authenticate as a paired device before
/// the connection gets any resources. One deadline covers TLS, Hello, and auth.
pub fn serve_remote(
    stream: Box<dyn ByteStream>,
    registry: Arc<Registry>,
    events: Receiver<ServerEvent>,
    admission: Admission,
) -> Result<(), WireError> {
    let cancellation: Arc<dyn StreamCancellation> = Arc::from(stream.cancellation()?);
    let deadline = watchdog(Arc::clone(&cancellation), admission.timeout)?;
    let (read, write) = stream.split()?;
    let mut decoder = Decoder::new(read);
    decoder.set_payload_limit(UNAUTHENTICATED_PAYLOAD);
    let mut encoder = Encoder::new(write);
    let Some(version) = handshake::accept_remote(&mut decoder, &mut encoder)? else {
        return Ok(());
    };
    let Some(admitted) = auth::accept(&mut decoder, &mut encoder, &registry)? else {
        return Ok(());
    };
    let outbox = Arc::new(Outbox::for_device(
        version,
        Arc::clone(&registry.attachment_changes),
        admitted.device,
    ));
    registry.register_connection(&outbox);
    // Registering before this check means a concurrent revoke either sees this
    // connection and closes it, or this check sees the revoke.
    if !registry.remote.authorized(admitted.device) {
        outbox.close();
        encoder.send(CONTROL, &auth::unauthorized(admitted.request))?;
        return Ok(());
    }
    outbox.push_control(Message::Reply {
        id: admitted.request,
        body: admitted.reply,
    });
    drop(deadline);
    drop(admission);
    decoder.set_payload_limit(MAX_FRAME);
    log::info!("device connected: {}", admitted.device);
    registry.remote.changed();
    let result = run(
        decoder,
        encoder,
        &cancellation,
        registry,
        events,
        &outbox,
        version,
    );
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
    events: Receiver<ServerEvent>,
    outbox: &Arc<Outbox>,
    version: Version,
) -> Result<(), WireError> {
    let output = Arc::clone(outbox);
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
    let output = Arc::clone(outbox);
    let catalog = Arc::clone(&registry);
    let forward = match thread::Builder::new()
        .name("connection-events".into())
        .spawn(move || forward_events(&output, &catalog, &events))
    {
        Ok(forward) => forward,
        Err(error) => {
            outbox.close();
            cancellation.cancel();
            let _ = writer.join();
            return Err(error.into());
        }
    };
    let result = reader::run(&mut decoder, &registry, outbox, version);
    outbox.close();
    registry.connection_closed(outbox);
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
    forward
        .join()
        .map_err(|_| io::Error::other("connection event forwarder panicked"))?;
    written.and(result.map(|_| ()))
}

fn forward_events(output: &Outbox, catalog: &Registry, events: &Receiver<ServerEvent>) {
    let mut revision = 0;
    let mut sessions_revision = 0;
    let mut remote_revision = 0;
    let mut activity_revision = None;
    while !output.is_closed() {
        let current = catalog.activity.revision();
        if activity_revision != Some(current) && output.activity_watched() {
            output.push_control(Message::ActivityChanged { revision: current });
            activity_revision = Some(current);
        }
        for (session, (progress, metadata)) in catalog.session_observations(output) {
            output.push_control(Message::SessionMetadata { session, metadata });
            output.push_control(Message::Progress { session, progress });
        }
        let current = catalog.catalog_revision();
        if current > revision && output.catalog_watched() {
            output.push_control(Message::CatalogChanged { revision: current });
            revision = current;
        }
        let current = catalog.sessions_revision();
        if current > sessions_revision && output.catalog_watched() {
            output.push_control(Message::SessionsChanged { revision: current });
            sessions_revision = current;
        }
        let current = catalog.remote.revision();
        if current > remote_revision && output.remote_access_watched() {
            output.push_control(Message::RemoteAccessChanged { revision: current });
            remote_revision = current;
        }
        match events.recv_timeout(POLL) {
            Ok(ServerEvent::SessionEnded { id, reason }) => {
                output.session_ended(id, reason);
            }
            Ok(ServerEvent::RestartRequested) => {
                output.push_control(Message::ServerRestarting);
            }
            Ok(ServerEvent::StopRequested) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                output.close();
                break;
            }
        }
    }
}
