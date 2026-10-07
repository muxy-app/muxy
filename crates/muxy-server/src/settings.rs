use std::path::PathBuf;

const DEFAULT_HISTORY_BUDGET_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerSettings {
    pub default_shell: Option<PathBuf>,
    pub sandbox: Option<muxy_protocol::SandboxSettings>,
    pub shell_integration: bool,
    pub history_budget_bytes: u64,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            default_shell: None,
            sandbox: None,
            shell_integration: true,
            history_budget_bytes: DEFAULT_HISTORY_BUDGET_BYTES,
        }
    }
}

impl ServerSettings {
    pub fn document(&self) -> muxy_protocol::ServerSettingsDoc {
        use std::os::unix::ffi::OsStrExt;
        muxy_protocol::ServerSettingsDoc {
            sandbox: self.sandbox.clone(),
            default_shell: self
                .default_shell
                .as_ref()
                .map(|path| muxy_protocol::ServerPath(path.as_os_str().as_bytes().to_vec())),
            history_budget_bytes: self.history_budget_bytes,
            shell_integration: self.shell_integration,
        }
    }

    pub fn validate(&self) -> Result<(), crate::ServerError> {
        use muxy_protocol::ErrorCode;
        use std::os::unix::fs::PermissionsExt;
        self.document().validate().map_err(|code| {
            crate::ServerError::new(
                code,
                "shell must be an absolute path; history budget must not exceed 64 GiB",
            )
        })?;
        if let Some(shell) = &self.default_shell {
            let metadata = std::fs::metadata(shell).map_err(|error| {
                crate::ServerError::new(ErrorCode::BadPath, format!("{}: {error}", shell.display()))
            })?;
            if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                return Err(crate::ServerError::new(
                    ErrorCode::BadPath,
                    "shell must be an executable file",
                ));
            }
        }
        Ok(())
    }
}

impl From<muxy_protocol::ServerSettingsDoc> for ServerSettings {
    fn from(document: muxy_protocol::ServerSettingsDoc) -> Self {
        use std::os::unix::ffi::OsStringExt;
        Self {
            sandbox: document.sandbox,
            default_shell: document
                .default_shell
                .map(|path| std::ffi::OsString::from_vec(path.0).into()),
            history_budget_bytes: document.history_budget_bytes,
            shell_integration: document.shell_integration,
        }
    }
}

type PersistSettings = dyn Fn(&ServerSettings) -> std::io::Result<()> + Send + Sync;

pub(crate) struct Persistence(pub(crate) Box<PersistSettings>);

impl std::fmt::Debug for Persistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Persistence").finish_non_exhaustive()
    }
}
