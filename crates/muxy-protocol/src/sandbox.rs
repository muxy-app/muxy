use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ServerPath, wire::cbor::open_enum};

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SandboxNetwork {
        #[default]
        Blocked = 0,
        Domains = 1,
        Unrestricted = 2,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxPolicy {
    #[n(0)]
    pub network: SandboxNetwork,
    #[n(1)]
    pub domains: Vec<String>,
    #[n(2)]
    pub read_paths: Vec<ServerPath>,
    #[n(3)]
    pub environment: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SandboxSpec {
    #[n(0)]
    pub workspace: ServerPath,
    #[n(1)]
    pub policy: SandboxPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SandboxInfo {
    #[n(0)]
    pub spec: SandboxSpec,
    #[n(1)]
    pub backend_version: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxSettings {
    #[n(0)]
    pub executable: Option<ServerPath>,
    #[n(1)]
    pub policy: SandboxPolicy,
}

impl SandboxPolicy {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if matches!(self.network, SandboxNetwork::Unrecognized(_))
            || self.domains.len() > 128
            || self.read_paths.len() > 128
            || self.environment.len() > 64
            || (self.network == SandboxNetwork::Domains && self.domains.is_empty())
        {
            return Err(ErrorCode::BadRequest);
        }
        for domain in &self.domains {
            if domain.len() > 253
                || !domain.contains('.')
                || domain.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
                || domain.parse::<std::net::IpAddr>().is_ok()
            {
                return Err(ErrorCode::BadRequest);
            }
        }
        for path in &self.read_paths {
            absolute(path)?;
        }
        for name in &self.environment {
            if !allowed_environment_name(name) {
                return Err(ErrorCode::BadRequest);
            }
        }
        Ok(())
    }
}

pub fn allowed_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        && name.as_bytes()[0].is_ascii_uppercase()
        && !["DYLD_", "LD_", "NONO_", "MUXY_", "XDG_", "GIT_", "BASH_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        && !matches!(
            name,
            "HOME"
                | "PATH"
                | "SHELL"
                | "ENV"
                | "ZDOTDIR"
                | "TMPDIR"
                | "TMP"
                | "TEMP"
                | "NODE_OPTIONS"
                | "PYTHONPATH"
                | "PYTHONHOME"
                | "RUBYOPT"
                | "PERL5OPT"
                | "HTTP_PROXY"
                | "HTTPS_PROXY"
                | "ALL_PROXY"
                | "NO_PROXY"
                | "SSH_AUTH_SOCK"
        )
}

impl SandboxSpec {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        absolute(&self.workspace)?;
        self.policy.validate()
    }
}

impl SandboxSettings {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if let Some(path) = &self.executable {
            absolute(path)?;
        }
        self.policy.validate()
    }
}

fn absolute(path: &ServerPath) -> Result<(), ErrorCode> {
    crate::validate_path(path)?;
    if !path.0.starts_with(b"/") || path.0 == b"/" || path.0.contains(&0) {
        return Err(ErrorCode::BadPath);
    }
    Ok(())
}
