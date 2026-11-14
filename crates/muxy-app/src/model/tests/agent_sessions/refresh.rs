use super::*;
use crate::model::agent_sessions::Entry;
use gpui::ListOffset;
use muxy_app_core::agent_sessions::Catalog;

#[gpui::test]
fn refresh_keeps_selection_and_scroll_while_changed_files_wait_for_a_batch(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().expect("directory");
    let state = AppState::bootstrap().expect("state");
    let cwd = state.current_project().directory.clone();
    let claude = directory.path().join(".claude/projects/project");
    std::fs::create_dir_all(&claude).expect("claude directory");
    for index in 0..10 {
        let path = claude.join(format!("{index}.jsonl"));
        std::fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({"type":"user","sessionId":format!("claude-{index}"),"cwd":cwd})
            ),
        )
        .expect("fixture");
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("file")
            .set_len(1024 * 1024)
            .expect("large history");
    }
    let roots = Roots::from_home(directory.path());
    let mut catalog = Catalog::default();
    let first = catalog.scan(&roots, &cwd);
    let complete = catalog.scan(&roots, &cwd);
    assert!(first.pending);
    assert!(!complete.pending);
    let deferred = complete
        .sessions
        .iter()
        .find(|session| !first.sessions.iter().any(|first| first.id == session.id))
        .expect("deferred session");
    let key = (deferred.provider, deferred.id.clone());
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        model.agent_sessions.listing = complete;
        model.filter_agent_sessions(cx);
        model.agent_sessions.list.selected = Some(key.clone());
        let row = model.agent_sessions.list.entries.iter().position(|entry| matches!(entry, Entry::Session { key: candidate, .. } if candidate == &key)).expect("selected row");
        model.agent_sessions.list.state.scroll_to(ListOffset { item_ix: row, offset_in_item: px(7.0) });
        for session in &model.agent_sessions.listing.sessions {
            std::fs::OpenOptions::new().write(true).open(&session.path).expect("file").set_len(1024 * 1024 + 1).expect("changed history");
        }
        for pending in [true, false] {
            model.agent_sessions.listing = catalog.scan(&roots, &cwd);
            assert_eq!(model.agent_sessions.listing.pending, pending);
            model.filter_agent_sessions(cx);
            assert_eq!(model.agent_sessions.list.selected.as_ref(), Some(&key));
            assert_eq!(model.agent_sessions.listing.sessions.len(), 10);
            let offset = model.agent_sessions.list.state.logical_scroll_top();
            assert!(matches!(&model.agent_sessions.list.entries[offset.item_ix], Entry::Session { key: candidate, .. } if candidate == &key));
            assert_eq!(offset.offset_in_item, px(7.0));
        }
    });
}
