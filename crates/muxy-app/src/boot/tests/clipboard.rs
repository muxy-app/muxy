use super::*;
use muxy_protocol::{MAX_CLIPBOARD_TEXT, MetadataEvent};

#[test]
fn clipboard_notifications_stay_coalesced_through_worker_and_ui_queues() -> TestResult {
    let directory = tempfile::Builder::new()
        .prefix("muxy-clipboard-")
        .tempdir_in(crate::app_socket::test_socket_folder())?;
    let socket = directory.path().join("server.sock");
    let listener = LocalListener::bind(&socket)?;
    let server = thread::spawn(move || clipboard_server(&listener));
    let (work, updates) = local_worker(socket)?;
    work.send((1, Work::Connect))?;
    assert!(matches!(updates.recv_blocking()?.2, Update::ServerInfo(_)));
    assert!(matches!(updates.recv_blocking()?.2, Update::Connected(_)));
    let (reply, client) = async_channel::bounded(1);
    work.send((1, Work::ExtensionClient(reply)))?;
    let client = client
        .recv_blocking()?
        .ok_or("missing client")?
        .with_timeout(Duration::from_secs(30));

    client.ping()?;
    let Update::Event(event @ ClientEvent::ClipboardWrite { .. }) = updates.recv_blocking()?.2
    else {
        return Err("expected clipboard notification".into());
    };
    expect_clipboard_barrier(&updates)?;
    let mut delivery = delivery::Delivery::default();
    delivery.pending = 1;
    assert!(delivery.event(event)?.is_empty());

    client.ping()?;
    expect_clipboard_barrier(&updates)?;
    let mut ready = delivery.complete(Some(attachment_update(ChannelId(1))?));
    assert_eq!(ready.len(), 2);
    let Some(Update::Event(ClientEvent::ClipboardWrite { write, .. })) = ready.pop() else {
        return Err("expected clipboard notification after attachment".into());
    };
    let text = write.take().ok_or("missing clipboard text")?;
    assert_eq!(text.len(), MAX_CLIPBOARD_TEXT);
    assert!(text.starts_with("1:127:"));

    for batch in 2..4 {
        client.ping()?;
        let Update::Event(ClientEvent::ClipboardWrite { write, .. }) = updates.recv_blocking()?.2
        else {
            return Err("clipboard notification was not rearmed".into());
        };
        if batch == 2 {
            drop(write);
        } else {
            assert_eq!(write.take().as_deref(), Some("3:0:"));
        }
        expect_clipboard_barrier(&updates)?;
    }
    work.send((1, Work::Stop))?;
    client.disconnect();
    server.join().map_err(|_| "clipboard server panicked")??;
    Ok(())
}

fn expect_clipboard_barrier(updates: &Updates) -> TestResult {
    assert!(matches!(
        updates.recv_blocking()?.2,
        Update::Event(ClientEvent::Metadata {
            event: MetadataEvent::Title(title),
            ..
        }) if title == "clipboard batch delivered"
    ));
    Ok(())
}

fn clipboard_server(listener: &LocalListener) -> TestResult {
    let (socket, _) = listener.accept()?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut decoder = Decoder::new(socket.try_clone()?);
    let mut encoder = Encoder::new(socket);
    assert!(matches!(decoder.next()?, (CONTROL, Message::Hello { .. })));
    encoder.send(
        CONTROL,
        &Message::HelloReply {
            versions: SUPPORTED.to_vec(),
            server: muxy_protocol::ServerInfo::current(),
            features: Vec::new(),
        },
    )?;
    identify_desktop(&mut decoder, &mut encoder)?;
    let mut batch = 0;
    while let Ok((CONTROL, Message::Request { id, body })) = decoder.next() {
        let reply = match body {
            RequestBody::ListSessions => ReplyBody::Sessions(Vec::new()),
            RequestBody::Ping => {
                let count = if batch == 1 { 128 } else { 1 };
                for index in 0..count {
                    let mut text = format!("{batch}:{index}:");
                    if batch == 1 {
                        text.extend(std::iter::repeat_n('x', MAX_CLIPBOARD_TEXT - text.len()));
                    }
                    encoder.send(
                        ChannelId(1),
                        &Message::Metadata(MetadataEvent::ClipboardWrite(text)),
                    )?;
                }
                encoder.send(
                    ChannelId(1),
                    &Message::Metadata(MetadataEvent::Title("clipboard batch delivered".into())),
                )?;
                batch += 1;
                ReplyBody::Pong
            }
            _ => return Err("unexpected clipboard test request".into()),
        };
        encoder.send(CONTROL, &Message::Reply { id, body: reply })?;
    }
    Ok(())
}
