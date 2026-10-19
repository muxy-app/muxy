use std::error::Error;
use std::thread;
use std::time::Duration;

use muxy_client::{Client, ClientEvent};
use muxy_protocol::transport::LocalStream;
use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{
    CONTROL, ChannelId, MAX_CLIPBOARD_TEXT, Message, MetadataEvent, ReplyBody, RequestBody,
    SUPPORTED, ServerInfo,
};

type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

#[test]
fn clipboard_floods_coalesce_until_consumed_without_blocking_control_replies() -> TestResult {
    let (client, server) = muxy_protocol::transport::socket_pair()?;
    server.set_read_timeout(Some(Duration::from_secs(10)))?;
    server.set_write_timeout(Some(Duration::from_secs(10)))?;
    let peer = thread::spawn(move || send_clipboard_batches(server));
    let client = Client::from_stream(Box::new(client))?;
    let events = client.events().ok_or("missing events")?;

    client.ping()?;
    let ClientEvent::ClipboardWrite {
        channel: ChannelId(1),
        write,
    } = events.try_recv()?
    else {
        return Err("expected first clipboard notification".into());
    };
    let ClientEvent::ClipboardWrite {
        channel: ChannelId(2),
        write: other,
    } = events.try_recv()?
    else {
        return Err("expected independent channel notification".into());
    };
    assert!(events.try_recv().is_err());

    client.ping()?;
    assert!(events.try_recv().is_err());
    let text = write.take().ok_or("missing latest clipboard text")?;
    assert_eq!(text.len(), MAX_CLIPBOARD_TEXT);
    assert!(text.starts_with("1:127:"));
    assert_eq!(other.take().as_deref(), Some("1:other"));

    client.ping()?;
    for channel in [ChannelId(1), ChannelId(2)] {
        let ClientEvent::ClipboardWrite {
            channel: received,
            write,
        } = events.try_recv()?
        else {
            return Err("expected clipboard notification after consumption".into());
        };
        assert_eq!(received, channel);
        let text = write.take().ok_or("missing next clipboard text")?;
        assert!(text.starts_with("2:"));
    }
    client.disconnect();
    peer.join().map_err(|_| "clipboard peer panicked")??;
    Ok(())
}

fn send_clipboard_batches(server: LocalStream) -> TestResult {
    let mut decoder = Decoder::new(server.try_clone()?);
    let mut encoder = Encoder::new(server);
    assert!(matches!(decoder.next()?, (CONTROL, Message::Hello { .. })));
    encoder.send(
        CONTROL,
        &Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: ServerInfo::current(),
            features: Vec::new(),
        },
    )?;
    for batch in 0..3 {
        let (
            CONTROL,
            Message::Request {
                id,
                body: RequestBody::Ping,
            },
        ) = decoder.next()?
        else {
            return Err("expected ping during clipboard flood".into());
        };
        for index in 0..128 {
            let mut text = format!("{batch}:{index}:");
            text.extend(std::iter::repeat_n('x', MAX_CLIPBOARD_TEXT - text.len()));
            encoder.send(
                ChannelId(1),
                &Message::Metadata(MetadataEvent::ClipboardWrite(text)),
            )?;
        }
        encoder.send(
            ChannelId(2),
            &Message::Metadata(MetadataEvent::ClipboardWrite(format!("{batch}:other"))),
        )?;
        encoder.send(
            CONTROL,
            &Message::Reply {
                id,
                body: ReplyBody::Pong,
            },
        )?;
    }
    let _ = decoder.next();
    Ok(())
}
