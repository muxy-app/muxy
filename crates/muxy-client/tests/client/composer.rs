use super::*;

#[test]
fn acknowledged_input_writes_in_order_and_rejects_detached_channels() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let client = &connection.client;
    let info = fixture.create(client)?;
    let mut attachment = client.attach(info.id, SIZE)?;
    client.write_input(attachment.channel, b"printf 'compo".to_vec())?;
    client.write_input(attachment.channel, b"ser-ok\\n'\r".to_vec())?;
    connection.frame_containing(&mut attachment, "composer-ok")?;
    assert!(matches!(
        client.write_input(CONTROL, vec![1]),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    assert!(matches!(
        client.write_input(attachment.channel, vec![0; muxy_protocol::MAX_INPUT + 1]),
        Err(ClientError::Invalid(ErrorCode::BadRequest))
    ));
    client.detach(attachment.channel)?;
    assert!(
        matches!(client.write_input(attachment.channel, b"must not be accepted".to_vec()), Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownChannel)
    );
    client.ping()?;
    Ok(())
}
