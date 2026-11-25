use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};

#[gpui::test]
fn simulated_windows_report_unavailable_native_handles(cx: &mut gpui::TestAppContext) {
    cx.add_window(|window, _| {
        assert!(matches!(
            HasWindowHandle::window_handle(window),
            Err(HandleError::Unavailable)
        ));
        assert!(matches!(
            HasDisplayHandle::display_handle(window),
            Err(HandleError::Unavailable)
        ));
        gpui::Empty
    });
}
