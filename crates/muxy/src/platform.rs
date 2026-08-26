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

#[cfg(target_os = "windows")]
fn reveal_command(path: &Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer.exe");
    command.arg(format!("/select,{}", path.display()));
    command
}
