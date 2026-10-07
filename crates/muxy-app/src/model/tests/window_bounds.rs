use super::*;
use std::os::unix::fs::MetadataExt;

#[gpui::test]
fn tab_changes_and_quit_flush_pending_bounds_without_stale_saves(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(900.0), px(650.0)));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(model.bounds_save.is_some());
        model.new_tab(cx);
        assert!(model.bounds_save.is_none());
        assert_eq!(store::load(&model.path).expect("saved tab"), model.state);
    });
    let path = view.read_with(cx, |model, _| model.path.clone());
    let inode = std::fs::metadata(&path).expect("state").ino();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(std::fs::metadata(&path).expect("state").ino(), inode);

    cx.simulate_resize(size(px(950.0), px(700.0)));
    cx.run_until_parked();
    view.update(cx, AppModel::quit);
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush))
    );
    view.update(cx, |model, cx| {
        model.receive(
            (
                ServerId::local(),
                model.servers.local.generation,
                Update::Flushed,
            ),
            cx,
        );
        assert!(model.bounds_save.is_none());
        assert_eq!(
            store::load(&model.path).expect("saved before quit"),
            model.state
        );
    });
}
