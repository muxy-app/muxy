use super::*;
use muxy_protocol::{HistoryCursor, SearchSource};

#[test]
fn clear_is_shared_invalidates_history_and_survives_reattachment() -> TestResult {
    let fixture = Fixture::with_storage(
        "stty -echo; seq 1 1000; printf CLEAR_READY; exec /bin/cat",
        true,
    )?;
    let first = fixture.connect()?;
    let session = fixture.create(&first.client)?;
    let mut attachment = first.client.attach(session.id, SIZE)?;
    if !text(&attachment.grid).contains("CLEAR_READY") {
        let frame = first.frame_containing(&mut attachment, "CLEAR_READY")?;
        first.client.ack(attachment.channel, frame.seq)?;
    }
    first.quiet(&mut attachment)?;
    let old = first
        .client
        .history_page(attachment.channel, HistoryCursor(0), 200)?;
    let cursor = old.next.ok_or("missing older history")?;
    let second = fixture.connect()?;
    let mut other = second.client.attach(session.id, SIZE)?;
    assert!(!other.grid.history.is_empty());
    first.client.clear_screen(attachment.channel)?;
    for (connection, view) in [(&first, &mut attachment), (&second, &mut other)] {
        let frame = connection.next_frame(view.channel)?;
        assert!(frame.reset);
        view.grid.apply(&frame);
        connection.client.ack(view.channel, frame.seq)?;
        assert!(view.grid.history.is_empty());
        assert_eq!(view.grid.row_text(0).trim_end(), "CLEAR_READY");
        assert!(
            view.grid
                .rows
                .iter()
                .skip(1)
                .flatten()
                .all(|run| run.text.trim().is_empty())
        );
        let history = connection
            .client
            .history_page(view.channel, HistoryCursor(0), 200)?;
        assert!(history.rows.is_empty());
        assert_eq!(history.total_rows, 0);
        assert!(history.next.is_none());
    }
    assert!(
        matches!(first.client.history_page(attachment.channel, cursor, 200), Err(ClientError::Server(error)) if error.code == ErrorCode::StaleHistoryCursor)
    );
    let search = first.client.search(
        SearchSource::Live(attachment.channel),
        "999",
        false,
        HistoryCursor(0),
        200,
    )?;
    assert!(search.matches.is_empty());
    let third = fixture.connect()?;
    let restored = third.client.attach(session.id, SIZE)?;
    assert!(restored.grid.history.is_empty());
    assert_eq!(restored.grid.row_text(0).trim_end(), "CLEAR_READY");
    assert!(matches!(
        first.client.clear_screen(CONTROL),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    assert!(
        matches!(first.client.clear_screen(ChannelId(u32::MAX)), Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownChannel)
    );
    first.client.end_session(session.id)?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Ok(saved) = first.client.read_saved_screen(session.id) {
            assert_eq!(
                saved.rows[0]
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
                    .trim_end(),
                "CLEAR_READY"
            );
            let history = first
                .client
                .saved_history_page(session.id, HistoryCursor(0), 200)?;
            assert!(history.rows.is_empty());
            break;
        }
        if Instant::now() >= deadline {
            return Err("cleared screen was not archived".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[test]
fn clearing_a_semantic_prompt_delivers_the_shell_redraw() -> TestResult {
    let fixture = Fixture::with_startup(
        r#"stty -echo -icanon min 1 time 0
seq 1 1000
printf '\033]133;A\007PROMPT_READY>\033]133;B\007'
key=$(dd bs=1 count=1 2>/dev/null)
if [ "$key" = "$(printf '\014')" ]; then
    printf '\033[H\033[2JREDRAW_OK'
else
    printf 'WRONG_INPUT'
fi
exec /bin/cat"#,
    )?;
    let connection = fixture.connect()?;
    let session = fixture.create(&connection.client)?;
    let mut attachment = connection.client.attach(session.id, SIZE)?;
    if !text(&attachment.grid).contains("PROMPT_READY>") {
        let frame = connection.frame_containing(&mut attachment, "PROMPT_READY>")?;
        connection.client.ack(attachment.channel, frame.seq)?;
    }
    connection.quiet(&mut attachment)?;
    connection.client.clear_screen(attachment.channel)?;
    let frame = connection.frame_containing(&mut attachment, "REDRAW_OK")?;
    connection.client.ack(attachment.channel, frame.seq)?;
    assert_eq!(attachment.grid.row_text(0).trim_end(), "REDRAW_OK");
    let history = connection
        .client
        .history_page(attachment.channel, HistoryCursor(0), 200)?;
    assert_eq!(history.total_rows, 0);
    assert!(history.rows.is_empty());
    Ok(())
}
