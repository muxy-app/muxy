use super::*;

#[gpui::test]
fn clear_frame_invalidates_frozen_history_selection_and_pending_pages(cx: &mut TestAppContext) {
    let pane = cx.new(|cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    pane.update(cx, |pane, cx| {
        prepare_mouse(pane);
        pane.grid.as_mut().unwrap().history_fresh = false;
        let request = pane
            .scroll
            .move_rows(1.0, pane.grid.as_ref().unwrap(), 3)
            .unwrap();
        pane.select_all(cx);
        assert!(pane.selection.is_some());
        assert!(pane.scroll.view.is_some());
        let frame = ScreenFrame {
            size: pane.grid.as_ref().unwrap().size,
            graphics: None,
            seq: 2,
            reset: true,
            rows: vec![row(0, "prompt")],
            cursor: pane.grid.as_ref().unwrap().cursor,
            modes: Modes::default(),
        };
        pane.apply(&frame, cx);
        assert!(pane.scroll.view.is_none());
        assert!(!pane.scroll.expects(request));
        assert!(pane.selection.is_none());
        assert!(pane.grid.as_ref().unwrap().history.is_empty());
    });
}
