use std::path::{Path, PathBuf};

use muxy_protocol::{ErrorCode, ServerPath};

const DEFAULT_HISTORY_BUDGET_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerSettings {
    pub default_shell: Option<PathBuf>,
    pub shell_integration: bool,
    pub history_budget_bytes: u64,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            default_shell: None,
            shell_integration: true,
            history_budget_bytes: DEFAULT_HISTORY_BUDGET_BYTES,
        }
    }
}

impl ServerSettings {
    pub fn document(&self) -> muxy_protocol::ServerSettingsDoc {
        muxy_protocol::ServerSettingsDoc {
            default_shell: self
                .default_shell
                .as_deref()
                .and_then(|path| ServerPath::from_path(path).ok()),
            history_budget_bytes: self.history_budget_bytes,
            shell_integration: self.shell_integration,
        }
    }

    pub fn validate(&self) -> Result<(), crate::ServerError> {
        self.document().validate().map_err(|code| {
            crate::ServerError::new(
                code,
                "shell must be an absolute path; history budget must not exceed 64 GiB",
            )
        })?;
        if let Some(shell) = &self.default_shell {
            if !shell.is_absolute() {
                return Err(crate::ServerError::new(
                    ErrorCode::BadPath,
                    "shell must be an absolute path",
                ));
            }
            if !runnable(shell)? {
                return Err(crate::ServerError::new(
                    ErrorCode::BadPath,
                    if cfg!(windows) {
                        "shell must be a program file, such as an .exe"
                    } else {
                        "shell must be an executable file"
                    },
                ));
            }
        }
        Ok(())
    }
}

/// Whether `shell` is a file the system can run: executable on Unix, and on
/// Windows a file whose extension is in `PATHEXT`.
fn runnable(shell: &Path) -> Result<bool, crate::ServerError> {
    let missing = |error: std::io::Error| {
        crate::ServerError::new(ErrorCode::BadPath, format!("{}: {error}", shell.display()))
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(shell).map_err(missing)?;
        Ok(metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(windows)]
    {
        let metadata = std::fs::symlink_metadata(shell).map_err(missing)?;
        Ok(!metadata.is_dir() && crate::exec::has_program_extension(shell))
    }
}

impl TryFrom<muxy_protocol::ServerSettingsDoc> for ServerSettings {
    type Error = crate::ServerError;

    /// Refuses a shell path this computer can't name, such as one that isn't
    /// UTF-8 on Windows.
    fn try_from(document: muxy_protocol::ServerSettingsDoc) -> Result<Self, Self::Error> {
        Ok(Self {
            default_shell: document
                .default_shell
                .map(|path| path.to_path_buf())
                .transpose()
                .map_err(|code| crate::ServerError::new(code, "shell path is not valid here"))?,
            history_budget_bytes: document.history_budget_bytes,
            shell_integration: document.shell_integration,
        })
    }
}

type PersistSettings = dyn Fn(&ServerSettings) -> std::io::Result<()> + Send + Sync;

pub(crate) struct Persistence(pub(crate) Box<PersistSettings>);

impl std::fmt::Debug for Persistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Persistence").finish_non_exhaustive()
    }
}
