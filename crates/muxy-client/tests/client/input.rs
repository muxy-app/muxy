use super::*;
use muxy_protocol::{Feature, KeyAction, KeyEvent, TerminalInput};

fn ready(connection: &Connection, attachment: &mut Attachment) -> TestResult {
    if !text(&attachment.grid).contains("INPUT_READY") {
        let frame = connection.frame_containing(attachment, "INPUT_READY")?;
        connection.client.ack(attachment.channel, frame.seq)?;
    }
    connection.quiet(attachment)
}

#[test]
fn server_encodes_keys_paste_and_focus_into_the_real_pty() -> TestResult {
    let expected = b"\x1b[13u\x1b[13;1:2u\x1b[13;1:3u\x1b[200~one\ntwo [201~\x1b[201~\x1b[I\x15";
    let fixture = Fixture::with_startup(&format!(
        r"stty raw -echo
printf '\033[>11u\033[?2004h\033[?1004hINPUT_READY'
dd bs=1 count={} of=input 2>/dev/null
printf '\r\nGOT:'
od -An -v -tx1 input | tr -d ' \n'
printf ':DONE'
exec /bin/cat",
        expected.len()
    ))?;
    let connection = fixture.connect()?;
    assert!(connection.client.supports(Feature::TERMINAL_INPUT));
    let session = fixture.create(&connection.client)?;
    let mut attachment = connection.client.attach(session.id, SIZE)?;
    assert!(attachment.server_input);
    ready(&connection, &mut attachment)?;
    for action in [KeyAction::Press, KeyAction::Repeat, KeyAction::Release] {
        connection.client.send_terminal_input(
            attachment.channel,
            TerminalInput::Key(KeyEvent {
                key: "enter".into(),
                action,
                ..KeyEvent::default()
            }),
        )?;
    }
    connection.client.send_terminal_input(
        attachment.channel,
        TerminalInput::Paste(b"one\r\ntwo\x1b[201~".to_vec()),
    )?;
    connection
        .client
        .send_terminal_input(attachment.channel, TerminalInput::Focus(true))?;
    connection.client.send_input(attachment.channel, b"\x15")?;
    connection.frame_containing(&mut attachment, ":DONE")?;
    assert_eq!(fs::read(fixture.directory.join("input"))?, expected);
    assert!(matches!(
        connection
            .client
            .send_terminal_input(CONTROL, TerminalInput::Focus(true)),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    Ok(())
}

#[test]
fn long_key_text_reaches_the_pty_without_ending_the_session() -> TestResult {
    let fixture = Fixture::with_startup(
        r"stty raw -echo
printf INPUT_READY
dd bs=1 count=40 of=input 2>/dev/null
printf '\r\nINPUT_ACCEPTED'
dd bs=1 count=1 of=next 2>/dev/null
printf '\r\nSTILL_ALIVE'
exec /bin/cat",
    )?;
    let connection = fixture.connect()?;
    let session = fixture.create(&connection.client)?;
    let mut attachment = connection.client.attach(session.id, SIZE)?;
    ready(&connection, &mut attachment)?;
    connection.client.send_terminal_input(
        attachment.channel,
        TerminalInput::Key(KeyEvent {
            text: "x".repeat(40),
            ..KeyEvent::default()
        }),
    )?;
    let frame = connection.frame_containing(&mut attachment, "INPUT_ACCEPTED")?;
    connection.client.ack(attachment.channel, frame.seq)?;
    assert_eq!(fs::read(fixture.directory.join("input"))?, [b'x'; 40]);
    connection.client.send_terminal_input(
        attachment.channel,
        TerminalInput::Key(KeyEvent {
            key: "z".into(),
            text: "z".into(),
            ..KeyEvent::default()
        }),
    )?;
    connection.frame_containing(&mut attachment, "STILL_ALIVE")?;
    assert_eq!(fs::read(fixture.directory.join("next"))?, b"z");
    assert_eq!(connection.client.list_sessions()?, vec![session]);
    Ok(())
}

#[test]
fn ordered_clear_cannot_be_overtaken_by_subsequent_keys() -> TestResult {
    let fixture = Fixture::with_startup(
        r"stty raw -echo
printf '\033]133;A\007INPUT_READY>\033]133;B\007'
dd bs=1 count=3 of=input 2>/dev/null
printf '\r\nGOT:'
od -An -v -tx1 input | tr -d ' \n'
printf ':DONE'
exec /bin/cat",
    )?;
    let connection = fixture.connect()?;
    let session = fixture.create(&connection.client)?;
    let mut attachment = connection.client.attach(session.id, SIZE)?;
    ready(&connection, &mut attachment)?;
    connection.client.send_input(attachment.channel, b"P")?;
    connection
        .client
        .send_terminal_input(attachment.channel, TerminalInput::ClearScreen)?;
    connection.client.send_terminal_input(
        attachment.channel,
        TerminalInput::Key(KeyEvent {
            key: "q".into(),
            text: "Q".into(),
            ..KeyEvent::default()
        }),
    )?;
    connection.frame_containing(&mut attachment, "GOT:500c51:DONE")?;
    assert!(attachment.grid.history.is_empty());
    Ok(())
}

#[test]
fn older_servers_are_not_sent_input_they_would_silently_ignore() -> TestResult {
    let (socket, server) = UnixStream::pair()?;
    server.set_read_timeout(Some(TIMEOUT))?;
    let mut decoder = Decoder::new(server.try_clone()?);
    let mut encoder = Encoder::new(server);
    let fake = thread::spawn(move || -> Result<(), WireError> {
        decoder.next()?;
        encoder.send(
            CONTROL,
            &Message::HelloReply {
                versions: muxy_protocol::SUPPORTED.to_vec(),
                server: muxy_protocol::ServerInfo::current(),
                features: Vec::new(),
            },
        )?;
        assert_eq!(
            decoder.next()?,
            (ChannelId(1), Message::Input(b"legacy".to_vec()))
        );
        Ok(())
    });
    let client = Client::from_stream(Box::new(socket))?;
    assert!(!client.supports(Feature::TERMINAL_INPUT));
    assert!(matches!(
        client.send_terminal_input(ChannelId(1), TerminalInput::ClearScreen),
        Err(ClientError::Invalid(ErrorCode::Unsupported))
    ));
    client.send_input(ChannelId(1), b"legacy")?;
    fake.join().map_err(|_| "fake server panicked")??;
    Ok(())
}
