use super::*;
use serde_json::{Value, json};

fn project(id: ProjectId, home: bool, pane: ProjectId, session: SessionId) -> Value {
    json!({
        "id": id, "server_id": "9eedd63357b54359b76e762c6bc2775c", "home": home,
        "name": if home { "Home" } else { "Project" }, "icon": null, "color": "#808080",
        "directory": "/tmp", "kind": null, "parent_id": null,
        "tabs": [{"panes": [{"id": pane, "content": {"type": "terminal", "session": session}}]}]
    })
}

fn fixture() -> Value {
    json!({"version": 1, "projects": [
        project(ProjectId::from_u128(1), true, ProjectId::from_u128(11), SessionId::new(1).unwrap()),
        project(ProjectId::from_u128(2), false, ProjectId::from_u128(12), SessionId::new(2).unwrap())
    ], "quick_terminal": {"id": ProjectId::from_u128(13), "content": {"type": "terminal", "session": 3}},
       "pending_discards": [4]})
}

#[test]
fn imports_identity_duplicate_directories_quick_terminal_and_pending_discards() {
    let snapshot = fixture();
    let imported = parse(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert_eq!(imported.projects.len(), 2);
    assert!(imported.projects[0].home);
    assert_eq!(
        imported.projects[0].directory,
        imported.projects[1].directory
    );
    for (session, project) in [(1, 1), (2, 2), (3, 1), (4, 1)] {
        assert_eq!(
            imported.sessions[&SessionId::new(session).unwrap()],
            ProjectId::from_u128(project)
        );
    }
}
