use super::*;
use std::error::Error;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

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
fn queued_history_precedes_later_detach() -> TestResult {
    let mut connection = Connection::new()?;
    let size = Size { cols: 80, rows: 24 };
    let session = connection
        .requests
        .registry
        .create(&std::env::temp_dir(), size)?
        .id;
    connection.send(1, RequestBody::Attach { session, size })?;
    let (_, body) = connection.reply()?;
    let ReplyBody::Attached { snapshot, .. } = body else {
        return Err("expected attachment".into());
    };
    let release = connection.block()?;
    connection.send(
        2,
        RequestBody::HistoryPage {
            channel: snapshot.channel,
            before: HistoryCursor(0),
            max_rows: 100,
        },
    )?;
    connection.send(3, RequestBody::Detach(snapshot.channel))?;
    release.send(())?;
    let (id, body) = connection.reply()?;
    assert_eq!(id, RequestId(2));
    assert!(matches!(body, ReplyBody::HistoryPage(_)));
    assert_eq!(connection.reply()?, (RequestId(3), ReplyBody::Detached));
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
fn git_and_files_each_execute_reads_while_another_read_is_pending() -> TestResult {
    let mut connection = Connection::new()?;
    for git in [true, false] {
        let (release, gate) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        let workers = if git {
            &connection.requests.git_readers
        } else {
            &connection.requests.files_readers
        };
        workers.try_spawn(move || {
            let _ = started.send(());
            let _ = gate.recv_timeout(Duration::from_secs(5));
        })?;
        ready.recv_timeout(Duration::from_secs(2))?;
        let project = muxy_protocol::ProjectId::new();
        connection.send(
            1,
            if git {
                RequestBody::Git(muxy_protocol::GitRequest {
                    project,
                    action: muxy_protocol::GitAction::Summary,
                })
            } else {
                RequestBody::Files(muxy_protocol::FilesRequest {
                    project,
                    action: muxy_protocol::FilesAction::List(muxy_protocol::ServerPath(Vec::new())),
                })
            },
        )?;
        let result = connection.replies.recv_timeout(Duration::from_secs(2));
        release.send(())?;
        assert!(matches!(result?, (RequestId(1), ReplyBody::Error(_))));
    }
    Ok(())
}
