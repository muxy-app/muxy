use muxy_protocol::{
    ErrorCode, OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation,
    ProjectPatch, ServerPath, SessionStatus, Size,
};
use muxy_server::{LegacyImport, Registry, ServerSettings};
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const SIZE: Size = Size { cols: 80, rows: 24 };

#[test]
fn catalog_pages_are_bounded_ordered_and_reject_cross_revision_continuation() -> Result {
    let (events, _) = mpsc::channel();
    let registry = Registry::new(ServerSettings::default(), events);
    for _ in 0..260 {
        registry.mutate_project(&intent(ProjectMutation::Create(project(Path::new("/tmp")))))?;
    }
    let mut catalog = registry.read_catalog(None, None)?;
    let revision = catalog.revision;
    let mut ids = Vec::new();
    loop {
        catalog
            .validate()
            .map_err(|code| format!("invalid page: {code:?}"))?;
        assert!(catalog.projects.len() <= muxy_protocol::CATALOG_PAGE_SIZE);
        ids.extend(catalog.projects.iter().map(|project| project.id));
        let Some(after) = catalog.next else {
            break;
        };
        catalog = registry.read_catalog(Some(after), Some(revision))?;
    }
    assert_eq!(ids.len(), 261);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    registry.mutate_project(&intent(ProjectMutation::Patch {
        project: registry.home_project(),
        patch: ProjectPatch::Name("Home renamed".into()),
    }))?;
    assert_eq!(
        registry
            .read_catalog(Some(ids[127]), Some(revision))
            .err()
            .ok_or("expected revision error")?
            .code(),
        ErrorCode::CatalogChanged
    );
    Ok(())
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("muxy-catalog-{}", ProjectId::new()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn registry(&self, legacy: LegacyImport) -> Result<Registry> {
        let (events, _) = mpsc::channel();
        Ok(Registry::persistent_with_import(
            ServerSettings {
                default_shell: Some("/bin/sh".into()),
                ..ServerSettings::default()
            },
            events,
            &self.0.join("sessions"),
            legacy,
        )?)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn project(path: &Path) -> ProjectDescriptor {
    ProjectDescriptor {
        id: ProjectId::new(),
        home: false,
        name: "Test".into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: ServerPath(path.as_os_str().as_bytes().into()),
        kind: None,
        parent_id: None,
    }
}
fn intent(mutation: ProjectMutation) -> ProjectIntent {
    ProjectIntent {
        operation: OperationId::new(),
        mutation,
    }
}

#[test]
fn exact_identity_duplicate_paths_private_catalog_and_idempotent_mutations() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry(LegacyImport::default())?;
    let before = registry.read_catalog(None, None)?;
    let first = project(&fixture.0);
    let second = project(&fixture.0);
    let create = intent(ProjectMutation::Create(first.clone()));
    registry.mutate_project(&create)?;
    registry.mutate_project(&create)?;
    registry.mutate_project(&intent(ProjectMutation::Create(second.clone())))?;
    assert_eq!(registry.read_catalog(None, None)?.projects.len(), 3);
    let rename = intent(ProjectMutation::Patch {
        project: first.id,
        patch: ProjectPatch::Name("Shared".into()),
    });
    registry.mutate_project(&rename)?;
    let revision = registry.catalog_revision();
    registry.mutate_project(&rename)?;
    assert_eq!(registry.catalog_revision(), revision);
    assert_eq!(
        registry
            .read_catalog(None, Some(before.revision))
            .err()
            .ok_or("expected stale revision")?
            .code(),
        ErrorCode::CatalogChanged
    );
    assert_eq!(
        fs::metadata(fixture.0.join("sessions/catalog.json"))?
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    drop(registry);
    let reopened = fixture.registry(LegacyImport::default())?;
    let after = reopened.read_catalog(None, None)?;
    assert_eq!((after.server, after.home), (before.server, before.home));
    assert!(
        after
            .projects
            .iter()
            .any(|project| project.id == first.id && project.name == "Shared")
    );
    assert!(
        reopened
            .mutate_project(&intent(ProjectMutation::Delete(after.home)))
            .is_err()
    );
    Ok(())
}

#[test]
fn retry_never_respawns_ended_or_discarded_sessions_and_cwd_never_owns() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry(LegacyImport::default())?;
    let owner = project(&fixture.0);
    registry.mutate_project(&intent(ProjectMutation::Create(owner.clone())))?;
    let operation = OperationId::new();
    let first = registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?;
    assert_eq!(first.project, owner.id);
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert_eq!(registry.list().len(), 1);
    registry.end(first.id)?;
    let listed = registry.list_project_sessions(owner.id, None, None)?;
    assert_eq!(listed.sessions[0].status, SessionStatus::Ended);
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert!(registry.list().is_empty());
    registry.discard(first.id)?;
    assert!(
        registry
            .list_project_sessions(owner.id, None, None)?
            .sessions
            .is_empty()
    );
    drop(registry);
    let registry = fixture.registry(LegacyImport::default())?;
    assert_eq!(
        registry.create_project_session(owner.id, operation, Path::new("/tmp"), SIZE)?,
        first
    );
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn deletion_races_creation_and_retries_without_touching_files() -> Result {
    let fixture = Fixture::new()?;
    let registry = Arc::new(fixture.registry(LegacyImport::default())?);
    let owner = project(&fixture.0);
    registry.mutate_project(&intent(ProjectMutation::Create(owner.clone())))?;
    fs::write(fixture.0.join("keep"), b"user files")?;
    let first = registry.create_project_session(owner.id, OperationId::new(), &fixture.0, SIZE)?;
    let deleting = Arc::clone(&registry);
    let delete = intent(ProjectMutation::Delete(owner.id));
    let repeated = delete.clone();
    let worker = std::thread::spawn(move || deleting.mutate_project(&delete));
    let created = registry.create_project_session(owner.id, OperationId::new(), &fixture.0, SIZE);
    worker.join().map_err(|_| "delete worker panicked")??;
    if let Ok(created) = created {
        assert!(registry.handle(created.id).is_none());
    }
    assert!(registry.handle(first.id).is_none());
    assert!(registry.read_saved_screen(first.id).is_err());
    registry.mutate_project(&repeated)?;
    assert_eq!(fs::read(fixture.0.join("keep"))?, b"user files");
    drop(registry);
    let reopened = fixture.registry(LegacyImport::default())?;
    assert_eq!(reopened.read_catalog(None, None)?.projects.len(), 1);
    Ok(())
}

#[test]
fn import_is_once_preserves_home_and_rejects_missing_membership_without_committing() -> Result {
    let fixture = Fixture::new()?;
    let mut home = project(&fixture.0);
    home.home = true;
    let first = project(&fixture.0);
    let legacy = LegacyImport {
        projects: vec![first.clone(), home.clone()],
        sessions: std::collections::BTreeMap::new(),
    };
    let registry = fixture.registry(legacy.clone())?;
    assert_eq!(registry.home_project(), home.id);
    registry.mutate_project(&intent(ProjectMutation::Delete(first.id)))?;
    drop(registry);
    let registry = fixture.registry(legacy)?;
    assert!(
        !registry
            .read_catalog(None, None)?
            .projects
            .iter()
            .any(|project| project.id == first.id)
    );
    drop(registry);
    let invalid = Fixture::new()?;
    let session = muxy_protocol::SessionId::new(99).ok_or("ID")?;
    assert!(
        invalid
            .registry(LegacyImport {
                projects: vec![],
                sessions: [(session, first.id)].into()
            })
            .is_err()
    );
    assert!(!invalid.0.join("sessions/catalog.json").exists());
    Ok(())
}

#[test]
fn cancellation_before_or_after_create_prevents_orphan_shells() -> Result {
    let fixture = Fixture::new()?;
    let registry = fixture.registry(LegacyImport::default())?;
    let home = registry.home_project();
    let cancelled = OperationId::new();
    registry.cancel_creation(cancelled)?;
    assert!(
        registry
            .create_project_session(home, cancelled, &fixture.0, SIZE)
            .is_err()
    );
    let operation = OperationId::new();
    let session = registry.create_project_session(home, operation, &fixture.0, SIZE)?;
    registry.cancel_creation(operation)?;
    registry.cancel_creation(operation)?;
    assert!(registry.handle(session.id).is_none());
    assert!(registry.read_saved_screen(session.id).is_err());
    drop(registry);
    let registry = fixture.registry(LegacyImport::default())?;
    assert!(
        registry
            .create_project_session(home, operation, &fixture.0, SIZE)
            .is_err()
    );
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn non_utf8_project_paths_and_failed_creation_survive_restart() -> Result {
    let fixture = Fixture::new()?;
    let path = fixture
        .0
        .join(std::ffi::OsString::from_vec(b"project-\xff".to_vec()));
    // macOS filesystems reject invalid UTF-8 names; imported bytes must still survive losslessly.
    let owner = project(&path);
    let registry = fixture.registry(LegacyImport {
        projects: vec![owner.clone()],
        sessions: std::collections::BTreeMap::new(),
    })?;
    let operation = OperationId::new();
    let missing = fixture.0.join("missing");
    let failure = registry
        .create_project_session(owner.id, operation, &missing, SIZE)
        .err()
        .ok_or("expected failure")?;
    fs::create_dir(&missing)?;
    drop(registry);
    let registry = fixture.registry(LegacyImport::default())?;
    assert!(registry.read_catalog(None, None)?.projects.contains(&owner));
    let repeated = registry
        .create_project_session(owner.id, operation, &missing, SIZE)
        .err()
        .ok_or("retry started a shell")?;
    assert_eq!(repeated, failure);
    assert!(registry.list().is_empty());
    Ok(())
}

#[test]
fn project_artwork_is_shared_and_persists_replacement_and_removal() -> Result {
    let fixture = Fixture::new()?;
    let logo: Arc<[u8]> = include_bytes!("../../muxy-protocol/tests/fixtures/project-logo.png")
        .as_slice()
        .into();
    let project = project(Path::new("/tmp"));
    let id = project.id;
    {
        let registry = fixture.registry(LegacyImport::default())?;
        registry.mutate_project(&intent(ProjectMutation::Create(project)))?;
        for patch in [
            ProjectPatch::Icon(Some("sf:terminal.fill".into())),
            ProjectPatch::Logo(Some(logo.clone())),
        ] {
            registry.mutate_project(&intent(ProjectMutation::Patch { project: id, patch }))?;
        }
    }
    {
        let registry = fixture.registry(LegacyImport::default())?;
        let page = registry.read_catalog(None, None)?;
        let project = page.projects.iter().find(|p| p.id == id).ok_or("project")?;
        assert_eq!(project.logo.as_ref(), Some(&logo));
        assert_eq!(project.icon.as_deref(), Some("sf:terminal.fill"));
        registry.mutate_project(&intent(ProjectMutation::Patch {
            project: id,
            patch: ProjectPatch::Logo(None),
        }))?;
    }
    let registry = fixture.registry(LegacyImport::default())?;
    let page = registry.read_catalog(None, None)?;
    let project = page.projects.iter().find(|p| p.id == id).ok_or("project")?;
    assert!(project.logo.is_none());
    assert_eq!(project.icon.as_deref(), Some("sf:terminal.fill"));
    Ok(())
}

#[test]
fn logo_heavy_catalog_pages_fit_the_wire_and_continue_without_losing_projects() -> Result {
    let (events, _) = mpsc::channel();
    let registry = Registry::new(ServerSettings::default(), events);
    let mut logo = include_bytes!("../../muxy-protocol/tests/fixtures/project-logo.png").to_vec();
    logo.resize(muxy_protocol::MAX_PROJECT_LOGO_BYTES, 0);
    let logo: Arc<[u8]> = logo.into();
    for _ in 0..60 {
        let mut project = project(Path::new("/tmp"));
        project.logo = Some(logo.clone());
        registry.mutate_project(&intent(ProjectMutation::Create(project)))?;
    }
    let mut after = None;
    let mut count = 0;
    loop {
        let page = registry.read_catalog(after, None)?;
        count += page.projects.len();
        after = page.next;
        let mut bytes = Vec::new();
        muxy_protocol::wire::encode(
            &muxy_protocol::Message::Reply {
                id: muxy_protocol::RequestId(1),
                body: muxy_protocol::ReplyBody::Catalog(page),
            },
            muxy_protocol::CONTROL,
            &mut bytes,
        )?;
        assert!(bytes.len() < 4 * 1024 * 1024);
        if after.is_none() {
            break;
        }
    }
    assert_eq!(count, 61);
    Ok(())
}
