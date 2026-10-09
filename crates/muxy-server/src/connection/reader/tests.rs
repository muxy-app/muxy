use super::*;
use std::error::Error;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

type TestResult = Result<(), Box<dyn Error>>;

struct Connection {
    requests: Requests,
    replies: Receiver<(RequestId, ReplyBody)>,
    pump: Option<JoinHandle<()>>,
}

impl Connection {
    fn new() -> Result<Self, Box<dyn Error>> {
        let (events, _) = mpsc::channel();
        let registry = Arc::new(Registry::new(
            crate::ServerSettings {
                default_shell: Some("/bin/sh".into()),
                ..crate::ServerSettings::default()
            },
            events,
        ));
        let outbox = Arc::new(Outbox::new(Arc::clone(&registry.attachment_changes)));
        let output = Arc::clone(&outbox);
        let (sender, replies) = mpsc::channel();
        let pump = thread::spawn(move || {
            while let Some((_, message)) = output.next() {
                if let Message::Reply { id, body } = message
                    && sender.send((id, body)).is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            requests: Requests {
                commands: crate::exec::Jobs::default(),
                registry,
                outbox,
                workers: WorkerPool::new("ordering-test", 1, 32)?,
                git_workers: WorkerPool::new("git-test", 1, 16)?,
                git_readers: WorkerPool::new("git-read-test", 4, 32)?,
                git_watch: Arc::new(Mutex::new(None)),
                files_workers: WorkerPool::new("connection-files", 1, 16)?,
                files_readers: WorkerPool::new("files-read-test", 4, 32)?,
                files_watch: Arc::new(Mutex::new(crate::files::watch::Subscriptions::default())),
                search_cache: Arc::new(Mutex::new(SearchCache::default())),
                last_channel: Arc::new(AtomicU32::new(0)),
            },
            replies,
            pump: Some(pump),
        })
    }

    fn send(&mut self, id: u32, body: RequestBody) -> TestResult {
        let id = RequestId(id);
        if let Some(body) = self.requests.route(body, id)? {
            self.requests
                .outbox
                .push_control(Message::Reply { id, body });
        }
        Ok(())
    }

    fn reply(&self) -> Result<(RequestId, ReplyBody), Box<dyn Error>> {
        Ok(self.replies.recv_timeout(Duration::from_secs(5))?)
    }

    fn block(&self) -> Result<mpsc::Sender<()>, Box<dyn Error>> {
        let (release, gate) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        self.requests.workers.try_spawn(move || {
            let _ = started.send(());
            let _ = gate.recv();
        })?;
        ready.recv_timeout(Duration::from_secs(2))?;
        Ok(release)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.requests.outbox.close();
        self.requests.registry.shutdown();
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

#[test]
fn queued_end_precedes_later_attach_while_ping_bypasses_work() -> TestResult {
    let mut connection = Connection::new()?;
    let size = Size { cols: 80, rows: 24 };
    let session = connection
        .requests
        .registry
        .create(&std::env::temp_dir(), size)?
        .id;
    let release = connection.block()?;
    connection.send(1, RequestBody::EndSession(session))?;
    connection.send(2, RequestBody::Attach { session, size })?;
    connection.send(3, RequestBody::Ping)?;
    let first = connection.reply();
    release.send(())?;
    assert_eq!(first?, (RequestId(3), ReplyBody::Pong));
    assert_eq!(connection.reply()?, (RequestId(1), ReplyBody::SessionEnded));
    let (id, body) = connection.reply()?;
    assert_eq!(id, RequestId(2));
    assert!(matches!(body, ReplyBody::Error(error) if error.code == ErrorCode::UnknownSession));
    Ok(())
}

#[test]
fn file_work_does_not_block_ping_or_catalog_requests() -> TestResult {
    let mut connection = Connection::new()?;
    let mut releases = Vec::new();
    for _ in 0..4 {
        let (release, gate) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        connection.requests.files_readers.try_spawn(move || {
            let _ = started.send(());
            let _ = gate.recv_timeout(Duration::from_secs(5));
        })?;
        ready.recv_timeout(Duration::from_secs(2))?;
        releases.push(release);
    }
    connection.send(
        1,
        RequestBody::Files(muxy_protocol::FilesRequest {
            project: muxy_protocol::ProjectId::new(),
            action: muxy_protocol::FilesAction::Read(muxy_protocol::ServerPath(b"file".to_vec())),
        }),
    )?;
    connection.send(2, RequestBody::Ping)?;
    connection.send(
        3,
        RequestBody::ReadCatalog {
            after: None,
            revision: None,
        },
    )?;
    assert_eq!(connection.reply()?, (RequestId(2), ReplyBody::Pong));
    assert!(matches!(
        connection.reply()?,
        (RequestId(3), ReplyBody::Catalog(_))
    ));
    for release in releases {
        release.send(())?;
    }
    assert!(matches!(
        connection.reply()?,
        (RequestId(1), ReplyBody::Error(_))
    ));
    Ok(())
}

#[test]
fn a_channel_is_accepted_before_its_session_can_reply() -> TestResult {
    const ATTACHES: u32 = 200;
    let connection = Connection::new()?;
    let (handle, commands) = crate::SessionHandle::fake();
    let session = handle.id();
    connection.requests.registry.insert_session(handle);
    let registry = Arc::clone(&connection.requests.registry);
    let outbox = Arc::clone(&connection.requests.outbox);
    let last_channel = Arc::clone(&connection.requests.last_channel);
    let attaching = thread::spawn(move || {
        (1..=ATTACHES).try_for_each(|request| {
            attach(
                session,
                None,
                RequestId(request),
                &registry,
                &outbox,
                &last_channel,
            )
        })
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received = Vec::new();
    for channel in 1..=ATTACHES {
        // Polls instead of blocking, to see each command as early as a session thread could.
        let command = loop {
            match commands.try_recv() {
                Ok(command) => break command,
                Err(TryRecvError::Empty) if Instant::now() < deadline => std::hint::spin_loop(),
                Err(_) => return Err(format!("attach {channel} never reached its session").into()),
            }
        };
        assert!(
            connection.requests.last_channel.load(Ordering::Acquire) >= channel,
            "channel {channel} reached its session before the reader accepts it"
        );
        received.push(command);
    }
    attaching.join().map_err(|_| "attaching panicked")??;
    Ok(())
}
