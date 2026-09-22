use super::*;
use crate::views::project_editor::{Field, logo::Cropper};
use gpui::{MouseButton, MouseDownEvent, MouseUpEvent, point};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let position = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn project_symbol_search_selects_and_removes_icons_without_touching_other_metadata(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state.add_project(std::env::temp_dir()).expect("project");
    let color = state.current_project().color.clone();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.open_project_editor(project, Field::Icon, point(px(20.0), px(40.0)), window, cx);
        });
    });
    cx.run_until_parked();
    let field = cx
        .debug_bounds("settings-field-project-symbol-search")
        .expect("search field");
    let grid = cx
        .debug_bounds("project-symbols-scroll")
        .expect("symbol grid");
    view.read_with(cx, |model, _| {
        assert_eq!(field.size.height, model.metrics.control_medium());
        assert!(grid.size.height > model.metrics.control_large() * 5.0);
    });
    assert!(grid.top() > field.bottom());
    cx.simulate_input("terminal fill");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(
            model.state.current_project().icon.as_deref(),
            Some("sf:terminal.fill")
        );
        let saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.project(project).expect("project").icon.as_deref(),
            Some("sf:terminal.fill")
        );
        assert_eq!(model.state.current_project().color, color);
    });
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.open_project_editor(project, Field::Icon, point(px(20.0), px(40.0)), window, cx);
        });
    });
    click(cx, "remove-project-icon");
    view.read_with(cx, |model, _| {
        assert!(model.state.current_project().icon.is_none());
    });
}

#[gpui::test]
fn project_logo_apply_cancel_and_remove_preserve_icon_and_release_cached_image(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state.add_project(std::env::temp_dir()).expect("project");
    state
        .set_project_icon(project, Some("sf:star.fill".into()))
        .expect("icon");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let source = image::RgbaImage::from_pixel(30, 20, image::Rgba([200, 80, 20, 255]));
    view.update(cx, |model, cx| {
        model.overlay = Some(Overlay::ProjectLogo(Cropper::new(
            project,
            source.clone(),
            cx,
        )));
        cx.notify();
    });
    cx.simulate_resize(size(px(360.0), px(300.0)));
    cx.run_until_parked();
    let dialog = cx.debug_bounds("project-logo-cropper").expect("cropper");
    assert!(dialog.left() >= px(0.0) && dialog.right() <= px(360.0));
    assert!(dialog.top() >= px(0.0) && dialog.bottom() <= px(300.0));
    click(cx, "logo-cancel");
    view.read_with(cx, |model, _| {
        assert!(model.state.current_project().logo.is_none());
    });
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.overlay = Some(Overlay::ProjectLogo(Cropper::new(project, source, cx)));
        cx.notify();
    });
    click(cx, "logo-apply");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.project_logos.contains_key(&project));
        assert!(
            store::load(&model.path)
                .expect("saved")
                .project(project)
                .expect("project")
                .logo
                .is_some()
        );
    });
    let cached = view.read_with(cx, |model, _| model.project_logos[&project].1.clone());
    let decoded = cx.update(|window, cx| {
        std::sync::Arc::downgrade(
            &cached
                .clone()
                .get_render_image(window, cx)
                .expect("decoded image"),
        )
    });
    view.update(cx, |model, cx| {
        let source = image::RgbaImage::from_pixel(20, 30, image::Rgba([10, 180, 200, 128]));
        model.overlay = Some(Overlay::ProjectLogo(Cropper::new(project, source, cx)));
        cx.notify();
    });
    click(cx, "logo-apply");
    assert!(decoded.upgrade().is_none());
    let replacement = view.read_with(cx, |model, _| model.project_logos[&project].1.clone());
    assert_ne!(cached.id(), replacement.id());
    let decoded_replacement = cx.update(|window, cx| {
        std::sync::Arc::downgrade(
            &replacement
                .clone()
                .get_render_image(window, cx)
                .expect("decoded replacement"),
        )
    });
    view.update(cx, |model, cx| {
        assert!(model.edit_project(|state| state.set_project_logo(project, None), cx));
    });
    cx.run_until_parked();
    assert!(decoded.upgrade().is_none());
    assert!(decoded_replacement.upgrade().is_none());
    view.read_with(cx, |model, _| {
        assert!(!model.project_logos.contains_key(&project));
        assert_eq!(
            model.state.current_project().icon.as_deref(),
            Some("sf:star.fill")
        );
        assert!(
            store::load(&model.path)
                .expect("saved")
                .project(project)
                .expect("project")
                .logo
                .is_none()
        );
    });
}
