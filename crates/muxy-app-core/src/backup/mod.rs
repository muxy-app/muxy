mod legacy;
mod projects;

pub use legacy::{import_settings, merge_legacy_files};
pub use projects::{import_projects, remap_settings};

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::settings::{Settings, TerminalSettings};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: Vec<String>,
}

pub fn settings_source(settings: &Settings) -> Result<String> {
    settings.validate()?;
    let mut value = toml::Value::try_from(settings)?;
    value["keymap"] = toml::Value::try_from(settings.keymap.overrides())?;
    Ok(toml::to_string_pretty(&value)?)
}

pub fn validate_configuration(directory: &Path) -> Result<()> {
    let settings = Settings::load(&directory.join("settings.toml"))?;
    let terminal = TerminalSettings::load_with_seed(&directory.join("ghostty.conf"), None)?;
    settings.validate_command_shortcuts(&terminal)?;
    let server = directory.join("server.toml");
    if server.exists() {
        let server: ServerSettings = toml::from_str(&std::fs::read_to_string(server)?)?;
        server
            .document()
            .validate()
            .map_err(|error| format!("Invalid server settings: {error:?}"))?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ServerSettings {
    default_shell: Option<PathBuf>,
    history_budget_bytes: u64,
    shell_integration: bool,
    sandbox: Option<muxy_protocol::SandboxSettings>,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            default_shell: None,
            history_budget_bytes: 16 * 1024 * 1024,
            shell_integration: true,
            sandbox: None,
        }
    }
}

impl ServerSettings {
    fn document(&self) -> muxy_protocol::ServerSettingsDoc {
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
}
