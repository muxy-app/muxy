#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
#[ignore = "Requires WebKitGTK and a display; runs in the Linux desktop guard"]
fn offscreen_views_disable_acceleration_before_realizing() {
    let _serial = super::super::tests::alone();
    let (send, receive) = mpsc::channel();
    post(move |runtime| {
        let runtime = runtime.unwrap();
        let g = runtime.g;
        let policies = unsafe {
            let library = libloading::Library::new("libwebkit2gtk-4.1.so.0").unwrap();
            let policy = library
                .get::<unsafe extern "C" fn(gpointer) -> c_int>(
                    b"webkit_settings_get_hardware_acceleration_policy\0",
                )
                .unwrap();
            let context = g.webkit_web_context_new_ephemeral();
            let policies = [false, true].map(|browser| {
                let (manager, view) = web_view(g, context, 0, browser);
                let policy = policy(g.webkit_web_view_get_settings(view));
                let window = offscreen_window(g, view, false, 0);
                g.gtk_widget_destroy(window);
                g.g_object_unref(manager);
                policy
            });
            g.g_object_unref(context);
            policies
        };
        send.send(policies).unwrap();
    });
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(20)).unwrap(),
        [ffi::HARDWARE_ACCELERATION_NEVER; 2],
        "extension and browser views must not request GL from an offscreen window"
    );
}
