use super::*;

#[test]
fn one_shot_attachments_preserve_unattended_size_and_leave_sessions_running() -> TestResult {
    let fixture = Fixture::new()?;
    let owner = fixture.connect()?;
    let size = Size {
        cols: 137,
        rows: 41,
    };
    let session = owner.client.create_session(&fixture.directory, size)?;
    for size in [size, Size { cols: 99, rows: 29 }] {
        let attachment = owner.client.attach(session.id, size)?;
        owner.client.resize(attachment.channel, size)?;
        owner.client.detach(attachment.channel)?;
        let observer = fixture.connect()?;
        let mut attachment = observer.client.attach_without_resize(session.id)?;
        assert_eq!(attachment.grid.size, size);
        observer.client.write_input(
            attachment.channel,
            b"printf '\\nOBSERVED_%s\\n' 'OUTPUT'\r".to_vec(),
        )?;
        observer.frame_containing(&mut attachment, "OBSERVED_OUTPUT")?;
        observer.client.detach(attachment.channel)?;
        assert_eq!(owner.client.list_sessions()?[0].id, session.id);
    }
    Ok(())
}
