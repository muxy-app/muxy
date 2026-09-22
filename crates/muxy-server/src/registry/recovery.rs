use super::*;
use muxy_protocol::{
    OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation, ProjectPatch,
    SessionStatus,
};
use std::fs;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cached_receipts_require_the_directory_durability_barrier() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let operation = OperationId::new();
    let home = registry.home_project();
    let session = registry.create_project_session(
        home,
        operation,
        Path::new("/tmp"),
        Size { cols: 80, rows: 24 },
    )?;
    registry.end(session.id)?;
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Patch {
            project: home,
            patch: ProjectPatch::Name("Saved".into()),
        },
    };
    registry.mutate_project(&intent)?;
    let parent = profile.0.join("sessions");
    let backup = profile.0.join("saved-sessions");
    fs::rename(&parent, &backup)?;
    fs::write(&parent, b"directory unavailable")?;
    assert_eq!(
        registry.mutate_project(&intent).unwrap_err().code(),
        ErrorCode::PersistenceFailed
    );
    assert_eq!(
        registry
            .create_project_session(
                home,
                operation,
                Path::new("/tmp"),
                Size { cols: 80, rows: 24 }
            )
            .unwrap_err()
            .code(),
        ErrorCode::PersistenceFailed
    );
    fs::remove_file(&parent)?;
    fs::rename(&backup, &parent)?;
    registry.mutate_project(&intent)?;
    assert_eq!(
        registry.create_project_session(
            home,
            operation,
            Path::new("/tmp"),
            Size { cols: 80, rows: 24 }
        )?,
        session
    );
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn natural_exit_retries_a_failed_catalog_write_without_server_restart() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let session = registry.create(Path::new("/tmp"), Size { cols: 80, rows: 24 })?;
    let revision = registry.catalog_revision();
    let path = profile.0.join("sessions/catalog.json");
    let backup = profile.0.join("catalog.backup");
    fs::rename(&path, &backup)?;
    fs::create_dir(&path)?;
    registry
        .handle(session.id)
        .ok_or("session")?
        .send(session::SessionCommand::Input(b"exit\r".into()))?;
    registry.wait_ended(session.id)?;
    assert!(registry.list().is_empty());
    assert_eq!(registry.catalog_revision(), revision);
    fs::remove_dir(&path)?;
    fs::rename(&backup, &path)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while registry.catalog_revision() == revision && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(registry.catalog_revision() > revision);
    assert_eq!(
        registry
            .list_project_sessions(session.project, None, None)?
            .sessions[0]
            .status,
        SessionStatus::Ended
    );
    Ok(())
}

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
fn deletion_and_cancellation_resume_after_their_durable_marker() -> TestResult {
    for cancel in [false, true] {
        let profile = Profile::new()?;
        let registry = profile.open()?;
        let project = ProjectDescriptor {
            id: ProjectId::new(),
            home: false,
            name: "Delete".into(),
            icon: None,
            logo: None,
            color: "#808080".into(),
            directory: ServerPath(profile.0.as_os_str().as_bytes().into()),
            kind: None,
            parent_id: None,
        };
        registry.mutate_project(&ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Create(project.clone()),
        })?;
        let operation = OperationId::new();
        let session = registry.create_project_session(
            project.id,
            operation,
            &profile.0,
            Size { cols: 80, rows: 24 },
        )?;
        registry.end(session.id)?;
        let path = profile
            .0
            .join("sessions")
            .join(format!("{:016x}.postcard", session.id.get()));
        assert!(path.exists());
        if cancel {
            registry.catalog.cancel_creation(operation)?;
        } else {
            registry.catalog.begin_mutation(&ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Delete(project.id),
            })?;
        }
        drop(registry);
        let registry = profile.open()?;
        assert!(!path.exists());
        assert!(registry.list().is_empty());
        if !cancel {
            assert!(
                !registry
                    .read_catalog(None, None)?
                    .projects
                    .iter()
                    .any(|entry| entry.id == project.id)
            );
        }
    }
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

#[test]
fn unreferenced_legacy_archive_belongs_to_home_without_payload_changes() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let session = registry.create(Path::new("/tmp"), Size { cols: 80, rows: 24 })?;
    registry.end(session.id)?;
    drop(registry);
    let path = profile
        .0
        .join("sessions")
        .join(format!("{:016x}.postcard", session.id.get()));
    let original = fs::read(&path)?;
    fs::remove_file(profile.0.join("sessions/catalog.json"))?;
    let imported = profile.open()?;
    let page = imported.list_project_sessions(imported.home_project(), None, None)?;
    assert_eq!(page.sessions[0].info.id, session.id);
    assert_eq!(page.sessions[0].status, SessionStatus::Ended);
    assert_eq!(fs::read(path)?, original);
    Ok(())
}
