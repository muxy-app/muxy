use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use muxy_terminal::pty::SpawnRequest;

/// `PowerShell` runs this after the user's profile, from the command line, so
/// no execution policy or quoted path applies.
const POWERSHELL: &str = include_str!("../shell/muxy.ps1");

/// Server-owned startup hooks, kept beside this server's socket.
#[derive(Clone, Debug)]
pub struct ShellIntegration {
    directory: PathBuf,
}

impl ShellIntegration {
    pub fn install(directory: &Path) -> io::Result<Self> {
        for path in [
            directory.to_path_buf(),
            directory.join("zsh"),
            directory.join("fish"),
            directory.join("fish/vendor_conf.d"),
        ] {
            private_directory(&path)?;
        }
        for (name, contents) in [
            ("zsh/.zshenv", include_str!("../shell/.zshenv")),
            ("muxy.zsh", include_str!("../shell/muxy.zsh")),
            ("muxy.bash", include_str!("../shell/muxy.bash")),
            ("muxy.ps1", POWERSHELL),
            (
                "fish/vendor_conf.d/muxy.fish",
                include_str!("../shell/muxy.fish"),
            ),
        ] {
            let path = directory.join(name);
            let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
            let mut file = muxy_core::files::private_file()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let result = file
                .write_all(contents.as_bytes())
                .and_then(|()| file.sync_all())
                .and_then(|()| muxy_core::files::replace_file(&temporary, &path));
            if result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            result?;
        }
        Ok(Self {
            directory: muxy_core::files::canonicalize(directory)?,
        })
    }

    pub(crate) fn configure(&self, request: &mut SpawnRequest, enabled: bool) {
        set_env(
            request,
            "MUXY_SHELL_INTEGRATION",
            if enabled { "1" } else { "0" }.into(),
        );
        if !enabled {
            return;
        }
        set_env(
            request,
            "MUXY_SHELL_INTEGRATION_DIR",
            self.directory.as_os_str().to_owned(),
        );
        #[cfg(windows)]
        if is_powershell(&request.program) {
            request.args.extend([
                "-NoExit".into(),
                "-EncodedCommand".into(),
                powershell_command(POWERSHELL).into(),
            ]);
            return;
        }
        match request.program.file_name().and_then(|name| name.to_str()) {
            Some("zsh") => {
                let original = get_env(request, "ZDOTDIR").cloned();
                set_env(
                    request,
                    "MUXY_ZDOTDIR_SET",
                    if original.is_some() { "1" } else { "0" }.into(),
                );
                set_env(
                    request,
                    "MUXY_ORIGINAL_ZDOTDIR",
                    original.unwrap_or_default(),
                );
                set_env(
                    request,
                    "ZDOTDIR",
                    self.directory.join("zsh").into_os_string(),
                );
            }
            Some("fish") => {
                let mut paths = self.directory.as_os_str().to_owned();
                paths.push(":");
                paths.push(
                    get_env(request, "XDG_DATA_DIRS")
                        .filter(|value| !value.is_empty())
                        .cloned()
                        .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
                );
                set_env(request, "XDG_DATA_DIRS", paths);
            }
            _ => {}
        }
    }
}

/// Makes `path` a folder only this user can open, refusing a link or a file
/// in its place.
fn private_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        match fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
            Err(_) => {}
        }
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(io::Error::other(
                "shell integration directory is not a directory",
            ));
        }
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        match fs::create_dir(path) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
            _ => {}
        }
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "shell integration directory is not a directory",
            ));
        }
        Ok(())
    }
}

/// `powershell.exe` and `pwsh.exe`, by file name.
#[cfg(windows)]
pub(crate) fn is_powershell(program: &Path) -> bool {
    program
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("pwsh") || name.eq_ignore_ascii_case("powershell")
        })
}

/// The value for `-EncodedCommand`: base64 of the UTF-16 script. A failure
/// in the script never stops the shell from starting.
#[cfg(any(windows, test))]
fn powershell_command(script: &str) -> String {
    let wrapped = format!("try {{ {script} }} catch {{}}");
    let bytes: Vec<u8> = wrapped.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

#[cfg(any(windows, test))]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | (u32::from(*byte) << (16 - 8 * index))
            });
        for index in 0..4 {
            if index <= chunk.len() {
                text.push(char::from(
                    ALPHABET[(value >> (18 - 6 * index)) as usize & 63],
                ));
            } else {
                text.push('=');
            }
        }
    }
    text
}

fn get_env<'a>(request: &'a SpawnRequest, name: &str) -> Option<&'a OsString> {
    request
        .env
        .iter()
        .rev()
        .find(|(key, _)| crate::spawn::same_name(key, name))
        .map(|(_, value)| value)
}

fn set_env(request: &mut SpawnRequest, name: &str, value: OsString) {
    request
        .env
        .retain(|(key, _)| !crate::spawn::same_name(key, name));
    request.env.push((name.into(), value));
}

#[cfg(all(test, unix))]
#[path = "shell/tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "shell/windows_tests.rs"]
mod windows_tests;

#[cfg(test)]
mod encoding_tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    #[test]
    fn powershell_receives_the_script_as_utf16_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        let utf16: Vec<u8> = "try { 1 } catch {}"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(powershell_command("1"), base64(&utf16));
    }

    #[test]
    fn powershell_script_fits_one_command_line() {
        assert!(POWERSHELL.is_ascii());
        assert!(POWERSHELL.len() < 12 * 1024);
        assert!(powershell_command(POWERSHELL).len() < 32_767 - 64);
    }

    #[cfg(windows)]
    #[test]
    fn integration_starts_powershell_with_the_encoded_script_only_when_enabled() {
        let directory = std::env::temp_dir().join(format!(
            "muxy-powershell-{}",
            muxy_protocol::OperationId::new()
        ));
        let integration = ShellIntegration::install(&directory).unwrap();
        let request = |program: &str| SpawnRequest {
            program: program.into(),
            args: vec!["-NoLogo".into()],
            cwd: directory.clone(),
            env: Vec::new(),
            size: muxy_terminal::pty::PtySize { cols: 80, rows: 24 },
        };
        let mut enabled = request("C:/Program Files/PowerShell/7/PWSH.EXE");
        integration.configure(&mut enabled, true);
        assert_eq!(
            enabled.args[1..3],
            ["-NoExit", "-EncodedCommand"].map(OsString::from)
        );
        assert_eq!(
            enabled.args[3],
            OsString::from(powershell_command(POWERSHELL))
        );
        let mut disabled = request("powershell.exe");
        integration.configure(&mut disabled, false);
        assert_eq!(disabled.args, ["-NoLogo"].map(OsString::from));
        let _ = fs::remove_dir_all(directory);
    }
}
