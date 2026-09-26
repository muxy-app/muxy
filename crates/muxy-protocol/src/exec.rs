use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ProjectId, ServerPath};

pub const MAX_EXEC_OUTPUT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ExecRequest {
    #[n(0)]
    pub job: u64,
    #[n(1)]
    pub project: ProjectId,
    #[n(2)]
    pub argv: Vec<String>,
    #[n(3)]
    pub shell: Option<String>,
    #[n(4)]
    pub cwd: Option<ServerPath>,
    #[n(5)]
    pub env: std::collections::BTreeMap<String, String>,
    #[n(6)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub stdin: Vec<u8>,
    #[n(7)]
    pub timeout_ms: u32,
}

impl ExecRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.job == 0
            || self.argv.first().is_some_and(String::is_empty)
            || self.env.len() > 128
            || self.env.iter().any(|(key, value)| {
                key.is_empty() || key.contains(['=', '\0']) || value.contains('\0')
            })
            || self
                .env
                .iter()
                .map(|(key, value)| key.len() + value.len())
                .sum::<usize>()
                > 64 * 1024
            || self.argv.len() > 4096
            || self.stdin.len() > MAX_EXEC_OUTPUT
            || self.timeout_ms == 0
            || self.timeout_ms > 300_000
            || self.argv.is_empty() == self.shell.is_none()
            || self.argv.iter().any(|s| s.contains('\0'))
            || self.argv.iter().map(String::len).sum::<usize>() > 1024 * 1024
            || self
                .shell
                .as_ref()
                .is_some_and(|s| s.len() > 1024 * 1024 || s.contains('\0'))
        {
            return Err(ErrorCode::BadRequest);
        }
        if let Some(path) = &self.cwd {
            crate::validate_path(path)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ExecResult {
    #[n(0)]
    pub stdout: String,
    #[n(1)]
    pub stderr: String,
    #[n(2)]
    pub exit_code: i32,
    #[n(3)]
    pub timed_out: bool,
    #[n(4)]
    pub truncated: bool,
    #[n(5)]
    pub cancelled: bool,
}

impl ExecResult {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.stdout.len() > MAX_EXEC_OUTPUT || self.stderr.len() > MAX_EXEC_OUTPUT {
            Err(ErrorCode::BadRequest)
        } else {
            Ok(())
        }
    }
}
