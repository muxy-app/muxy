use std::env;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub const RESOURCE_DIR_ENV: &str = "MUXY_RESOURCE_DIR";

const GHOSTTY_TERMINFO_ENTRY: &str = "67/ghostty";
const XTERM_GHOSTTY_TERMINFO_ENTRY: &str = "78/xterm-ghostty";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppResources {
    pub root: PathBuf,
    pub ghostty: PathBuf,
    pub shell_integration: PathBuf,
    pub defaults_config: PathBuf,
    pub transparent_surface_config: PathBuf,
    pub terminfo: PathBuf,
}

impl AppResources {
    pub fn discover() -> Result<Self, ResourceError> {
        if let Some(resource_dir) = env::var_os(RESOURCE_DIR_ENV) {
            return Self::from_resource_dir(resource_dir);
        }

        let executable = env::current_exe().map_err(ResourceError::CurrentExecutable)?;
        Self::from_executable(executable)
    }

    pub fn from_executable(executable: impl AsRef<Path>) -> Result<Self, ResourceError> {
        let executable = executable.as_ref();
        let macos_dir = executable
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "MacOS"));
        let contents_dir = macos_dir
            .and_then(Path::parent)
            .filter(|path| path.file_name().is_some_and(|name| name == "Contents"));
        let Some(contents_dir) = contents_dir else {
            return Err(ResourceError::InvalidBundleExecutable(
                executable.to_path_buf(),
            ));
        };

        Self::from_resource_dir(contents_dir.join("Resources"))
    }

    pub fn from_resource_dir(resource_dir: impl Into<PathBuf>) -> Result<Self, ResourceError> {
        let root = resource_dir.into();
        require_directory(&root, "resource root")?;

        let ghostty = root.join("ghostty");
        require_directory(&ghostty, "ghostty resources")?;

        let shell_integration = ghostty.join("shell-integration");
        require_directory(&shell_integration, "ghostty shell integration")?;

        let defaults_config = root.join("ghostty-overrides").join("muxy-defaults.conf");
        require_file(&defaults_config, "Muxy ghostty defaults")?;
        let transparent_surface_config = root
            .join("ghostty-overrides")
            .join("transparent-surface.conf");
        require_file(
            &transparent_surface_config,
            "Muxy transparent surface override",
        )?;

        let terminfo = root.join("terminfo");
        require_directory(&terminfo, "terminfo database")?;
        require_file(
            &terminfo.join(GHOSTTY_TERMINFO_ENTRY),
            "ghostty terminfo entry",
        )?;
        require_file(
            &terminfo.join(XTERM_GHOSTTY_TERMINFO_ENTRY),
            "xterm-ghostty terminfo entry",
        )?;

        Ok(Self {
            root,
            ghostty,
            shell_integration,
            defaults_config,
            transparent_surface_config,
            terminfo,
        })
    }

    #[allow(dead_code)]
    pub fn terminfo_entries(&self) -> [PathBuf; 2] {
        [
            self.terminfo.join(GHOSTTY_TERMINFO_ENTRY),
            self.terminfo.join(XTERM_GHOSTTY_TERMINFO_ENTRY),
        ]
    }
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("failed to resolve the current executable")]
    CurrentExecutable(#[source] io::Error),

    #[error(
        "executable {0} is not under Contents/MacOS; set MUXY_RESOURCE_DIR for an unbundled launch"
    )]
    InvalidBundleExecutable(PathBuf),

    #[error("missing {kind} directory at {path}")]
    MissingDirectory { kind: &'static str, path: PathBuf },

    #[error("missing {kind} file at {path}")]
    MissingFile { kind: &'static str, path: PathBuf },
}

fn require_directory(path: &Path, kind: &'static str) -> Result<(), ResourceError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(ResourceError::MissingDirectory {
            kind,
            path: path.to_path_buf(),
        })
    }
}

fn require_file(path: &Path, kind: &'static str) -> Result<(), ResourceError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(ResourceError::MissingFile {
            kind,
            path: path.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::process::Command;

    use tempfile::TempDir;

    use super::{AppResources, ResourceError};

    fn create_valid_resources(root: &Path) {
        fs::create_dir_all(root.join("ghostty/shell-integration/zsh")).unwrap();
        fs::create_dir_all(root.join("ghostty-overrides")).unwrap();
        fs::write(
            root.join("ghostty-overrides/muxy-defaults.conf"),
            "font-size = 13\n",
        )
        .unwrap();
        fs::write(
            root.join("ghostty-overrides/transparent-surface.conf"),
            "background-opacity = 0\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("terminfo/67")).unwrap();
        fs::write(root.join("terminfo/67/ghostty"), b"compiled terminfo").unwrap();
        fs::create_dir_all(root.join("terminfo/78")).unwrap();
        fs::write(root.join("terminfo/78/xterm-ghostty"), b"compiled terminfo").unwrap();
    }

    fn synthetic_bundle(temp: &TempDir) -> (PathBuf, PathBuf) {
        let contents = temp.path().join("Muxy.app/Contents");
        let executable = contents.join("MacOS/Muxy");
        let resources = contents.join("Resources");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"synthetic executable").unwrap();
        create_valid_resources(&resources);
        (executable, resources)
    }

    #[test]
    fn resolves_contents_resources_from_bundle_executable() {
        let temp = TempDir::new().unwrap();
        let (executable, resources_root) = synthetic_bundle(&temp);

        let resources = AppResources::from_executable(executable).unwrap();

        assert_eq!(resources.root, resources_root);
        assert_eq!(
            resources.shell_integration,
            resources_root.join("ghostty/shell-integration")
        );
        assert_eq!(
            resources.defaults_config,
            resources_root.join("ghostty-overrides/muxy-defaults.conf")
        );
        assert_eq!(
            resources.transparent_surface_config,
            resources_root.join("ghostty-overrides/transparent-surface.conf")
        );
        assert_eq!(resources.terminfo_entries().len(), 2);
    }

    #[test]
    fn explicit_override_resolves_without_a_bundle() {
        let temp = TempDir::new().unwrap();
        let override_root = temp.path().join("development-resources");
        create_valid_resources(&override_root);

        let resources = AppResources::from_resource_dir(&override_root).unwrap();

        assert_eq!(resources.root, override_root);
        assert_eq!(resources.ghostty, resources.root.join("ghostty"));
    }

    #[test]
    fn rejects_an_unbundled_executable_without_an_override() {
        let temp = TempDir::new().unwrap();
        let executable = temp.path().join("target/debug/Muxy");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"synthetic executable").unwrap();

        let error = AppResources::from_executable(&executable).unwrap_err();

        assert!(
            matches!(error, ResourceError::InvalidBundleExecutable(path) if path == executable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn development_cli_version_forms_report_the_injected_target() {
        let temp = TempDir::new().unwrap();
        let resources = temp.path().join("MuxyTests.app/Contents/Resources");
        let bin = resources.join("muxy-dev-bin");
        let scripts = resources.join("Muxy_Muxy.bundle/scripts");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&scripts).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/muxy-dev-bin/muxy");
        let launcher = bin.join("muxy");
        fs::copy(source, &launcher).unwrap();
        fs::write(scripts.join("muxy-cli"), b"#!/bin/bash\nexit 0\n").unwrap();
        let app = temp.path().join("Injected.app");
        let socket = temp.path().join(
            muxy_core::environment::RuntimePathPolicy::new(
                muxy_core::environment::BuildMode::Development,
            )
            .main_socket_filename(),
        );
        for form in ["version", "--version", "-V"] {
            let output = Command::new("bash")
                .arg(&launcher)
                .arg(form)
                .env("MUXY_DEVELOPMENT_VERSION", "2.3.4")
                .env("MUXY_DEVELOPMENT_APP_PATH", &app)
                .env("MUXY_DEVELOPMENT_SOCKET_PATH", &socket)
                .env("MUXY_VERSION", "wrong")
                .env("MUXY_APP_PATH", "/wrong.app")
                .env("MUXY_SOCKET_PATH", "/wrong.sock")
                .output()
                .unwrap();
            assert!(output.status.success());
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.contains("Muxy 2.3.4\n"));
            assert!(stdout.contains("mode: development\n"));
            assert!(stdout.contains(&format!("app: {}\n", app.display())));
            assert!(stdout.contains(&format!(
                "cli: {}/muxy-cli\n",
                fs::canonicalize(&scripts).unwrap().display()
            )));
            assert!(stdout.contains(&format!("socket: {}\n", socket.display())));
            assert!(stdout.contains("socket-status: missing\n"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn development_cli_forwards_other_commands_unchanged() {
        let temp = TempDir::new().unwrap();
        let resources = temp.path().join("MuxyTests.app/Contents/Resources");
        let bin = resources.join("muxy-dev-bin");
        let scripts = resources.join("Muxy_Muxy.bundle/scripts");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&scripts).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/muxy-dev-bin/muxy");
        let launcher = bin.join("muxy");
        fs::copy(source, &launcher).unwrap();
        let cli = scripts.join("muxy-cli");
        fs::write(
            &cli,
            b"#!/bin/bash\nprintf 'socket=%s\\n' \"$MUXY_SOCKET_PATH\"\nprintf '%s\\n' \"$@\"\n",
        )
        .unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let output = Command::new("bash")
            .arg(launcher)
            .args(["send", "--pane", "ABC", "hello world"])
            .env("MUXY_DEVELOPMENT_SOCKET_PATH", "/development.sock")
            .env("MUXY_SOCKET_PATH", "/production.sock")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "socket=/development.sock\nsend\n--pane\nABC\nhello world\n"
        );
        let extra_version_argument = Command::new("bash")
            .arg(bin.join("muxy"))
            .args(["--version", "extra"])
            .env("MUXY_DEVELOPMENT_SOCKET_PATH", "/development.sock")
            .output()
            .unwrap();
        assert!(extra_version_argument.status.success());
        assert_eq!(
            String::from_utf8(extra_version_argument.stdout).unwrap(),
            "socket=/development.sock\n--version\nextra\n"
        );
    }

    #[test]
    fn validates_shell_integration_defaults_and_each_terminfo_entry() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("Resources");
        create_valid_resources(&root);

        fs::remove_dir_all(root.join("ghostty/shell-integration")).unwrap();
        assert!(matches!(
            AppResources::from_resource_dir(&root),
            Err(ResourceError::MissingDirectory {
                kind: "ghostty shell integration",
                ..
            })
        ));

        create_valid_resources(&root);
        fs::remove_file(root.join("ghostty-overrides/muxy-defaults.conf")).unwrap();
        assert!(matches!(
            AppResources::from_resource_dir(&root),
            Err(ResourceError::MissingFile {
                kind: "Muxy ghostty defaults",
                ..
            })
        ));

        create_valid_resources(&root);
        fs::remove_file(root.join("terminfo/78/xterm-ghostty")).unwrap();
        assert!(matches!(
            AppResources::from_resource_dir(&root),
            Err(ResourceError::MissingFile {
                kind: "xterm-ghostty terminfo entry",
                ..
            })
        ));
    }
}
