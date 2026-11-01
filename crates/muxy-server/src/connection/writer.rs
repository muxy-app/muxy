use std::io::Write;

use muxy_protocol::transport::StreamCancellation;
use muxy_protocol::wire::{Encoder, WireError};

use super::Connection;

pub(super) fn run(
    mut encoder: Encoder<impl Write>,
    connection: &Connection,
    cancellation: &dyn StreamCancellation,
) -> Result<(), WireError> {
    while let Some((channel, message)) = connection.next() {
        if let Err(error) = encoder.send(channel, &message) {
            let closed = connection.is_closed();
            connection.close();
            cancellation.cancel();
            return if closed { Ok(()) } else { Err(error) };
        }
    }
    Ok(())
}
