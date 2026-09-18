mod handshake;
mod merge;
mod outbox;
mod reader;
mod writer;

use std::io;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder, WireError};

use crate::{Registry, ServerEvent};
pub(crate) use outbox::{Outbox, References};

const POLL: Duration = Duration::from_millis(50);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

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
    let output = Arc::clone(&outbox);
    let cancel = Arc::clone(&cancellation);
    let (done, finished) = mpsc::channel();
    let writer = thread::Builder::new()
        .name("connection-writer".into())
        .spawn(move || {
            let result = writer::run(encoder, &output, cancel.as_ref());
            let _ = done.send(());
            cancel.cancel();
            result
        })?;
    let output = Arc::clone(&outbox);
    let catalog = Arc::clone(&registry);
    let forward = match thread::Builder::new()
        .name("connection-events".into())
        .spawn(move || {
            let mut revision = 0;
            let mut sessions_revision = 0;
            let mut activity_revision = None;
            while !output.is_closed() {
                let current = catalog.activity.revision();
                if activity_revision != Some(current) && output.activity_watched() {
                    output.push_control(muxy_protocol::Message::ActivityChanged {
                        revision: current,
                    });
                    activity_revision = Some(current);
                }
                for (session, (progress, metadata)) in catalog.session_observations(&output) {
                    output.push_control(muxy_protocol::Message::SessionMetadata {
                        session,
                        metadata,
                    });
                    output.push_control(muxy_protocol::Message::Progress { session, progress });
                }
                let current = catalog.catalog_revision();
                if current > revision && output.catalog_watched() {
                    output
                        .push_control(muxy_protocol::Message::CatalogChanged { revision: current });
                    revision = current;
                }
                let current = catalog.sessions_revision();
                if current > sessions_revision && output.catalog_watched() {
                    output.push_control(muxy_protocol::Message::SessionsChanged {
                        revision: current,
                    });
                    sessions_revision = current;
                }
                match events.recv_timeout(POLL) {
                    Ok(ServerEvent::SessionEnded { id, reason }) => {
                        output.session_ended(id, reason);
                    }
                    Ok(ServerEvent::RestartRequested) => {
                        output.push_control(muxy_protocol::Message::ServerRestarting);
                    }
                    Ok(ServerEvent::StopRequested) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        output.close();
                        break;
                    }
                }
            }
        }) {
        Ok(forward) => forward,
        Err(error) => {
            outbox.close();
            cancellation.cancel();
            let _ = writer.join();
            return Err(error.into());
        }
    };
    let result = reader::run(&mut decoder, &registry, &outbox, version);
    drop(registry);
    outbox.close();
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
