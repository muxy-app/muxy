//! Finding the user's shell and the programs the server runs.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
const FALLBACK_SHELL: &str = "/bin/zsh";
#[cfg(target_os = "linux")]
const FALLBACK_SHELL: &str = "/bin/sh";

/// `$SHELL` when it names an executable file, otherwise the system's default shell.
pub(crate) fn user_shell() -> PathBuf {
    shell_or_fallback(std::env::var_os("SHELL"))
}

fn shell_or_fallback(inherited: Option<OsString>) -> PathBuf {
    inherited
        .map(PathBuf::from)
        .filter(|shell| shell.is_absolute() && is_executable(shell))
        .unwrap_or_else(|| PathBuf::from(FALLBACK_SHELL))
}

/// The first executable `name` in `directories`, skipping empty entries.
/// Stops at the first match, so later directories are only computed when needed.
pub(crate) fn find_executable(
    name: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    directories
        .into_iter()
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join(name))
        .find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    path.is_file() && rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_ok()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "muxy-process-{}",
                muxy_protocol::OperationId::new()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, name: &str, mode: u32) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, "#!/bin/sh\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            path
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn shell_falls_back_unless_it_names_an_executable_file() {
        let directory = TestDirectory::new();
        let valid = directory.file("valid", 0o755);
        let plain = directory.file("plain", 0o644);
        let fallback = PathBuf::from(FALLBACK_SHELL);

        assert_eq!(shell_or_fallback(None), fallback);
        assert_eq!(shell_or_fallback(Some("".into())), fallback);
        assert_eq!(shell_or_fallback(Some("sh".into())), fallback);
        assert_eq!(shell_or_fallback(Some(plain.into())), fallback);
        assert_eq!(
            shell_or_fallback(Some(directory.0.clone().into())),
            fallback
        );
        assert_eq!(shell_or_fallback(Some(valid.clone().into())), valid);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_falls_back_to_sh() {
        assert_eq!(shell_or_fallback(None), Path::new("/bin/sh"));
    }

    #[test]
    fn finds_the_first_executable_file() {
        let first = TestDirectory::new();
        let second = TestDirectory::new();
        first.file("tool", 0o644);
        fs::create_dir(first.0.join("folder")).unwrap();
        let tool = second.file("tool", 0o755);
        let folder = second.file("folder", 0o755);
        let directories = || [PathBuf::new(), first.0.clone(), second.0.clone()];

        assert_eq!(find_executable("tool", directories()), Some(tool));
        assert_eq!(find_executable("folder", directories()), Some(folder));
        assert_eq!(find_executable("missing", directories()), None);
    }
}
