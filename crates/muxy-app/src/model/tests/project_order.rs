use super::*;
use muxy_app_core::settings::ProjectOrder;

fn fixture() -> (AppState, [ProjectId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = ["repo10", "repo2b", "Repo1", "repo2a"].map(|name| {
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.rename_project(id, name).expect("name");
        id
    });
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for (index, project) in stored["projects"]
        .as_array_mut()
        .expect("projects")
        .iter_mut()
        .enumerate()
    {
        project["created_at"] =
            serde_json::json!({"secs_since_epoch": index, "nanos_since_epoch": 0});
    }
    let mut state: AppState = serde_json::from_value(stored).expect("dated state");
    state.move_project(ids[0], 4).expect("manual order");
    state.select_project(state.home().id).expect("home");
    (state, ids)
}

fn parent_ids(model: &AppModel) -> Vec<ProjectId> {
    model
        .listed_parents()
        .iter()
        .map(|project| project.id)
        .collect()
}

#[gpui::test]
fn project_sort_date_added_keeps_undated_projects_stable_ahead_of_new_projects(
    cx: &mut TestAppContext,
) {
    let (state, [ten, two_b, one, two_a]) = fixture();
    let mut stored = serde_json::to_value(&state).expect("state JSON");
    for project in stored["projects"].as_array_mut().expect("projects") {
        if [two_a.to_string(), one.to_string()]
            .iter()
            .any(|id| project["id"].as_str() == Some(id))
        {
            project
                .as_object_mut()
                .expect("project")
                .remove("created_at");
        }
    }
    let (boot, _requests) = stub_boot(serde_json::from_value(stored).expect("legacy state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.appearance.sidebar_project_order = ProjectOrder::DateAdded;
        assert_eq!(
            parent_ids(model),
            [model.state.home().id, one, two_a, ten, two_b]
        );
    });
}
