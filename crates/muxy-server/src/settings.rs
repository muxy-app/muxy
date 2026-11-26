use std::path::{Path, PathBuf};

use muxy_protocol::ErrorCode;

const DEFAULT_HISTORY_BUDGET_BYTES: u64 = 16 * 1024 * 1024;

const DEFAULT_RESUME_ALLOWLIST: [&str; 9] = [
    "claude",
    "codex",
    "copilot",
    "cursor-agent",
    "droid",
    "nvim",
    "omp",
    "opencode",
    "xal",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerSettings {
    pub default_shell: Option<PathBuf>,
    pub shell_integration: bool,
    pub history_budget_bytes: u64,
    pub resume_programs: bool,
    pub resume_allowlist: Vec<String>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            default_shell: None,
            shell_integration: true,
            history_budget_bytes: DEFAULT_HISTORY_BUDGET_BYTES,
            resume_programs: true,
            resume_allowlist: DEFAULT_RESUME_ALLOWLIST.map(String::from).to_vec(),
        }
    }
}

impl ServerSettings {
    pub fn document(&self) -> muxy_protocol::ServerSettingsDoc {
        muxy_protocol::ServerSettingsDoc {
            default_shell: self
                .default_shell
                .as_ref()
                .map(muxy_protocol::ServerPath::from),
            history_budget_bytes: self.history_budget_bytes,
            shell_integration: self.shell_integration,
            resume_programs: cfg!(unix).then_some(self.resume_programs),
            resume_allowlist: cfg!(unix).then(|| self.resume_allowlist.clone()),
        }
    }

    #[must_use]
    pub fn with_document(&self, document: muxy_protocol::ServerSettingsDoc) -> Self {
        Self {
            default_shell: document.default_shell.map(Into::into),
            history_budget_bytes: document.history_budget_bytes,
            shell_integration: document.shell_integration,
            resume_programs: document.resume_programs.unwrap_or(self.resume_programs),
            resume_allowlist: document
                .resume_allowlist
                .unwrap_or_else(|| self.resume_allowlist.clone()),
        }
    }

    pub(crate) fn allows_resume(&self, program: &str) -> bool {
        let name = program.rsplit('/').next().unwrap_or(program);
        self.resume_programs && self.resume_allowlist.iter().any(|allowed| allowed == name)
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
        Ok(!metadata.is_dir()
            && shell.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
            }))
    }
}

type PersistSettings = dyn Fn(&ServerSettings) -> std::io::Result<()> + Send + Sync;

pub(crate) struct Persistence(pub(crate) Box<PersistSettings>);

impl std::fmt::Debug for Persistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Persistence").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_apps_settings_write_keeps_the_resume_settings() {
        let current = ServerSettings {
            resume_programs: false,
            resume_allowlist: vec!["nvim".into()],
            ..ServerSettings::default()
        };
        let older = muxy_protocol::ServerSettingsDoc {
            shell_integration: false,
            resume_programs: None,
            resume_allowlist: None,
            ..current.document()
        };
        let written = current.with_document(older);
        assert!(!written.shell_integration);
        assert!(!written.resume_programs);
        assert_eq!(written.resume_allowlist, ["nvim"]);
        let newer = muxy_protocol::ServerSettingsDoc {
            resume_programs: Some(true),
            resume_allowlist: Some(vec!["claude".into()]),
            ..current.document()
        };
        let written = current.with_document(newer);
        assert!(written.allows_resume("/usr/local/bin/claude"));
        assert!(!written.allows_resume("nvim"));
    }
}
