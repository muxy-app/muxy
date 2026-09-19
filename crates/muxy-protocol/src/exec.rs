use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ProjectId, ServerPath};

pub const MAX_EXEC_OUTPUT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecRequest {
    pub job: u64,
    pub project: ProjectId,
    pub argv: Vec<String>,
    pub shell: Option<String>,
    pub cwd: Option<ServerPath>,
    pub env: std::collections::BTreeMap<String, String>,
    pub stdin: Vec<u8>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub truncated: bool,
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
