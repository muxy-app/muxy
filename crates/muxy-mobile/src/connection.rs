use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use muxy_client::{Client, ClientError, ClientEvent, RemoteEndpoint};
use muxy_protocol::{
    DeviceCredential, ErrorCode, OperationId, ProjectId, ProjectSession, SessionId, Size,
};

use crate::MobileError;
use crate::files::{FilesAction, FilesReply};
use crate::git::{GitAction, GitReply};
use crate::records::{Activity, Project, ServerCredential, Session, SessionStatus, text};
use crate::terminal::Terminal;
use crate::terminals::{Delivery, Terminals};

/// Receives connection events on an SDK thread; hop to the main thread before touching UI.
#[uniffi::export(with_foreign)]
pub trait ConnectionListener: Send + Sync {
    fn on_event(&self, event: ConnectionEvent);
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ConnectionEvent {
    /// The terminal's screen changed; read it again.
    ScreenChanged {
        session_id: u64,
    },
    /// The terminal's title, directory, history size, or mouse modes changed.
    MetadataChanged {
        session_id: u64,
    },
    SessionEnded {
        session_id: u64,
    },
    /// Projects changed; read them again.
    CatalogChanged,
    /// Sessions were created, ended, or attached; read them again.
    SessionsChanged,
    /// Agent activity changed; read it again.
    ActivityChanged,
    /// The repository of the project that `GitAction::Watch` watches changed.
    GitChanged {
        project_id: String,
    },
    /// Files changed in a project that `FilesAction::Watch` watches. `paths`
    /// are relative to the project's folder; empty means anything may have changed.
    FilesChanged {
        project_id: String,
        paths: Vec<String>,
    },
    /// The server is restarting for an update; connect again shortly.
    ServerRestarting,
    /// The connection closed; connect again, then attach again.
    Disconnected,
}

/// A live connection to one paired server.
#[derive(uniffi::Object)]
pub struct Connection {
    client: Client,
    terminals: Arc<Terminals>,
    /// Frames are held only for channels newer than every attached one, which
    /// is true only while attaches run one at a time.
    attaching: Mutex<()>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Connection")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.client.disconnect();
    }
}

#[uniffi::export]
impl Connection {
    /// Connects with a saved pairing, trying each of the server's addresses.
    #[uniffi::constructor]
    pub fn connect(
        credential: ServerCredential,
        listener: Arc<dyn ConnectionListener>,
    ) -> Result<Arc<Self>, MobileError> {
        let endpoint = RemoteEndpoint {
            hosts: credential.hosts,
            port: credential.port,
            fingerprint: credential
                .fingerprint
                .as_slice()
                .try_into()
                .map_err(|_| MobileError::InvalidCredential)?,
        };
        let device = DeviceCredential {
            device: credential
                .device_id
                .parse()
                .map_err(|_| MobileError::InvalidCredential)?,
            token: credential
                .token
                .as_slice()
                .try_into()
                .map_err(|_| MobileError::InvalidCredential)?,
        };
        let client = Client::connect_remote(&endpoint, device)?;
        let events = client.events().ok_or(MobileError::Disconnected)?;
        let terminals = Arc::<Terminals>::default();
        let pump = (client.clone(), Arc::clone(&terminals));
        thread::Builder::new()
            .name("muxy-mobile-events".into())
            .spawn(move || deliver(&pump.0, &events, &pump.1, listener.as_ref()))
            .map_err(|error| MobileError::Unreachable {
                reason: error.to_string(),
            })?;
        Ok(Arc::new(Self {
            client,
            terminals,
            attaching: Mutex::new(()),
        }))
    }

    pub fn server_version(&self) -> String {
        self.client.server_info().build.version.clone()
    }

    pub fn projects(&self) -> Result<Vec<Project>, MobileError> {
        Ok(self
            .client
            .catalog()?
            .projects
            .iter()
            .map(Project::from)
            .collect())
    }

    /// Every session of a project, including ended ones whose content is kept.
    pub fn sessions(&self, project_id: String) -> Result<Vec<Session>, MobileError> {
        let project = project(&project_id)?;
        Ok(all_sessions(&self.client, project)?
            .iter()
            .map(Session::from)
            .collect())
    }

    /// Starts a shell in the project's directory.
    pub fn create_session(
        &self,
        project_id: String,
        columns: u16,
        rows: u16,
    ) -> Result<Session, MobileError> {
        let project = project(&project_id)?;
        let catalog = self.client.catalog()?;
        let descriptor = catalog
            .projects
            .iter()
            .find(|descriptor| descriptor.id == project)
            .ok_or_else(|| MobileError::Server {
                reason: "The project no longer exists.".into(),
            })?;
        let info = self.client.create_project_session(
            project,
            OperationId::new(),
            Path::new(OsStr::from_bytes(&descriptor.directory.0)),
            Size {
                cols: columns,
                rows,
            },
        )?;
        Ok(Session {
            id: info.id.get(),
            project_id,
            directory: String::from_utf8_lossy(&info.directory.0).into_owned(),
            status: SessionStatus::Live,
            owner: None,
            attached: false,
        })
    }

    /// Ends the session for every client; its panes close everywhere.
    pub fn end_session(&self, session_id: u64) -> Result<(), MobileError> {
        Ok(self.client.end_session(session(session_id)?)?)
    }

    /// Attaches without resizing a session other clients already show.
    pub fn attach(
        &self,
        session_id: u64,
        columns: u16,
        rows: u16,
    ) -> Result<Arc<Terminal>, MobileError> {
        let session = session(session_id)?;
        let _attaching = self
            .attaching
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let attachment = self.client.attach(
            session,
            Size {
                cols: columns,
                rows,
            },
        )?;
        let channel = attachment.channel;
        let (view, acknowledge) = self.terminals.install(session, attachment);
        if let Some(seq) = acknowledge {
            self.client.ack(channel, seq)?;
        }
        Ok(Arc::new(Terminal {
            client: self.client.clone(),
            view,
            terminals: Arc::clone(&self.terminals),
        }))
    }

    pub fn activity(&self) -> Result<Activity, MobileError> {
        Ok(self.client.activity()?.into())
    }

    /// Marks events as seen on every client.
    pub fn acknowledge_activity(&self, event_ids: Vec<u64>) -> Result<(), MobileError> {
        Ok(self.client.acknowledge_activity(event_ids)?)
    }

    /// Runs a Git action in the project's repository, as the desktop does.
    pub fn git(&self, project_id: String, action: GitAction) -> Result<GitReply, MobileError> {
        let request = muxy_protocol::GitRequest {
            project: project(&project_id)?,
            action: action.into(),
        };
        Ok(self.client.git(request)?.into())
    }

    /// Runs a files action in the project's folder, as the desktop does.
    pub fn files(
        &self,
        project_id: String,
        action: FilesAction,
    ) -> Result<FilesReply, MobileError> {
        let request = muxy_protocol::FilesRequest {
            project: project(&project_id)?,
            action: action.into(),
        };
        Ok(self.client.files(request)?.into())
    }

    pub fn disconnect(&self) {
        self.client.disconnect();
    }
}

fn project(id: &str) -> Result<ProjectId, MobileError> {
    id.parse().map_err(|_| MobileError::Server {
        reason: "Unknown project.".into(),
    })
}

fn session(id: u64) -> Result<SessionId, MobileError> {
    SessionId::new(id).ok_or_else(|| MobileError::Server {
        reason: "Unknown session.".into(),
    })
}

/// Reads every page of one consistent listing, retrying if the catalog changes meanwhile.
fn all_sessions(client: &Client, project: ProjectId) -> Result<Vec<ProjectSession>, ClientError> {
    let mut changed = None;
    for _ in 0..8 {
        match session_pages(client, project) {
            Err(ClientError::Server(error)) if error.code == ErrorCode::CatalogChanged => {
                changed = Some(error);
            }
            result => return result,
        }
    }
    Err(changed.map_or(ClientError::Timeout, ClientError::Server))
}

fn session_pages(client: &Client, project: ProjectId) -> Result<Vec<ProjectSession>, ClientError> {
    let mut page = client.project_sessions(project, None, None)?;
    let revision = page.revision;
    let mut sessions = std::mem::take(&mut page.sessions);
    while let Some(after) = page.next {
        page = client.project_sessions(project, Some(after), Some(revision))?;
        if page.next.is_some_and(|next| next <= after) {
            return Err(ClientError::Protocol("invalid session pagination".into()));
        }
        sessions.append(&mut page.sessions);
    }
    Ok(sessions)
}

/// Applies frames and acknowledges them, then tells the app what to read again.
fn deliver(
    client: &Client,
    events: &Receiver<ClientEvent>,
    terminals: &Terminals,
    listener: &dyn ConnectionListener,
) {
    for event in events {
        let notice = match event {
            ClientEvent::Frame { channel, frame } => match terminals.frame(channel, frame) {
                Delivery::Applied { session, seq } => {
                    let _ = client.ack(channel, seq);
                    Some(ConnectionEvent::ScreenChanged {
                        session_id: session.get(),
                    })
                }
                Delivery::Held | Delivery::Unknown => None,
            },
            ClientEvent::Metadata { channel, event } => {
                terminals
                    .metadata(channel, event)
                    .map(|session| ConnectionEvent::MetadataChanged {
                        session_id: session.get(),
                    })
            }
            ClientEvent::SessionEnded { session, .. } => {
                terminals.ended(session);
                Some(ConnectionEvent::SessionEnded {
                    session_id: session.get(),
                })
            }
            ClientEvent::CatalogChanged { .. } => Some(ConnectionEvent::CatalogChanged),
            ClientEvent::SessionsChanged { .. } => Some(ConnectionEvent::SessionsChanged),
            ClientEvent::ActivityChanged { .. } => Some(ConnectionEvent::ActivityChanged),
            ClientEvent::GitChanged { project } => Some(ConnectionEvent::GitChanged {
                project_id: project.to_string(),
            }),
            ClientEvent::FilesChanged { project, changes } => Some(ConnectionEvent::FilesChanged {
                project_id: project.to_string(),
                paths: if changes.rescan {
                    Vec::new()
                } else {
                    changes.paths.iter().map(text).collect()
                },
            }),
            ClientEvent::ServerRestarting => Some(ConnectionEvent::ServerRestarting),
            ClientEvent::Disconnected => {
                listener.on_event(ConnectionEvent::Disconnected);
                return;
            }
            ClientEvent::SessionMetadata { .. }
            | ClientEvent::Progress { .. }
            | ClientEvent::RemoteAccessChanged { .. } => None,
        };
        if let Some(notice) = notice {
            listener.on_event(notice);
        }
    }
}
