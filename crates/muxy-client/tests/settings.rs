use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Client, ClientEvent};
use muxy_protocol::{ServerPath, Size};
use muxy_server::{Registry, ServerSettings, connection};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn settings_round_trip_validate_persist_and_reach_new_shells_only() -> TestResult {
    let (send, events) = mpsc::channel();
    let persisted = Arc::new(Mutex::new(Vec::new()));
    let writes = persisted.clone();
    let failing = Arc::new(AtomicBool::new(false));
    let fail = failing.clone();
    let registry = Arc::new(
        Registry::new(
            ServerSettings {
                default_shell: Some("/bin/sh".into()),
                ..ServerSettings::default()
            },
            send,
        )
        .with_settings_persistence(move |settings| {
            if fail.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("test disk failure"));
            }
            writes
                .lock()
                .map_err(|_| std::io::Error::other("poisoned"))?
                .push(settings.clone());
            Ok(())
        }),
    );
    let (local, remote) = UnixStream::pair()?;
    let sessions = registry.clone();
    let serving = thread::spawn(move || connection::serve(Box::new(remote), sessions, events));
    let client = Client::from_stream(Box::new(local))?;
    let result = (|| -> TestResult {
        let size = Size {
            cols: 100,
            rows: 24,
        };
        let old = client.create_session(&std::env::temp_dir(), size)?;
        let original = client.read_server_settings()?;
        assert_eq!(original, registry.settings().document());
        let mut changed = original.clone();
        changed.default_shell = Some(ServerPath(b"/not/a/shell".to_vec()));
        assert!(client.write_server_settings(changed.clone()).is_err());
        assert_eq!(client.read_server_settings()?, original);
        changed.default_shell = Some(ServerPath(b"/bin/bash".to_vec()));
        changed.history_budget_bytes = 2 * 1024 * 1024;
        changed.shell_integration = false;
        failing.store(true, Ordering::SeqCst);
        assert!(client.write_server_settings(changed.clone()).is_err());
        assert_eq!(client.read_server_settings()?, original);
        failing.store(false, Ordering::SeqCst);
        client.write_server_settings(changed.clone())?;
        assert_eq!(client.read_server_settings()?, changed);
        assert_eq!(persisted.lock().map_err(|_| "poisoned")?.len(), 1);
        let new = client.create_session(&std::env::temp_dir(), size)?;
        let events = client.events().ok_or("missing events")?;
        for (session, expected) in [
            (old.id, "shell-path=/bin/sh"),
            (new.id, "shell-path=/bin/bash"),
        ] {
            verify_shell(&client, &events, session, size, expected)?;
        }
        client.discard_session(old.id)?;
        client.discard_session(new.id)?;
        Ok(())
    })();
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "connection panicked")??;
    result
}

fn verify_shell(
    client: &Client,
    events: &mpsc::Receiver<ClientEvent>,
    session: muxy_protocol::SessionId,
    size: Size,
    expected: &str,
) -> TestResult {
    let mut attachment = client.attach(session, size)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while attachment
        .grid
        .rows
        .iter()
        .flatten()
        .all(|run| run.text.trim().is_empty())
    {
        if let ClientEvent::Frame { channel, frame } = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| {
                format!(
                    "waiting for {expected} startup: {error}; rows={:?}",
                    attachment.grid.rows
                )
            })?
        {
            client.ack(channel, frame.seq)?;
            if channel == attachment.channel {
                attachment.grid.apply(&frame);
            }
        }
    }
    client.send_input(
        attachment.channel,
        b"printf '\\nshell-path=%s\\n' \"$SHELL\"\n",
    )?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if attachment
            .grid
            .rows
            .iter()
            .enumerate()
            .any(|(row, _)| attachment.grid.row_text(row).contains(expected))
        {
            break;
        }
        if let ClientEvent::Frame { channel, frame } = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| {
                format!(
                    "waiting for {expected} output: {error}; rows={:?}",
                    attachment.grid.rows
                )
            })?
        {
            client.ack(channel, frame.seq)?;
            if channel == attachment.channel {
                attachment.grid.apply(&frame);
            }
        }
    }
    client.detach(attachment.channel)?;
    Ok(())
}
