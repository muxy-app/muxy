use super::SessionId;
use super::window_size::WindowSize;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::Path;

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;
pub const MAX_STRUCTURED_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_STREAM_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_PENDING_INPUT_BYTES: usize = 1024 * 1024;
pub const MAX_PENDING_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CONTROL_CONNECTIONS: usize = 64;
pub const MAX_REPLAY_BYTES: usize = 256 * 1024;
pub const MAX_ARGV_ENTRIES: usize = 256;
pub const MAX_ENVIRONMENT_ENTRIES: usize = 512;
pub const MAX_ENVIRONMENT_KEY_BYTES: usize = 256;
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_AGGREGATE_ARGV_BYTES: usize = 256 * 1024;
pub const MAX_OWNER_ID_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const CURRENT: Self = Self {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
    };

    pub fn negotiate(self, peer: Self) -> Result<Self, VersionMismatch> {
        if self.major != peer.major {
            return Err(VersionMismatch {
                supported: self,
                received: peer,
            });
        }
        Ok(Self {
            major: self.major,
            minor: self.minor.min(peer.minor),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMismatch {
    pub supported: ProtocolVersion,
    pub received: ProtocolVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientKind {
    Control,
    Renderer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildMode {
    Development,
    Production,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub version: ProtocolVersion,
    pub client_kind: ClientKind,
    pub process_id: u32,
    pub nonce: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAccepted {
    pub version: ProtocolVersion,
    pub daemon: ProcessIdentity,
    pub daemon_nonce: [u8; 32],
    pub build_mode: BuildMode,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIdentity {
    pub process_id: u32,
    pub start_identity: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOwner {
    pub project_id: String,
    pub worktree_id: String,
    pub original_tab_id: String,
}

impl SessionOwner {
    pub fn validate(&self) -> Result<(), MessageValidationError> {
        validate_id("projectId", &self.project_id)?;
        validate_id("worktreeId", &self.worktree_id)?;
        validate_id("originalTabId", &self.original_tab_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePlacement {
    pub project_id: String,
    pub worktree_id: String,
    pub tab_id: String,
    pub area_id: String,
}

impl WorkspacePlacement {
    pub fn validate(&self) -> Result<(), MessageValidationError> {
        validate_id("placement.projectId", &self.project_id)?;
        validate_id("placement.worktreeId", &self.worktree_id)?;
        validate_id("placement.tabId", &self.tab_id)?;
        validate_id("placement.areaId", &self.area_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub session_id: SessionId,
    pub owner: SessionOwner,
    pub placement: Option<WorkspacePlacement>,
    pub working_directory: String,
    pub initial_size: WindowSize,
    pub shell_executable: String,
    pub argv: Vec<String>,
    pub startup_command: Option<String>,
    pub keep_shell_open: bool,
    pub environment: Vec<EnvironmentEntry>,
    pub ghostty_resources: String,
    pub terminfo: String,
    pub terminal_type: String,
    pub color_terminal: String,
    pub title: String,
}

impl CreateSessionRequest {
    pub fn validate(&self) -> Result<(), MessageValidationError> {
        self.owner.validate()?;
        if let Some(placement) = &self.placement {
            placement.validate()?;
        }
        validate_absolute_path("workingDirectory", &self.working_directory)?;
        validate_absolute_path("shellExecutable", &self.shell_executable)?;
        validate_absolute_path("ghosttyResources", &self.ghostty_resources)?;
        validate_absolute_path("terminfo", &self.terminfo)?;
        self.initial_size
            .validate()
            .map_err(|_| invalid("initialSize"))?;
        if self.argv.len() > MAX_ARGV_ENTRIES {
            return Err(invalid("argv"));
        }
        let mut aggregate_argv = 0usize;
        for value in &self.argv {
            validate_value("argv", value)?;
            aggregate_argv = aggregate_argv
                .checked_add(value.len())
                .ok_or_else(|| invalid("argv"))?;
        }
        if aggregate_argv > MAX_AGGREGATE_ARGV_BYTES {
            return Err(invalid("argv"));
        }
        if let Some(command) = &self.startup_command {
            validate_value("startupCommand", command)?;
        }
        if self.environment.len() > MAX_ENVIRONMENT_ENTRIES {
            return Err(invalid("environment"));
        }
        let mut keys = HashSet::new();
        for entry in &self.environment {
            validate_environment_key(&entry.key)?;
            validate_value("environment.value", &entry.value)?;
            if !keys.insert(entry.key.as_str()) {
                return Err(MessageValidationError::DuplicateEnvironmentKey(
                    entry.key.clone(),
                ));
            }
        }
        for (field, value) in [
            ("terminalType", self.terminal_type.as_str()),
            ("colorTerminal", self.color_terminal.as_str()),
            ("title", self.title.as_str()),
        ] {
            validate_value(field, value)?;
        }
        let encoded = serde_json::to_vec(self).map_err(|_| invalid("request"))?;
        if encoded.len() > MAX_STRUCTURED_FRAME_BYTES {
            return Err(MessageValidationError::StructuredFrameTooLarge(
                encoded.len(),
            ));
        }
        Ok(())
    }

    pub fn same_launch_contract(&self, other: &Self) -> bool {
        self.working_directory == other.working_directory
            && self.initial_size == other.initial_size
            && self.shell_executable == other.shell_executable
            && self.argv == other.argv
            && self.startup_command == other.startup_command
            && self.keep_shell_open == other.keep_shell_open
            && self.environment == other.environment
            && self.ghostty_resources == other.ghostty_resources
            && self.terminfo == other.terminfo
            && self.terminal_type == other.terminal_type
            && self.color_terminal == other.color_terminal
            && self.title == other.title
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateSessionResolution {
    Create,
    Existing(SessionId),
    DuplicateOwnerConflict,
}

pub fn resolve_create_session<'a>(
    existing: impl IntoIterator<Item = &'a CreateSessionRequest>,
    proposed: &CreateSessionRequest,
) -> Result<CreateSessionResolution, MessageValidationError> {
    proposed.validate()?;
    let matches: Vec<_> = existing
        .into_iter()
        .filter(|request| request.owner == proposed.owner)
        .collect();
    match matches.as_slice() {
        [] => Ok(CreateSessionResolution::Create),
        [request] if request.same_launch_contract(proposed) => {
            Ok(CreateSessionResolution::Existing(request.session_id))
        }
        [_] => Ok(CreateSessionResolution::DuplicateOwnerConflict),
        _ => Err(MessageValidationError::DuplicateOwnerInvariant),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDescriptor {
    pub session_id: SessionId,
    pub owner: SessionOwner,
    pub placement: Option<WorkspacePlacement>,
    pub title: String,
    pub working_directory: String,
    pub shell: ProcessIdentity,
    pub process_session_id: u32,
    pub process_group_id: u32,
    pub tty_device: u64,
    pub created_at_milliseconds: u64,
    pub renderer_attached: bool,
    pub status: SessionStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum SessionStatus {
    Running,
    Exited { status: Option<i32> },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "operation", content = "payload")]
pub enum ControlRequest {
    ListSessions,
    GetSession {
        session_id: SessionId,
    },
    CreateSession(Box<CreateSessionRequest>),
    EndSession {
        session_id: SessionId,
    },
    EndSessionsByOwner {
        owner: SessionOwner,
    },
    EndAllSessions,
    SetWorkspacePlacement {
        session_id: SessionId,
        placement: Option<WorkspacePlacement>,
    },
    Ping,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "result", content = "payload")]
pub enum ControlResponse {
    Sessions(Vec<SessionDescriptor>),
    Session(Option<SessionDescriptor>),
    Created(SessionDescriptor),
    Acknowledged,
    DuplicateOwnerConflict,
    Error { code: String, message: String },
    Pong,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachRequest {
    pub session_id: SessionId,
    pub attachment_generation: u64,
    pub size: WindowSize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attached {
    pub session: SessionDescriptor,
    pub attachment_generation: u64,
    pub replay_generation: u64,
    pub next_output_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resize {
    pub attachment_generation: u64,
    pub resize_generation: u64,
    pub size: WindowSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExited {
    pub status: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageValidationError {
    InvalidField(&'static str),
    DuplicateEnvironmentKey(String),
    StructuredFrameTooLarge(usize),
    DuplicateOwnerInvariant,
}

impl Display for MessageValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField(field) => write!(formatter, "invalid {field}"),
            Self::DuplicateEnvironmentKey(key) => {
                write!(formatter, "duplicate environment key: {key}")
            }
            Self::StructuredFrameTooLarge(length) => {
                write!(formatter, "structured frame is too large: {length}")
            }
            Self::DuplicateOwnerInvariant => formatter.write_str("duplicate owner invariant"),
        }
    }
}

impl std::error::Error for MessageValidationError {}

fn invalid(field: &'static str) -> MessageValidationError {
    MessageValidationError::InvalidField(field)
}

fn validate_id(field: &'static str, value: &str) -> Result<(), MessageValidationError> {
    if value.is_empty() || value.len() > MAX_OWNER_ID_BYTES || value.contains('\0') {
        return Err(invalid(field));
    }
    Ok(())
}

fn validate_absolute_path(field: &'static str, value: &str) -> Result<(), MessageValidationError> {
    validate_value(field, value)?;
    if !Path::new(value).is_absolute() {
        return Err(invalid(field));
    }
    Ok(())
}

fn validate_value(field: &'static str, value: &str) -> Result<(), MessageValidationError> {
    if value.len() > MAX_VALUE_BYTES || value.contains('\0') {
        return Err(invalid(field));
    }
    Ok(())
}

fn validate_environment_key(key: &str) -> Result<(), MessageValidationError> {
    if key.is_empty() || key.len() > MAX_ENVIRONMENT_KEY_BYTES || key.contains('\0') {
        return Err(invalid("environment.key"));
    }
    let mut bytes = key.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        || bytes.any(|byte| byte != b'_' && !byte.is_ascii_alphanumeric())
    {
        return Err(invalid("environment.key"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CreateSessionRequest {
        CreateSessionRequest {
            session_id: SessionId::parse("123E4567-E89B-12D3-A456-426614174000").unwrap(),
            owner: SessionOwner {
                project_id: "project".into(),
                worktree_id: "worktree".into(),
                original_tab_id: "tab".into(),
            },
            placement: Some(WorkspacePlacement {
                project_id: "project".into(),
                worktree_id: "worktree".into(),
                tab_id: "tab".into(),
                area_id: "area".into(),
            }),
            working_directory: "/workspace".into(),
            initial_size: WindowSize::new(120, 40),
            shell_executable: "/bin/zsh".into(),
            argv: vec!["-l".into()],
            startup_command: Some("printf ready".into()),
            keep_shell_open: true,
            environment: vec![EnvironmentEntry {
                key: "PATH".into(),
                value: "/bin".into(),
            }],
            ghostty_resources: "/Applications/Muxy.app/Contents/Resources/ghostty".into(),
            terminfo: "/Applications/Muxy.app/Contents/Resources/terminfo".into(),
            terminal_type: "xterm-ghostty".into(),
            color_terminal: "truecolor".into(),
            title: "Terminal".into(),
        }
    }

    #[test]
    fn protocol_negotiates_minor_and_rejects_major_mismatch() {
        assert_eq!(
            ProtocolVersion::CURRENT
                .negotiate(ProtocolVersion { major: 1, minor: 9 })
                .unwrap(),
            ProtocolVersion::CURRENT
        );
        assert_eq!(
            ProtocolVersion { major: 1, minor: 9 }
                .negotiate(ProtocolVersion::CURRENT)
                .unwrap(),
            ProtocolVersion::CURRENT
        );
        assert!(
            ProtocolVersion::CURRENT
                .negotiate(ProtocolVersion { major: 2, minor: 0 })
                .is_err()
        );
    }

    #[test]
    fn complete_create_request_round_trips_and_validates() {
        let request = request();
        request.validate().unwrap();
        let encoded = serde_json::to_vec(&request).unwrap();
        assert!(encoded.len() < MAX_STRUCTURED_FRAME_BYTES);
        assert_eq!(
            serde_json::from_slice::<CreateSessionRequest>(&encoded).unwrap(),
            request
        );
    }

    #[test]
    fn create_request_rejects_invalid_paths_sizes_environment_and_bounds() {
        let mut value = request();
        value.working_directory = "relative".into();
        assert_eq!(value.validate(), Err(invalid("workingDirectory")));

        let mut value = request();
        value.initial_size = WindowSize::new(1, 1);
        assert_eq!(value.validate(), Err(invalid("initialSize")));

        let mut value = request();
        value.environment.push(EnvironmentEntry {
            key: "PATH".into(),
            value: "/usr/bin".into(),
        });
        assert!(matches!(
            value.validate(),
            Err(MessageValidationError::DuplicateEnvironmentKey(key)) if key == "PATH"
        ));

        let mut value = request();
        value.environment[0].key = "BAD-NAME".into();
        assert_eq!(value.validate(), Err(invalid("environment.key")));

        let mut value = request();
        value.argv = vec!["x".into(); MAX_ARGV_ENTRIES + 1];
        assert_eq!(value.validate(), Err(invalid("argv")));

        let mut value = request();
        value.title = "x".repeat(MAX_VALUE_BYTES + 1);
        assert_eq!(value.validate(), Err(invalid("title")));

        let mut value = request();
        value.shell_executable = "/bin/zsh\0other".into();
        assert_eq!(value.validate(), Err(invalid("shellExecutable")));
    }

    #[test]
    fn owner_idempotency_requires_exact_owner_and_launch_contract() {
        let held = request();
        let mut same = held.clone();
        same.session_id = SessionId::parse("223E4567-E89B-12D3-A456-426614174000").unwrap();
        assert_eq!(
            resolve_create_session([&held], &same).unwrap(),
            CreateSessionResolution::Existing(held.session_id)
        );

        let mut conflict = same.clone();
        conflict.argv.push("--different".into());
        assert_eq!(
            resolve_create_session([&held], &conflict).unwrap(),
            CreateSessionResolution::DuplicateOwnerConflict
        );

        let mut other_owner = same;
        other_owner.owner.original_tab_id = "other".into();
        assert_eq!(
            resolve_create_session([&held], &other_owner).unwrap(),
            CreateSessionResolution::Create
        );
        assert_eq!(
            resolve_create_session([&held, &held], &conflict),
            Err(MessageValidationError::DuplicateOwnerInvariant)
        );
    }
}
