use muxy_api::execution_environment::{ExecutionEnvironment, ExecutionEnvironmentSource};
use muxy_api::git::GitOptions;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;

pub fn environment_source() -> ExecutionEnvironmentSource {
    environment_source_with(ExecutionEnvironment::from_current_process())
}

fn environment_source_with(environment: ExecutionEnvironment) -> ExecutionEnvironmentSource {
    ExecutionEnvironmentSource::new(environment)
}

pub fn options(environment: &ExecutionEnvironment) -> GitOptions {
    GitOptions {
        executable: environment
            .resolve_executable(OsStr::new("git"))
            .unwrap_or_else(|| PathBuf::from("git")),
        environment: HashMap::from([
            ("GIT_OPTIONAL_LOCKS".to_owned(), "0".to_owned()),
            (
                "PATH".to_owned(),
                environment
                    .get(OsStr::new("PATH"))
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};

    fn environment(path: &str) -> muxy_api::execution_environment::ExecutionEnvironment {
        muxy_api::execution_environment::ExecutionEnvironment::fallback([
            (OsString::from("PATH"), OsString::from(path)),
            (OsString::from("HOME"), OsString::from("/fixture/home")),
        ])
    }

    #[test]
    fn initial_git_environment_uses_the_shared_inherited_first_fallback() {
        let source = environment_source_with(environment("/launch/bin:/usr/bin"));
        let snapshot = source.snapshot();
        let entries: Vec<&str> = snapshot
            .get(OsStr::new("PATH"))
            .unwrap()
            .to_str()
            .unwrap()
            .split(':')
            .collect();

        assert_eq!(&entries[..2], ["/launch/bin", "/usr/bin"]);
        for candidate in muxy_api::execution_environment::FALLBACK_EXECUTABLE_PATHS {
            assert_eq!(
                entries.iter().filter(|entry| **entry == candidate).count(),
                1,
                "{candidate} should appear exactly once"
            );
        }
    }

    #[test]
    fn future_git_options_are_derived_from_the_latest_snapshot() {
        let launch = options(&environment("/launch/bin"));
        let hydrated = options(&environment("/hydrated/bin"));

        assert!(launch.environment["PATH"].starts_with("/launch/bin:"));
        assert!(hydrated.environment["PATH"].starts_with("/hydrated/bin:"));
        assert!(!hydrated.environment["PATH"].contains("/launch/bin"));
        assert_eq!(launch.environment["GIT_OPTIONAL_LOCKS"], "0");
        assert_eq!(hydrated.environment["GIT_OPTIONAL_LOCKS"], "0");
    }

    #[cfg(unix)]
    #[test]
    fn git_options_resolve_an_executable_from_the_supplied_snapshot() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("git");
        std::fs::write(&executable, b"fixture").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            options(&environment(directory.path().to_str().unwrap())).executable,
            std::fs::canonicalize(executable).unwrap()
        );
    }
}
