use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{
    CONTROL, DeviceId, ErrorCode, ErrorReply, Message, ReplyBody, RequestBody, RequestId,
};

use crate::Registry;

use super::handshake::rejection;

pub(super) struct Admitted {
    pub(super) device: DeviceId,
    pub(super) request: RequestId,
    pub(super) reply: ReplyBody,
}

/// Reads the one request a network peer may send before it is trusted.
pub(super) fn accept(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
    registry: &Registry,
) -> Result<Option<Admitted>, WireError> {
    let (request, body) = match decoder.next() {
        Ok((CONTROL, Message::Request { id, body })) => (id, body),
        Ok(_) => {
            report("expected authentication");
            encoder.send(CONTROL, &rejection("authentication required"))?;
            return Ok(None);
        }
        Err(WireError::Closed) => return Ok(None),
        Err(error @ WireError::Io(_)) => return Err(error),
        Err(error) => {
            report(&error.to_string());
            encoder.send(CONTROL, &rejection(error.to_string()))?;
            return Ok(None);
        }
    };
    let admitted = match &body {
        RequestBody::Authenticate(credential) => registry
            .remote
            .authenticate(credential)
            .map(|device| (device, ReplyBody::Authenticated)),
        RequestBody::Pair(pairing) if pairing.validate().is_ok() => registry
            .pair_device(pairing)
            .map(|paired| (paired.credential.device, ReplyBody::Paired(paired))),
        _ => None,
    };
    if let Some((device, reply)) = admitted {
        return Ok(Some(Admitted {
            device,
            request,
            reply,
        }));
    }
    report("unknown or revoked device");
    encoder.send(CONTROL, &unauthorized(request))?;
    Ok(None)
}

/// A correlated reply, so the phone can tell "pair again" from a dropped network.
pub(super) fn unauthorized(request: RequestId) -> Message {
    Message::Reply {
        id: request,
        body: ReplyBody::Error(ErrorReply {
            code: ErrorCode::Unauthorized,
            message: "this device is not paired with the server".into(),
        }),
    }
}

/// Logs unauthenticated failures at most once per interval, so a noisy network
/// cannot grow the server log without bound.
pub(super) fn report(reason: &str) {
    const INTERVAL: u64 = 60;
    static LAST: AtomicU64 = AtomicU64::new(0);
    static SUPPRESSED: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = LAST.load(Ordering::Acquire);
    if now >= last.saturating_add(INTERVAL)
        && LAST
            .compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let suppressed = SUPPRESSED.swap(0, Ordering::AcqRel);
        log::warn!("rejected network connection: {reason} ({suppressed} more since last report)");
    } else {
        SUPPRESSED.fetch_add(1, Ordering::AcqRel);
    }
}
