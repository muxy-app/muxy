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

#[test]
fn a_moved_session_leaves_its_old_project_and_survives_restart() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let home = registry.home_project();
    let folder = profile.0.join("app");
    fs::create_dir(&folder)?;
    let app = muxy_protocol::ProjectDescriptor {
        id: muxy_protocol::ProjectId::new(),
        home: false,
        name: "App".into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: ServerPath::from(folder.as_path()),
        kind: None,
        parent_id: None,
    };
    registry.mutate_project(&ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(app.clone()),
    })?;
    let info = registry.create_project_session(
        app.id,
        OperationId::new(),
        &folder,
        Size { cols: 80, rows: 24 },
    )?;
    let revision = registry.sessions_revision();

    registry.move_session(info.id, home)?;
    assert!(registry.sessions_revision() > revision);
    let revision = registry.sessions_revision();
    registry.move_session(info.id, home)?;
    assert_eq!(registry.sessions_revision(), revision);

    assert_eq!(registry.catalog.owned(app.id), Vec::new());
    assert_eq!(registry.catalog.owned(home), vec![info.id]);
    assert!(
        registry
            .list_project_sessions(app.id, None, None)?
            .sessions
            .is_empty()
    );
    let listed = registry.list_project_sessions(home, None, None)?.sessions;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].info.id, info.id);
    assert_eq!(listed[0].info.project, home);
    assert_eq!(registry.list()[0].project, home);

    registry.mutate_project(&ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Delete(app.id),
    })?;
    assert!(registry.handle(info.id).is_some());
    assert_eq!(
        registry.list_project_sessions(home, None, None)?.sessions[0].status,
        SessionStatus::Live
    );
    registry.shutdown();
    drop(registry);

    let registry = profile.open()?;
    let listed = registry.list_project_sessions(home, None, None)?.sessions;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].info.id, info.id);
    assert_eq!(listed[0].info.project, home);
    Ok(())
}

#[test]
fn a_session_moves_only_to_a_project_that_exists() -> TestResult {
    let profile = Profile::new()?;
    let registry = profile.open()?;
    let info = registry.create(Path::new("/tmp"), Size { cols: 80, rows: 24 })?;
    let missing = muxy_protocol::ProjectId::new();
    assert_eq!(
        registry
            .move_session(info.id, missing)
            .map_err(|error| error.code()),
        Err(ErrorCode::UnknownProject)
    );
    assert_eq!(
        registry
            .move_session(
                SessionId::from(std::num::NonZeroU64::MAX),
                registry.home_project()
            )
            .map_err(|error| error.code()),
        Err(ErrorCode::UnknownSession)
    );
    assert_eq!(registry.list()[0].project, registry.home_project());
    registry.shutdown();
    Ok(())
}
