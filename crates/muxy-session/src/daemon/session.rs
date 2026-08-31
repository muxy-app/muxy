use crate::process_tree::ProcessTracker;
use crate::pty::PseudoTerminal;
use muxy_proto::session::replay::{ReplayBuffer, ReplaySnapshot};
use muxy_proto::session::{
    Attached, CreateSessionRequest, MAX_PENDING_OUTPUT_BYTES, MAX_STREAM_CHUNK_BYTES, Resize,
    SessionDescriptor, SessionExited, SessionStatus,
};
use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum SessionEvent {
    Output {
        sequence: u64,
        bytes: Vec<u8>,
        pending_bytes: Arc<AtomicUsize>,
    },
    Exited(SessionExited),
}

pub struct AttachmentRegistration {
    pub attached: Attached,
    pub replay: ReplaySnapshot,
    pub receiver: Receiver<SessionEvent>,
}

pub struct DaemonSession {
    request: CreateSessionRequest,
    inner: Mutex<SessionInner>,
    tracker: Mutex<ProcessTracker>,
}

struct SessionInner {
    descriptor: SessionDescriptor,
    pty: Option<PseudoTerminal>,
    replay: ReplayBuffer,
    next_sequence: u64,
    next_attachment_generation: u64,
    attachment: Option<Attachment>,
    terminating: bool,
}

struct Attachment {
    generation: u64,
    last_resize_generation: u64,
    sender: Sender<SessionEvent>,
    pending_bytes: Arc<AtomicUsize>,
    shutdown: UnixStream,
}

impl DaemonSession {
    pub fn spawn(request: CreateSessionRequest, socket_path: &Path) -> io::Result<Arc<Self>> {
        let pty = PseudoTerminal::spawn(&request, socket_path)?;
        let descriptor = SessionDescriptor {
            session_id: request.session_id,
            owner: request.owner.clone(),
            placement: request.placement.clone(),
            title: request.title.clone(),
            working_directory: request.working_directory.clone(),
            shell: pty.child,
            process_session_id: pty.process_session_id,
            process_group_id: pty.process_group_id,
            tty_device: pty.tty_device,
            created_at_milliseconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
            renderer_attached: false,
            status: SessionStatus::Running,
        };
        let tracker = ProcessTracker::new(
            pty.child,
            pty.process_group_id,
            pty.process_session_id,
            pty.tty_device,
        )?;
        let session = Arc::new(Self {
            request,
            inner: Mutex::new(SessionInner {
                descriptor,
                pty: Some(pty),
                replay: ReplayBuffer::new(muxy_proto::session::MAX_REPLAY_BYTES),
                next_sequence: 1,
                next_attachment_generation: 1,
                attachment: None,
                terminating: false,
            }),
            tracker: Mutex::new(tracker),
        });
        let worker = Arc::clone(&session);
        std::thread::Builder::new()
            .name(format!("session-{}", session.request.session_id))
            .spawn(move || worker.run())?;
        Ok(session)
    }

    pub fn request(&self) -> &CreateSessionRequest {
        &self.request
    }

    pub fn descriptor(&self) -> SessionDescriptor {
        self.lock().descriptor.clone()
    }

    pub fn set_placement(
        &self,
        placement: Option<muxy_proto::session::WorkspacePlacement>,
    ) -> io::Result<SessionDescriptor> {
        if let Some(value) = &placement {
            value
                .validate()
                .map_err(|error| invalid(error.to_string()))?;
        }
        let mut inner = self.lock();
        inner.descriptor.placement = placement;
        Ok(inner.descriptor.clone())
    }

    pub fn attach(
        &self,
        size: muxy_proto::session::WindowSize,
        shutdown: UnixStream,
    ) -> io::Result<AttachmentRegistration> {
        size.validate()
            .map_err(|error| invalid(error.to_string()))?;
        let mut inner = self.lock();
        if inner.terminating || inner.descriptor.status != SessionStatus::Running {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "session is not running",
            ));
        }
        let generation = inner.next_attachment_generation;
        inner.next_attachment_generation = generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("attachment generation exhausted"))?;
        if let Some(previous) = inner.attachment.take() {
            let _ = previous.shutdown.shutdown(Shutdown::Both);
        }
        inner
            .pty
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "session PTY is unavailable"))?
            .resize(size)?;
        let replay = inner.replay.snapshot();
        let next_output_sequence = inner.next_sequence;
        let (sender, receiver) = channel();
        inner.attachment = Some(Attachment {
            generation,
            last_resize_generation: 0,
            sender,
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            shutdown,
        });
        inner.descriptor.renderer_attached = true;
        Ok(AttachmentRegistration {
            attached: Attached {
                session: inner.descriptor.clone(),
                attachment_generation: generation,
                replay_generation: replay.generation,
                next_output_sequence,
            },
            replay,
            receiver,
        })
    }

    pub fn detach(&self, generation: u64) {
        let mut inner = self.lock();
        if inner
            .attachment
            .as_ref()
            .is_some_and(|attachment| attachment.generation == generation)
        {
            inner.attachment = None;
            inner.descriptor.renderer_attached = false;
        }
    }

    pub fn input(&self, generation: u64, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MAX_STREAM_CHUNK_BYTES {
            return Err(invalid("input chunk is too large"));
        }
        let mut inner = self.lock();
        ensure_current_attachment(&inner, generation)?;
        inner
            .pty
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "session PTY is unavailable"))?
            .write_all_nonblocking(bytes)
    }

    pub fn resize(&self, resize: Resize) -> io::Result<()> {
        resize
            .size
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        let mut inner = self.lock();
        let attachment = inner
            .attachment
            .as_mut()
            .filter(|attachment| attachment.generation == resize.attachment_generation)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotConnected, "stale attachment generation")
            })?;
        if resize.resize_generation <= attachment.last_resize_generation {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "stale resize generation",
            ));
        }
        attachment.last_resize_generation = resize.resize_generation;
        inner
            .pty
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "session PTY is unavailable"))?
            .resize(resize.size)
    }

    pub fn terminate(&self) -> io::Result<()> {
        {
            let mut inner = self.lock();
            inner.terminating = true;
            if let Some(attachment) = inner.attachment.take() {
                let _ = attachment.shutdown.shutdown(Shutdown::Both);
            }
            inner.descriptor.renderer_attached = false;
        }
        self.lock_tracker().terminate_all(Duration::from_secs(2))?;
        let mut inner = self.lock();
        inner.pty = None;
        inner.descriptor.status = SessionStatus::Exited { status: None };
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.lock().descriptor.status == SessionStatus::Running
    }

    fn complete_natural_exit(&self, status: Option<i32>) -> io::Result<()> {
        self.lock().terminating = true;
        if let Err(error) = self.lock_tracker().terminate_all(Duration::from_secs(2)) {
            disconnect_attachment(&mut self.lock());
            return Err(error);
        }
        let mut inner = self.lock();
        inner.pty = None;
        inner.descriptor.status = SessionStatus::Exited { status };
        if let Some(attachment) = inner.attachment.take() {
            let _ = attachment
                .sender
                .send(SessionEvent::Exited(SessionExited { status }));
            let _ = attachment.shutdown.shutdown(Shutdown::Both);
        }
        inner.descriptor.renderer_attached = false;
        Ok(())
    }

    fn run(self: Arc<Self>) {
        let mut bytes = vec![0; MAX_STREAM_CHUNK_BYTES];
        let mut next_observation = Instant::now();
        loop {
            if Instant::now() >= next_observation {
                let _ = self.lock_tracker().observe();
                next_observation = Instant::now() + Duration::from_millis(50);
            }
            let mut inner = self.lock();
            if inner.terminating {
                return;
            }
            let read = match inner.pty.as_mut() {
                Some(pty) => pty.read(&mut bytes),
                None => return,
            };
            let mut should_sleep = true;
            match read {
                Ok(0) => {}
                Ok(count) => {
                    should_sleep = false;
                    let sequence = inner.next_sequence;
                    let Some(next_sequence) = sequence.checked_add(1) else {
                        drop(inner);
                        let _ = self.complete_natural_exit(None);
                        return;
                    };
                    inner.next_sequence = next_sequence;
                    if inner.replay.append(sequence, &bytes[..count]).is_err() {
                        inner.replay.clear();
                    }
                    send_output(&mut inner, sequence, &bytes[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
                Err(_) => {}
            }
            let status = inner
                .pty
                .as_ref()
                .and_then(|pty| pty.wait_status().ok())
                .flatten();
            if let Some(status) = status {
                drop(inner);
                let _ = self.complete_natural_exit((status >= 0).then_some(status));
                return;
            }
            drop(inner);
            if should_sleep {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, SessionInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_tracker(&self) -> MutexGuard<'_, ProcessTracker> {
        self.tracker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn send_output(inner: &mut SessionInner, sequence: u64, bytes: &[u8]) {
    let Some(attachment) = inner.attachment.as_ref() else {
        return;
    };
    let reserved = attachment
        .pending_bytes
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
            pending
                .checked_add(bytes.len())
                .filter(|total| *total <= MAX_PENDING_OUTPUT_BYTES)
        })
        .is_ok();
    if !reserved {
        disconnect_attachment(inner);
        return;
    }
    let event = SessionEvent::Output {
        sequence,
        bytes: bytes.to_vec(),
        pending_bytes: Arc::clone(&attachment.pending_bytes),
    };
    if attachment.sender.send(event).is_err() {
        attachment
            .pending_bytes
            .fetch_sub(bytes.len(), Ordering::AcqRel);
        disconnect_attachment(inner);
    }
}

fn disconnect_attachment(inner: &mut SessionInner) {
    if let Some(attachment) = inner.attachment.take() {
        let _ = attachment.shutdown.shutdown(Shutdown::Both);
    }
    inner.descriptor.renderer_attached = false;
}

fn ensure_current_attachment(inner: &SessionInner, generation: u64) -> io::Result<()> {
    if inner
        .attachment
        .as_ref()
        .is_some_and(|attachment| attachment.generation == generation)
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stale attachment generation",
        ))
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
