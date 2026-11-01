use super::storage::StoredState;
use super::{Catalog, Creation, GitReceipt, LegacyImport, Membership, Receipt, State, expiry};
use crate::archive::Archive;
use muxy_protocol::{
    ErrorCode, ErrorReply, GitReply, OperationId, ProjectDescriptor, ProjectId, ProjectIntent,
    ProjectKind, ProjectMutation, ProjectPatch, ServerIdentity, ServerPath, SessionId, SessionInfo,
    SessionStatus, WorktreeAction, WorktreeHook, WorktreeIntent, WorktreeOptions, WorktreeRemoval,
};
use std::collections::{BTreeMap, BTreeSet};

fn logo() -> std::sync::Arc<[u8]> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&16u32.to_be_bytes());
    png.extend_from_slice(&16u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0, 1, 2, 3, 4]);
    png.into()
}

fn session(id: u64) -> SessionId {
    SessionId::new(id).expect("non-zero session id")
}

fn error(code: ErrorCode) -> ErrorReply {
    ErrorReply {
        code,
        message: format!("{code:?} message"),
    }
}

#[allow(clippy::too_many_lines, reason = "One record per saved variant")]
pub(super) fn sample() -> State {
    let home = ProjectDescriptor {
        id: ProjectId::from_u128(1),
        home: true,
        directory: ServerPath::from("/home/muxy"),
        name: "Home".into(),
        icon: None,
        logo: None,
        color: "#e5484d".into(),
        kind: None,
        parent_id: None,
    };
    let project = ProjectDescriptor {
        id: ProjectId::from_u128(2),
        home: false,
        directory: ServerPath::from("/home/muxy/app"),
        name: "App".into(),
        icon: Some("sf:folder.fill".into()),
        logo: Some(logo()),
        color: "#30a46c".into(),
        kind: None,
        parent_id: None,
    };
    let worktree = ProjectDescriptor {
        id: ProjectId::from_u128(3),
        home: false,
        directory: ServerPath::from("/home/muxy/app-feature"),
        name: "feature".into(),
        icon: Some("🌿".into()),
        logo: None,
        color: "#30a46c".into(),
        kind: Some(ProjectKind::Worktree),
        parent_id: Some(project.id),
    };
    let info = |id: u64, project: &ProjectDescriptor| SessionInfo {
        project: project.id,
        id: session(id),
        directory: project.directory.clone(),
    };
    let operation = |id: u128| OperationId::from_u128(0x100 + id);
    let removal = WorktreeRemoval {
        directory: worktree.directory.clone(),
        device: 7,
        inode: 11,
        dirty: true,
        status: b" M src/main.rs\0".to_vec(),
        head: Some("0123abcd".into()),
        branch: Some("feature".into()),
    };
    let git_receipt = |id: u128, action: WorktreeAction| GitReceipt {
        owner: project.id,
        intent: WorktreeIntent {
            operation: operation(id),
            action,
            options: None,
        },
        project: worktree.clone(),
        device: 7,
        inode: 11,
        applied: true,
        hooks_started: false,
        hooks_finished: false,
        hook_error: None,
        failed: None,
        reply: None,
    };
    let receipt = |id: u128, mutation: ProjectMutation, error: Option<ErrorReply>| {
        (
            operation(id),
            Receipt {
                intent: ProjectIntent {
                    operation: operation(id),
                    mutation,
                },
                error,
                complete: true,
            },
        )
    };
    let finished = |ids: &[u128]| {
        ids.iter()
            .map(|id| {
                (
                    operation(*id),
                    1_700_000_000_000 + u64::try_from(*id).expect("small"),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };

    let mut git = BTreeMap::new();
    let mut created = git_receipt(
        1,
        WorktreeAction::Create {
            project: project.id,
            directory: worktree.directory.clone(),
            branch: "feature".into(),
            base: Some("main".into()),
        },
    );
    created.intent.options = Some(WorktreeOptions {
        name: Some("feature".into()),
        hooks: Some(vec![WorktreeHook {
            command: "npm install".into(),
            name: Some("Install".into()),
            project: true,
        }]),
    });
    created.hooks_started = true;
    created.hooks_finished = true;
    created.reply = Some(GitReply::Project(worktree.clone()));
    let mut registered = git_receipt(
        2,
        WorktreeAction::Register {
            project: project.id,
            directory: worktree.directory.clone(),
        },
    );
    registered.hooks_started = true;
    registered.hooks_finished = true;
    registered.hook_error = Some("hook failed".into());
    registered.reply = Some(GitReply::WorktreeSetupFailed {
        project: worktree.clone(),
        message: "hook failed".into(),
    });
    let mut removed = git_receipt(3, WorktreeAction::Remove { expected: removal });
    removed.reply = Some(GitReply::Done);
    let mut checkout = git_receipt(
        4,
        WorktreeAction::CheckoutPullRequest {
            project: project.id,
            directory: worktree.directory.clone(),
            number: 42,
        },
    );
    checkout.applied = false;
    checkout.failed = Some(error(ErrorCode::BadPath));
    let pending = git_receipt(
        5,
        WorktreeAction::Register {
            project: project.id,
            directory: worktree.directory.clone(),
        },
    );
    for receipt in [created, registered, removed, checkout, pending] {
        git.insert(receipt.intent.operation, receipt);
    }

    let codes = [
        ErrorCode::UnknownProject,
        ErrorCode::CatalogChanged,
        ErrorCode::UnknownSession,
        ErrorCode::UnknownChannel,
        ErrorCode::BadSize,
        ErrorCode::BadPath,
        ErrorCode::SpawnFailed,
        ErrorCode::BadRequest,
        ErrorCode::SavedContentUnavailable,
        ErrorCode::StaleHistoryCursor,
        ErrorCode::HistoryUnavailable,
        ErrorCode::PersistenceFailed,
        ErrorCode::Unauthorized,
        ErrorCode::Unsupported,
    ];
    let mut creations: BTreeMap<_, _> = codes
        .into_iter()
        .zip(1u64..)
        .map(|(code, id)| {
            (
                operation(0x10 + u128::from(id)),
                Creation {
                    info: info(id, &project),
                    error: Some(error(code)),
                },
            )
        })
        .collect();
    creations.insert(
        operation(0x20),
        Creation {
            info: info(20, &worktree),
            error: None,
        },
    );

    let sessions = [
        (20, SessionStatus::Starting, None),
        (21, SessionStatus::Live, None),
        (22, SessionStatus::Ended, Some(1_700_000_000)),
        (23, SessionStatus::Unavailable, Some(1_700_000_001)),
    ]
    .into_iter()
    .map(|(id, status, ended_at)| {
        (
            session(id),
            Membership {
                info: info(id, &worktree),
                status,
                ended_at,
            },
        )
    })
    .collect();

    let receipts = [
        receipt(0x30, ProjectMutation::Create(project.clone()), None),
        receipt(
            0x31,
            ProjectMutation::Patch {
                project: project.id,
                patch: ProjectPatch::Name("App".into()),
            },
            None,
        ),
        receipt(
            0x32,
            ProjectMutation::Patch {
                project: project.id,
                patch: ProjectPatch::Icon(Some("sf:folder.fill".into())),
            },
            None,
        ),
        receipt(
            0x33,
            ProjectMutation::Patch {
                project: project.id,
                patch: ProjectPatch::Color("#30a46c".into()),
            },
            None,
        ),
        receipt(
            0x34,
            ProjectMutation::Patch {
                project: project.id,
                patch: ProjectPatch::Logo(Some(logo())),
            },
            None,
        ),
        receipt(
            0x35,
            ProjectMutation::Delete(ProjectId::from_u128(9)),
            Some(error(ErrorCode::UnknownProject)),
        ),
        receipt(0x36, ProjectMutation::PruneWorktree(worktree.id), None),
    ]
    .into_iter()
    .collect();

    State {
        git,
        version: 1,
        server: ServerIdentity::from_u128(0xabc),
        home: home.id,
        legacy_home: Some(home.id),
        revision: 17,
        projects: [home, project, worktree]
            .into_iter()
            .map(|project| (project.id, project))
            .collect(),
        deleting: BTreeSet::new(),
        discarding: BTreeSet::from([session(23)]),
        cancelled: BTreeSet::from([operation(0x11)]),
        sessions,
        creations,
        receipts,
        closes: BTreeMap::from([(operation(0x40), (session(23), true))]),
        finished: expiry::Finished {
            creations: finished(&[0x11]),
            cancelled: finished(&[0x11]),
            receipts: finished(&[0x30, 0x31]),
            closes: finished(&[0x40]),
            git: finished(&[1, 2, 3, 4]),
        },
    }
}

const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/catalog-v1.json");

fn saved(state: &State) -> serde_json::Value {
    serde_json::to_value(StoredState::try_from(state).expect("storable")).expect("serializes")
}

fn load(json: &serde_json::Value) -> State {
    serde_json::from_value::<StoredState>(json.clone())
        .expect("readable")
        .into()
}

fn open(json: &serde_json::Value) -> std::io::Result<State> {
    let directory = std::env::temp_dir().join(format!("muxy-catalog-{}", OperationId::new()));
    std::fs::create_dir(&directory)?;
    std::fs::write(directory.join("catalog.json"), serde_json::to_vec(json)?)?;
    let catalog = Catalog::open(
        &directory,
        &Archive::memory(0),
        LegacyImport::default(),
        std::sync::Arc::default(),
    );
    std::fs::remove_dir_all(&directory)?;
    Ok(catalog?.lock().clone())
}

fn fixture() -> serde_json::Value {
    serde_json::from_slice(FIXTURE).expect("fixture is JSON")
}

#[test]
fn saved_catalog_keeps_its_disk_format() {
    assert_eq!(saved(&sample()), fixture());
    assert_eq!(saved(&load(&fixture())), fixture());
}

#[test]
fn saved_catalog_opens() -> std::io::Result<()> {
    let state = open(&fixture())?;
    assert_eq!(state.projects.len(), 3);
    assert_eq!(state.sessions.len(), 4);
    assert!(state.revision > sample().revision);
    Ok(())
}

#[test]
fn values_from_a_newer_build_do_not_stop_startup() -> std::io::Result<()> {
    let mut json = fixture();
    let worktree = ProjectId::from_u128(3).to_string();
    let creation = OperationId::from_u128(0x111).to_string();
    let mutation = OperationId::from_u128(0x130).to_string();
    let patch = OperationId::from_u128(0x131).to_string();
    let action = OperationId::from_u128(0x101).to_string();
    let reply = OperationId::from_u128(0x103).to_string();
    json["projects"][&worktree]["kind"] = "sandbox".into();
    json["sessions"]["21"]["status"] = "Paused".into();
    json["creations"][&creation]["error"]["code"] = "Throttled".into();
    json["receipts"][&mutation]["intent"]["mutation"] =
        serde_json::json!({ "Archive": { "project": worktree } });
    json["receipts"][&patch]["intent"]["mutation"]["Patch"]["patch"] =
        serde_json::json!({ "Pinned": true });
    json["git"][&action]["intent"]["action"] = serde_json::json!({ "Move": { "to": [47] } });
    json["git"][&reply]["reply"] = "Merged".into();

    open(&json)?;
    let state = load(&json);

    let unknown = |id: &str| id.parse::<OperationId>().expect("operation id");
    assert_eq!(
        state.projects[&ProjectId::from_u128(3)].kind,
        Some(ProjectKind::Unrecognized(u32::MAX))
    );
    assert_eq!(
        state.creations[&unknown(&creation)]
            .error
            .as_ref()
            .map(|error| error.code),
        Some(ErrorCode::Unrecognized(u32::MAX))
    );
    assert!(state.sessions.contains_key(&session(21)));
    for operation in [&mutation, &patch] {
        assert!(!state.receipts.contains_key(&unknown(operation)));
    }
    for operation in [&action, &reply] {
        assert!(!state.git.contains_key(&unknown(operation)));
    }
    assert_eq!(state.receipts.len(), 5);
    assert_eq!(state.git.len(), 3);
    Ok(())
}

#[test]
fn a_parent_of_a_kind_from_a_newer_build_does_not_stop_startup() -> std::io::Result<()> {
    let mut json = fixture();
    json["projects"][ProjectId::from_u128(2).to_string()]["kind"] = "repository".into();
    let state = open(&json)?;
    assert_eq!(
        state.projects[&ProjectId::from_u128(3)].parent_id,
        Some(ProjectId::from_u128(2))
    );
    Ok(())
}

#[test]
fn an_unreadable_catalog_is_moved_aside_and_startup_continues() -> std::io::Result<()> {
    let mut invalid = fixture();
    invalid["version"] = 2.into();
    for contents in [b"damaged {".to_vec(), serde_json::to_vec(&invalid)?] {
        let directory = std::env::temp_dir().join(format!("muxy-catalog-{}", OperationId::new()));
        std::fs::create_dir(&directory)?;
        let path = directory.join("catalog.json");
        std::fs::write(&path, &contents)?;
        let legacy = LegacyImport {
            projects: vec![load(&fixture()).projects[&ProjectId::from_u128(2)].clone()],
            sessions: BTreeMap::new(),
        };
        let catalog = Catalog::open(
            &directory,
            &Archive::memory(0),
            legacy,
            std::sync::Arc::default(),
        )?;
        let kept = catalog
            .unreadable()
            .map(|file| std::path::PathBuf::from(file.kept_as.clone()))
            .ok_or_else(|| std::io::Error::other("the unreadable catalog is not reported"))?;
        assert_eq!(std::fs::read(kept)?, contents);
        assert!(path.exists(), "the fresh catalog is saved");
        let state = catalog.lock().clone();
        assert_eq!(
            state.projects.len(),
            1,
            "only Home, without the legacy import"
        );
        assert!(state.projects[&state.home].home);
        std::fs::remove_dir_all(&directory)?;
    }
    Ok(())
}
