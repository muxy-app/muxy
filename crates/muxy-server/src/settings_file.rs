use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use muxy_core::files::private_file;

use muxy_server::ServerSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct SettingsFile {
    default_shell: Option<PathBuf>,
    history_budget_bytes: u64,
    shell_integration: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
struct ResumeFile {
    enabled: bool,
    programs: Vec<String>,
}

impl Default for ResumeFile {
    fn default() -> Self {
        let defaults = ServerSettings::default();
        Self {
            enabled: defaults.resume_programs,
            programs: defaults.resume_allowlist,
        }
    }
}

fn resume_path(path: &Path) -> PathBuf {
    path.with_file_name("program-resume.toml")
}

fn load_resume(path: &Path) -> io::Result<ResumeFile> {
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str(&contents).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ResumeFile::default()),
        Err(error) => Err(error),
    }
}

impl Default for SettingsFile {
    fn default() -> Self {
        let defaults = ServerSettings::default();
        Self {
            default_shell: defaults.default_shell,
            shell_integration: defaults.shell_integration,
            history_budget_bytes: defaults.history_budget_bytes,
        }
    }
}

pub(crate) fn load(path: &Path) -> io::Result<ServerSettings> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let defaults =
                toml::to_string_pretty(&SettingsFile::default()).map_err(io::Error::other)?;
            let mut file = match private_file().write(true).create_new(true).open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return load(path),
                Err(error) => return Err(error),
            };
            file.write_all(defaults.as_bytes())?;
            defaults
        }
        Err(error) => return Err(error),
    };
    let settings: SettingsFile = toml::from_str(&contents).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {error}", path.display()),
        )
    })?;
    let resume = load_resume(&resume_path(path))?;
    let settings = ServerSettings {
        default_shell: settings.default_shell,
        shell_integration: settings.shell_integration,
        history_budget_bytes: settings.history_budget_bytes,
        resume_programs: resume.enabled,
        resume_allowlist: resume.programs,
    };
    settings.document().validate().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid server settings: {error:?}"),
        )
    })?;
    Ok(settings)
}

pub(crate) fn save(path: &Path, settings: &ServerSettings) -> io::Result<()> {
    let source = toml::to_string_pretty(&SettingsFile {
        default_shell: settings.default_shell.clone(),
        history_budget_bytes: settings.history_budget_bytes,
        shell_integration: settings.shell_integration,
    })
    .map_err(io::Error::other)?;
    let resume = toml::to_string_pretty(&ResumeFile {
        enabled: settings.resume_programs,
        programs: settings.resume_allowlist.clone(),
    })
    .map_err(io::Error::other)?;
    let sidecar = resume_path(path);
    let previous = match fs::read(&sidecar) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    muxy_core::fs::write_atomic(&sidecar, resume.as_bytes(), 0o600)?;
    if let Err(error) = muxy_core::fs::write_atomic(path, source.as_bytes(), 0o600) {
        let _ = match previous {
            Some(bytes) => {
                muxy_core::fs::write_atomic(&sidecar, &bytes, 0o600).map_err(io::Error::other)
            }
            None => fs::remove_file(&sidecar),
        };
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OlderSettingsFile {
        default_shell: Option<PathBuf>,
        history_budget_bytes: u64,
        shell_integration: bool,
    }

    #[test]
    fn resume_settings_live_beside_the_settings_an_older_server_reads() -> io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-settings-{}",
            muxy_protocol::OperationId::new()
        ));
        fs::create_dir_all(&directory)?;
        let path = directory.join("server.toml");
        let settings = ServerSettings {
            resume_programs: false,
            resume_allowlist: vec!["nvim".into()],
            ..ServerSettings::default()
        };
        save(&path, &settings)?;
        let older: Result<OlderSettingsFile, _> = toml::from_str(&fs::read_to_string(&path)?);
        let loaded = load(&path);
        fs::remove_file(resume_path(&path))?;
        let without = load(&path);
        fs::remove_dir_all(&directory)?;
        let older = older.map_err(io::Error::other)?;
        assert_eq!(older.history_budget_bytes, settings.history_budget_bytes);
        assert_eq!(older.shell_integration, settings.shell_integration);
        assert_eq!(older.default_shell, settings.default_shell);
        assert_eq!(loaded?, settings);
        assert_eq!(
            without?.resume_allowlist,
            ServerSettings::default().resume_allowlist
        );
        Ok(())
    }

    #[test]
    fn a_failed_settings_write_leaves_the_resume_settings_as_they_were() -> io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-settings-{}",
            muxy_protocol::OperationId::new()
        ));
        fs::create_dir_all(directory.join("server.toml"))?;
        let path = directory.join("server.toml");
        fs::write(
            resume_path(&path),
            "enabled = true\nprograms = [\"claude\"]\n",
        )?;
        let failed = save(
            &path,
            &ServerSettings {
                resume_programs: false,
                ..ServerSettings::default()
            },
        );
        let kept = fs::read_to_string(resume_path(&path))?;
        fs::remove_dir_all(&directory)?;
        assert!(failed.is_err());
        assert_eq!(kept, "enabled = true\nprograms = [\"claude\"]\n");
        Ok(())
    }
}
