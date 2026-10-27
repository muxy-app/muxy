use super::*;

#[gpui::test]
fn popovers_mark_the_area_they_cover(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.toggle_remote_popover(window, cx);
    });
    cx.run_until_parked();
    let popover = cx
        .debug_bounds("remote-servers-popover")
        .expect("remote servers popover");
    let covers = view.update(cx, |model, _| model.webviews.occlusions.borrow().clone());
    assert!(
        covers
            .iter()
            .any(|(owner, cover)| owner.is_none() && *cover == popover),
        "{covers:?} should cover {popover:?}"
    );
}
