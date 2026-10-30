use muxy_protocol::{
    ChannelId, HistoryCursor, HistoryPage, MAX_LINK_SPANS, Message, ProjectId, ReplyBody,
    RequestId, Size,
};
use muxy_server::{
    AttachmentEvent, AttachmentId, Registry, ServerSettings, SessionCommand, SessionHandle,
};
use std::fmt::Write;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Fixture {
    directory: PathBuf,
    registry: Registry,
    handle: SessionHandle,
}

impl Fixture {
    fn new(rows: usize, spans: u16, size: Size) -> Result<Self> {
        let directory = std::env::temp_dir().join(format!("muxy-history-{}", ProjectId::new()));
        fs::create_dir(&directory)?;
        let mut output = String::new();
        for row in 0..rows {
            if row % 7 == 0 {
                output.push_str("\x1b]133;A\x07");
            }
            write!(output, "row-{row:03} ")?;
            output.push_str("\x1b]133;B\x07");
            let target = row % 7;
            for column in 0..spans {
                write!(
                    output,
                    "\x1b]8;id={target}-{column};https://test/{target}/{column}\x07x\x1b]8;;\x07"
                )?;
            }
            output.push_str("\x1b]133;C\x07\n\x1b]133;D;0\x07");
        }
        fs::write(directory.join("output"), output)?;
        let shell = directory.join("shell");
        fs::write(&shell, "#!/bin/sh\ncat output\nexec /bin/sleep 60\n")?;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;
        let (events, _) = mpsc::channel();
        let registry = Registry::new(
            ServerSettings {
                default_shell: Some(shell),
                shell_integration: false,
                ..ServerSettings::default()
            },
            events,
        );
        let info = registry.create(&directory, size)?;
        let handle = registry.handle(info.id).ok_or("session disappeared")?;
        Ok(Self {
            directory,
            registry,
            handle,
        })
    }

    fn page(&self, before: HistoryCursor, max_rows: u16) -> Result<HistoryPage> {
        let (reply, receive) = mpsc::channel();
        self.handle.send(SessionCommand::HistoryPage {
            before,
            max_rows,
            reply,
        })?;
        Ok(receive.recv_timeout(Duration::from_secs(5))??)
    }

    fn wait_for_output(&self, total: usize) -> Result<HistoryPage> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let page = self.page(HistoryCursor(0), 200)?;
            if page.total_rows == total as u64 {
                return Ok(page);
            }
            if Instant::now() >= deadline {
                return Err("terminal output did not arrive".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.registry.shutdown();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn check_page(page: &HistoryPage, end: usize, spans: u16) -> Result {
    Message::Reply {
        id: RequestId(1),
        body: ReplyBody::HistoryPage(page.clone()),
    }
    .validate()
    .map_err(|error| format!("{error:?}"))?;
    let links = page.links.as_ref().ok_or("missing links")?;
    for history in [true, false] {
        assert!(
            links
                .iter()
                .filter(|row| (usize::from(row.row) < page.rows.len()) == history)
                .map(|row| row.spans.len())
                .sum::<usize>()
                <= MAX_LINK_SPANS
        );
    }
    let start = end - page.rows.len();
    let screen = page
        .screen
        .iter()
        .flat_map(|screen| screen.rows.iter().take(screen.rows.len() - 1));
    let mut prompts = Vec::new();
    let mut count = 0;
    for (index, row) in page.rows.iter().chain(screen).enumerate() {
        let absolute = start + index;
        let text: String = row.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(
            text.trim_end(),
            format!("row-{absolute:03} {}", "x".repeat(usize::from(spans)))
        );
        let row_links = links.get(index).ok_or("missing row links")?;
        assert_eq!(usize::from(row_links.row), index);
        assert_eq!(row_links.spans.len(), usize::from(spans));
        for (column, span) in row_links.spans.iter().enumerate() {
            assert_eq!(span.uri, format!("https://test/{}/{column}", absolute % 7));
            assert_eq!(usize::from(span.start), 8 + column);
            assert_eq!(span.end, span.start + 1);
        }
        if absolute.is_multiple_of(7) {
            prompts.push(u16::try_from(index)?);
        }
        count += 1;
    }
    assert_eq!(links.len(), count);
    assert_eq!(page.prompts, prompts);
    Ok(())
}

#[test]
fn dense_history_pages_and_attachments_preserve_all_links() -> Result {
    for (rows, spans, height) in [(620, 6, 3), (32, 64, 17)] {
        let size = Size {
            cols: 80,
            rows: height,
        };
        let fixture = Fixture::new(rows, spans, size)?;
        let total = rows - usize::from(height) + 1;
        let recent = fixture.wait_for_output(total)?;
        check_page(&recent, total, spans)?;
        assert_eq!(
            recent.rows.len(),
            (MAX_LINK_SPANS / usize::from(spans)).min(total)
        );
        let (sink, attached) = mpsc::channel();
        fixture.handle.send(SessionCommand::Attach {
            id: AttachmentId(1),
            channel: ChannelId(1),
            size,
            sink: sink.into(),
        })?;
        let AttachmentEvent::Snapshot { snapshot, .. } =
            attached.recv_timeout(Duration::from_secs(5))?
        else {
            return Err("attachment did not start with a snapshot".into());
        };
        assert_eq!(snapshot.history, recent.rows);
        assert_eq!(snapshot.links, recent.links);
        assert_eq!(snapshot.prompts, recent.prompts);
        assert_eq!(snapshot.history_cursor, recent.next);
        assert_eq!(snapshot.history_total, total as u64);
        Message::Reply {
            id: RequestId(1),
            body: ReplyBody::Attached {
                snapshot: Box::new(snapshot),
                process: None,
            },
        }
        .validate()
        .map_err(|error| format!("{error:?}"))?;
        for max_rows in [200, 500] {
            let mut before = HistoryCursor(0);
            let mut end = total;
            loop {
                let page = fixture.page(before, max_rows)?;
                check_page(&page, end, spans)?;
                assert_eq!(page.total_rows, total as u64);
                assert!(page.rows.len() <= usize::from(max_rows));
                assert!(before.0 == 0 || !page.rows.is_empty());
                end -= page.rows.len();
                let Some(next) = page.next else { break };
                assert_ne!(next, before);
                before = next;
            }
            assert_eq!(end, 0);
        }
    }
    Ok(())
}
