use super::*;
use muxy_protocol::{OperationId, ProjectIntent, ProjectMutation, ProjectPatch, SessionStatus};
use std::fs;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Profile(PathBuf);
impl Profile {
    fn new() -> io::Result<Self> {
        let directory = std::env::temp_dir().join(format!("muxy-recovery-{}", OperationId::new()));
        fs::create_dir(&directory)?;
        Ok(Self(directory))
    }
    fn open(&self) -> io::Result<Registry> {
        let (events, _) = std::sync::mpsc::channel();
        Registry::persistent(
            ServerSettings {
                default_shell: Some("/bin/sh".into()),
                ..ServerSettings::default()
            },
            events,
            &self.0.join("sessions"),
        )
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reservation_survives_restart_before_pty_start() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let operation = OperationId::new();
    let info = SessionInfo {
        id: registry.reserve_start()?,
        project: registry.home_project(),
        directory: ServerPath(b"/tmp".to_vec()),
    };
    registry.catalog.reserve(operation, &info)?;
    drop(registry);
    let registry = profile.open()?;
    assert_eq!(
        registry.create_project_session(
            info.project,
            operation,
            Path::new("/tmp"),
            Size { cols: 80, rows: 24 }
        )?,
        info
    );
    assert!(registry.list().is_empty());
    assert_eq!(
        registry
            .list_project_sessions(info.project, None, None)?
            .sessions[0]
            .status,
        SessionStatus::Unavailable
    );
    Ok(())
}

#[test]
fn failed_catalog_replace_does_not_publish_or_forget_a_mutation() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let before = registry.read_catalog(None, None)?;
    let path = profile.0.join("sessions/catalog.json");
    let backup = profile.0.join("catalog.backup");
    fs::rename(&path, &backup)?;
    fs::create_dir(&path)?;
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Patch {
            project: before.home,
            patch: ProjectPatch::Name("Saved".into()),
        },
    };
    assert!(registry.mutate_project(&intent).is_err());
    assert_eq!(registry.read_catalog(None, None)?, before);
    fs::remove_dir(&path)?;
    fs::rename(&backup, &path)?;
    registry.mutate_project(&intent)?;
    assert_eq!(registry.read_catalog(None, None)?.projects[0].name, "Saved");
    Ok(())
}
