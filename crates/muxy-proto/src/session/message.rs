use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SessionCodecError;

pub const MAX_FRAME_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const MAX_FIELD_BYTES: usize = 256;
pub const MAX_TITLE_BYTES: usize = 1024;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_STARTUP_COMMAND_BYTES: usize = 64 * 1024;
pub const MAX_ENVIRONMENT_ENTRIES: usize = 128;
pub const MAX_ENVIRONMENT_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum MessageKind {
    AttachCreateOrAttach = 1,
    AttachExisting = 2,
    Attached = 3,
    Replay = 4,
    Output = 5,
    Input = 6,
    Resize = 7,
    Exited = 8,
    ProtocolError = 9,
    Query = 10,
    QueryResult = 11,
    Recover = 12,
    Recovered = 13,
    CommandActivity = 14,
    CommandActivityResult = 15,
    TerminateOne = 16,
    TerminateAll = 17,
    TerminateResult = 18,
}

impl MessageKind {
    pub fn from_raw(raw: u16) -> Option<Self> {
        Some(match raw {
            1 => Self::AttachCreateOrAttach,
            2 => Self::AttachExisting,
            3 => Self::Attached,
            4 => Self::Replay,
            5 => Self::Output,
            6 => Self::Input,
            7 => Self::Resize,
            8 => Self::Exited,
            9 => Self::ProtocolError,
            10 => Self::Query,
            11 => Self::QueryResult,
            12 => Self::Recover,
            13 => Self::Recovered,
            14 => Self::CommandActivity,
            15 => Self::CommandActivityResult,
            16 => Self::TerminateOne,
            17 => Self::TerminateAll,
            18 => Self::TerminateResult,
            _ => return None,
        })
    }

    pub const fn is_raw(self) -> bool {
        matches!(self, Self::Replay | Self::Output | Self::Input)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerMetadata {
    pub project_id: String,
    pub worktree_id: Option<String>,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchSpecification {
    pub shell: String,
    pub resources_directory: String,
    pub working_directory: String,
    pub startup_command: Option<String>,
    pub environment: Vec<EnvironmentEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttachRequest {
    pub session_id: String,
    pub owner: OwnerMetadata,
    pub launch: LaunchSpecification,
    pub size: Resize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttachExisting {
    pub session_id: String,
    pub size: Resize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attached {
    pub created: bool,
    pub descriptor: SessionDescriptor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resize {
    pub columns: u16,
    pub rows: u16,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExitStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolFailure {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionQuery {
    pub session_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandActivity {
    Idle,
    Running,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionDescriptor {
    pub session_id: String,
    pub owner: OwnerMetadata,
    pub working_directory: String,
    pub shell_pid: u32,
    pub tty_device: u64,
    pub command_activity: CommandActivity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "descriptor", rename_all = "camelCase")]
pub enum QueryResult {
    Found(SessionDescriptor),
    Missing,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandActivityQuery {
    pub session_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandActivityResult {
    pub session_id: String,
    pub activity: CommandActivity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminationOutcome {
    Terminated,
    NoSessions,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerminateResult {
    pub outcome: TerminationOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionMessage {
    AttachCreateOrAttach(AttachRequest),
    AttachExisting(AttachExisting),
    Attached(Attached),
    Replay(Vec<u8>),
    Output(Vec<u8>),
    Input(Vec<u8>),
    Resize(Resize),
    Exited(ExitStatus),
    ProtocolError(ProtocolFailure),
    Query(SessionQuery),
    QueryResult(QueryResult),
    Recover(Empty),
    Recovered(Vec<SessionDescriptor>),
    CommandActivity(CommandActivityQuery),
    CommandActivityResult(CommandActivityResult),
    TerminateOne(SessionQuery),
    TerminateAll(Empty),
    TerminateResult(TerminateResult),
}

impl SessionMessage {
    pub const fn kind(&self) -> MessageKind {
        match self {
            Self::AttachCreateOrAttach(_) => MessageKind::AttachCreateOrAttach,
            Self::AttachExisting(_) => MessageKind::AttachExisting,
            Self::Attached(_) => MessageKind::Attached,
            Self::Replay(_) => MessageKind::Replay,
            Self::Output(_) => MessageKind::Output,
            Self::Input(_) => MessageKind::Input,
            Self::Resize(_) => MessageKind::Resize,
            Self::Exited(_) => MessageKind::Exited,
            Self::ProtocolError(_) => MessageKind::ProtocolError,
            Self::Query(_) => MessageKind::Query,
            Self::QueryResult(_) => MessageKind::QueryResult,
            Self::Recover(_) => MessageKind::Recover,
            Self::Recovered(_) => MessageKind::Recovered,
            Self::CommandActivity(_) => MessageKind::CommandActivity,
            Self::CommandActivityResult(_) => MessageKind::CommandActivityResult,
            Self::TerminateOne(_) => MessageKind::TerminateOne,
            Self::TerminateAll(_) => MessageKind::TerminateAll,
            Self::TerminateResult(_) => MessageKind::TerminateResult,
        }
    }

    pub fn validate(&self) -> Result<(), SessionCodecError> {
        match self {
            Self::AttachCreateOrAttach(request) => request.validate(),
            Self::AttachExisting(request) => {
                validate_uuid("session_id", &request.session_id)?;
                request.size.validate()
            }
            Self::Attached(attached) => attached.descriptor.validate(),
            Self::Replay(bytes) | Self::Output(bytes) | Self::Input(bytes) => {
                validate_payload_size(bytes.len())
            }
            Self::Resize(size) => size.validate(),
            Self::Exited(status) => status.validate(),
            Self::ProtocolError(error) => {
                validate_string("protocol_error.code", &error.code, MAX_FIELD_BYTES)?;
                validate_string("protocol_error.message", &error.message, MAX_PATH_BYTES)
            }
            Self::Query(query) | Self::TerminateOne(query) => {
                validate_uuid("session_id", &query.session_id)
            }
            Self::QueryResult(QueryResult::Found(descriptor)) => descriptor.validate(),
            Self::QueryResult(QueryResult::Missing)
            | Self::Recover(_)
            | Self::TerminateAll(_)
            | Self::TerminateResult(_) => Ok(()),
            Self::Recovered(descriptors) => {
                if descriptors.len() > MAX_ENVIRONMENT_ENTRIES {
                    return Err(SessionCodecError::TooManyItems {
                        field: "recovered",
                        maximum: MAX_ENVIRONMENT_ENTRIES,
                        actual: descriptors.len(),
                    });
                }
                descriptors.iter().try_for_each(SessionDescriptor::validate)
            }
            Self::CommandActivity(query) => validate_uuid("session_id", &query.session_id),
            Self::CommandActivityResult(result) => validate_uuid("session_id", &result.session_id),
        }
    }
}

impl AttachRequest {
    fn validate(&self) -> Result<(), SessionCodecError> {
        validate_uuid("session_id", &self.session_id)?;
        self.owner.validate()?;
        self.launch.validate()?;
        self.size.validate()
    }
}

impl OwnerMetadata {
    fn validate(&self) -> Result<(), SessionCodecError> {
        validate_uuid("project_id", &self.project_id)?;
        if let Some(worktree_id) = &self.worktree_id {
            validate_uuid("worktree_id", worktree_id)?;
        }
        validate_string("title", &self.title, MAX_TITLE_BYTES)
    }
}

impl LaunchSpecification {
    fn validate(&self) -> Result<(), SessionCodecError> {
        validate_string("shell", &self.shell, MAX_PATH_BYTES)?;
        validate_string(
            "resources_directory",
            &self.resources_directory,
            MAX_PATH_BYTES,
        )?;
        validate_string("working_directory", &self.working_directory, MAX_PATH_BYTES)?;
        if let Some(command) = &self.startup_command {
            validate_string("startup_command", command, MAX_STARTUP_COMMAND_BYTES)?;
        }
        if self.environment.len() > MAX_ENVIRONMENT_ENTRIES {
            return Err(SessionCodecError::TooManyItems {
                field: "environment",
                maximum: MAX_ENVIRONMENT_ENTRIES,
                actual: self.environment.len(),
            });
        }
        let mut total = 0usize;
        for entry in &self.environment {
            validate_string("environment.key", &entry.key, MAX_FIELD_BYTES)?;
            validate_string("environment.value", &entry.value, MAX_PATH_BYTES)?;
            total = total
                .checked_add(entry.key.len())
                .and_then(|value| value.checked_add(entry.value.len()))
                .ok_or(SessionCodecError::FieldTooLong {
                    field: "environment",
                    maximum: MAX_ENVIRONMENT_BYTES,
                    actual: usize::MAX,
                })?;
        }
        if total > MAX_ENVIRONMENT_BYTES {
            return Err(SessionCodecError::FieldTooLong {
                field: "environment",
                maximum: MAX_ENVIRONMENT_BYTES,
                actual: total,
            });
        }
        Ok(())
    }
}

impl Resize {
    fn validate(self) -> Result<(), SessionCodecError> {
        if self.columns == 0 || self.rows == 0 {
            return Err(SessionCodecError::InvalidTerminalSize);
        }
        Ok(())
    }
}

impl ExitStatus {
    fn validate(self) -> Result<(), SessionCodecError> {
        if self.code.is_some() == self.signal.is_some() {
            return Err(SessionCodecError::InvalidExitStatus);
        }
        Ok(())
    }
}

impl SessionDescriptor {
    fn validate(&self) -> Result<(), SessionCodecError> {
        validate_uuid("session_id", &self.session_id)?;
        self.owner.validate()?;
        validate_string("working_directory", &self.working_directory, MAX_PATH_BYTES)?;
        if self.shell_pid == 0 || self.tty_device == 0 {
            return Err(SessionCodecError::InvalidDescriptor);
        }
        Ok(())
    }
}

fn validate_payload_size(len: usize) -> Result<(), SessionCodecError> {
    if len > MAX_FRAME_PAYLOAD_BYTES {
        return Err(SessionCodecError::PayloadTooLarge {
            maximum: MAX_FRAME_PAYLOAD_BYTES,
            actual: len,
        });
    }
    Ok(())
}

fn validate_uuid(field: &'static str, value: &str) -> Result<(), SessionCodecError> {
    let parsed = Uuid::parse_str(value).map_err(|_| SessionCodecError::InvalidUuid {
        field,
        value: value.to_owned(),
    })?;
    let canonical = parsed.hyphenated().to_string().to_uppercase();
    if canonical != value {
        return Err(SessionCodecError::InvalidUuid {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_string(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), SessionCodecError> {
    if value.as_bytes().contains(&0) {
        return Err(SessionCodecError::NulByte { field });
    }
    if value.len() > maximum {
        return Err(SessionCodecError::FieldTooLong {
            field,
            maximum,
            actual: value.len(),
        });
    }
    Ok(())
}
