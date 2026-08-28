use muxy_api::repository::ValidatedExternalUrl;
use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum RevealError {
    MissingPath,
    Launch(io::Error),
    Failed(Option<i32>),
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    Unsupported,
}

impl fmt::Display for RevealError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath => formatter.write_str("path does not exist"),
            Self::Launch(error) => write!(formatter, "{error}"),
            Self::Failed(Some(code)) => write!(formatter, "file manager exited with status {code}"),
            Self::Failed(None) => formatter.write_str("file manager was terminated"),
            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            Self::Unsupported => {
                formatter.write_str("path reveal is not supported on this platform")
            }
        }
    }
}

#[derive(Debug)]
pub enum ExternalUrlError {
    Launch(io::Error),
    Failed(Option<i32>),
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    Unsupported,
}

impl fmt::Display for ExternalUrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(error) => write!(formatter, "{error}"),
            Self::Failed(Some(code)) => write!(formatter, "URL launcher exited with status {code}"),
            Self::Failed(None) => formatter.write_str("URL launcher was terminated"),
            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            Self::Unsupported => formatter.write_str("opening external URLs is not supported"),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub fn open_external_url(url: &ValidatedExternalUrl) -> Result<(), ExternalUrlError> {
    let status = external_url_command(url)
        .status()
        .map_err(ExternalUrlError::Launch)?;
    map_external_url_status(status.success(), status.code())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn open_external_url(_: &ValidatedExternalUrl) -> Result<(), ExternalUrlError> {
    Err(ExternalUrlError::Unsupported)
}

fn map_external_url_status(success: bool, code: Option<i32>) -> Result<(), ExternalUrlError> {
    if success {
        Ok(())
    } else {
        Err(ExternalUrlError::Failed(code))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub fn reveal_path(path: &Path) -> Result<(), RevealError> {
    if !path.exists() {
        return Err(RevealError::MissingPath);
    }
    let status = reveal_command(path).status().map_err(RevealError::Launch)?;
    if status.success() {
        Ok(())
    } else {
        Err(RevealError::Failed(status.code()))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn reveal_path(_: &Path) -> Result<(), RevealError> {
    Err(RevealError::Unsupported)
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg("-R").arg(path);
    command
}

#[cfg(target_os = "macos")]
fn external_url_command(url: &ValidatedExternalUrl) -> std::process::Command {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg(url.as_str());
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_path_reveal_rejects_a_missing_path_before_launch() {
        let missing =
            std::env::temp_dir().join(format!("muxy-missing-reveal-{}", std::process::id()));
        assert!(matches!(
            reveal_path(&missing),
            Err(RevealError::MissingPath)
        ));
    }

    #[test]
    fn external_url_contract_accepts_only_validated_https_and_maps_launcher_status() {
        let url = ValidatedExternalUrl::try_from("https://github.com/muxy/app/pull/42".to_owned())
            .unwrap();
        assert_eq!(url.as_str(), "https://github.com/muxy/app/pull/42");
        assert!(ValidatedExternalUrl::try_from("http://example.com".to_owned()).is_err());
        assert!(ValidatedExternalUrl::try_from("file:///tmp/value".to_owned()).is_err());
        assert!(map_external_url_status(true, Some(0)).is_ok());
        assert!(matches!(
            map_external_url_status(false, Some(7)),
            Err(ExternalUrlError::Failed(Some(7)))
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    #[test]
    fn external_url_launcher_is_direct_argv_without_a_shell() {
        let url = ValidatedExternalUrl::try_from("https://github.com/muxy/app/pull/42".to_owned())
            .unwrap();
        let command = external_url_command(&url);
        let program = command.get_program().to_string_lossy();
        assert!(!program.ends_with("sh"));
        assert!(!program.ends_with("cmd.exe"));
        assert!(command.get_args().any(|argument| argument == url.as_str()));
    }
}

#[cfg(target_os = "linux")]
fn reveal_command(path: &Path) -> std::process::Command {
    let target = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let mut command = std::process::Command::new("xdg-open");
    command.arg(target);
    command
}

#[cfg(target_os = "linux")]
fn external_url_command(url: &ValidatedExternalUrl) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(url.as_str());
    command
}

#[cfg(target_os = "windows")]
fn reveal_command(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer.exe");
    command.arg(format!("/select,{}", path.display()));
    command
}

#[cfg(target_os = "windows")]
fn external_url_command(url: &ValidatedExternalUrl) -> std::process::Command {
    let mut command = std::process::Command::new("rundll32.exe");
    command.arg("url.dll,FileProtocolHandler").arg(url.as_str());
    command
}
