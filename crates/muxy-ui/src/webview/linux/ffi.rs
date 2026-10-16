//! The parts of GTK 3 and `WebKitGTK` 4.1 the page backend calls, bound once
//! an extension is there to need them, so the app starts on a desktop
//! without them.

#![allow(
    non_camel_case_types,
    clippy::struct_field_names,
    reason = "Names follow the C libraries"
)]

use std::ffi::{c_char, c_double, c_int, c_uint, c_ulong, c_void};
use std::sync::OnceLock;

use libloading::Library;

use crate::tr;

pub(super) type gboolean = c_int;
pub(super) type gpointer = *mut c_void;

pub(super) type GType = usize;
pub(super) type GQuark = u32;

pub(super) const FALSE: gboolean = 0;
pub(super) const TRUE: gboolean = 1;
/// `G_PRIORITY_DEFAULT`.
pub(super) const PRIORITY_DEFAULT: c_int = 0;
/// `G_TYPE_STRING`.
pub(super) const TYPE_STRING: GType = 16 << 2;
/// `G_IO_ERROR_NOT_FOUND`.
pub(super) const IO_ERROR_NOT_FOUND: c_int = 1;

pub(super) const EVENT_MOTION_NOTIFY: c_int = 3;
pub(super) const EVENT_BUTTON_PRESS: c_int = 4;
pub(super) const EVENT_BUTTON_RELEASE: c_int = 7;
pub(super) const EVENT_KEY_PRESS: c_int = 8;
pub(super) const EVENT_KEY_RELEASE: c_int = 9;
pub(super) const EVENT_LEAVE_NOTIFY: c_int = 11;
pub(super) const EVENT_FOCUS_CHANGE: c_int = 12;
pub(super) const EVENT_SELECTION_CLEAR: c_int = 17;
pub(super) const EVENT_SCROLL: c_int = 31;
pub(super) const SCROLL_SMOOTH: c_int = 4;
/// `GDK_SELECTION_PRIMARY`: what is selected, as opposed to what was copied.
pub(super) const SELECTION_PRIMARY: usize = 1;

pub(super) const SHIFT_MASK: c_uint = 1 << 0;
pub(super) const CONTROL_MASK: c_uint = 1 << 2;
pub(super) const ALT_MASK: c_uint = 1 << 3;
pub(super) const BUTTON1_MASK: c_uint = 1 << 8;
pub(super) const BUTTON2_MASK: c_uint = 1 << 9;
pub(super) const BUTTON3_MASK: c_uint = 1 << 10;
pub(super) const SUPER_MASK: c_uint = 1 << 26;

pub(super) const CAIRO_FORMAT_ARGB32: c_int = 0;
pub(super) const CAIRO_FORMAT_RGB24: c_int = 1;

pub(super) const LOAD_STARTED: c_int = 0;
pub(super) const LOAD_COMMITTED: c_int = 2;
pub(super) const LOAD_FINISHED: c_int = 3;
pub(super) const HARDWARE_ACCELERATION_NEVER: c_int = 2;
pub(super) const POLICY_NAVIGATION_ACTION: c_int = 0;
pub(super) const POLICY_RESPONSE: c_int = 2;
pub(super) const INJECT_ALL_FRAMES: c_int = 0;
pub(super) const INJECT_TOP_FRAME: c_int = 1;
/// `WEBKIT_USER_STYLE_LEVEL_USER`: above anything a page declares.
pub(super) const STYLE_LEVEL_USER: c_int = 0;
/// `WEBKIT_SCRIPT_DIALOG_BEFORE_UNLOAD_CONFIRM`.
pub(super) const DIALOG_BEFORE_UNLOAD: c_int = 3;
pub(super) const INJECT_AT_DOCUMENT_START: c_int = 0;
pub(super) const HIT_LINK: c_uint = 1 << 2;
pub(super) const HIT_EDITABLE: c_uint = 1 << 5;
/// `WEBKIT_NETWORK_ERROR_CANCELLED`.
pub(super) const NETWORK_ERROR_CANCELLED: c_int = 302;
/// `WEBKIT_POLICY_ERROR_FRAME_LOAD_INTERRUPTED_BY_POLICY_CHANGE`.
pub(super) const POLICY_ERROR_INTERRUPTED: c_int = 102;
/// `SOUP_MESSAGE_HEADERS_RESPONSE`.
pub(super) const SOUP_HEADERS_RESPONSE: c_int = 1;

#[repr(C)]
pub(super) struct GError {
    pub(super) domain: GQuark,
    pub(super) code: c_int,
    pub(super) message: *mut c_char,
}

#[repr(C)]
pub(super) struct GValue {
    kind: GType,
    data: [u64; 2],
}

impl GValue {
    pub(super) const EMPTY: Self = Self {
        kind: 0,
        data: [0; 2],
    };
}

#[repr(C)]
pub(super) struct GdkRGBA {
    pub(super) red: c_double,
    pub(super) green: c_double,
    pub(super) blue: c_double,
    pub(super) alpha: c_double,
}

#[repr(C)]
pub(super) struct GdkRectangle {
    pub(super) x: c_int,
    pub(super) y: c_int,
    pub(super) width: c_int,
    pub(super) height: c_int,
}

#[repr(C)]
pub(super) struct GdkKeymapKey {
    pub(super) keycode: c_uint,
    pub(super) group: c_int,
    pub(super) level: c_int,
}

/// The fields every `GdkEvent` starts with. Each event below repeats them,
/// because C lays the fields after them out without this struct's padding.
#[repr(C)]
pub(super) struct GdkEventAny {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
}

#[repr(C)]
pub(super) struct GdkEventKey {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) time: u32,
    pub(super) state: c_uint,
    pub(super) keyval: c_uint,
    pub(super) length: c_int,
    pub(super) string: *mut c_char,
    pub(super) hardware_keycode: u16,
    pub(super) group: u8,
    pub(super) is_modifier: u8,
}

#[repr(C)]
pub(super) struct GdkEventButton {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) time: u32,
    pub(super) x: c_double,
    pub(super) y: c_double,
    pub(super) axes: *mut c_double,
    pub(super) state: c_uint,
    pub(super) button: c_uint,
    pub(super) device: gpointer,
    pub(super) x_root: c_double,
    pub(super) y_root: c_double,
}

#[repr(C)]
pub(super) struct GdkEventMotion {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) time: u32,
    pub(super) x: c_double,
    pub(super) y: c_double,
    pub(super) axes: *mut c_double,
    pub(super) state: c_uint,
    pub(super) is_hint: i16,
    pub(super) device: gpointer,
    pub(super) x_root: c_double,
    pub(super) y_root: c_double,
}

#[repr(C)]
pub(super) struct GdkEventScroll {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) time: u32,
    pub(super) x: c_double,
    pub(super) y: c_double,
    pub(super) state: c_uint,
    pub(super) direction: c_int,
    pub(super) device: gpointer,
    pub(super) x_root: c_double,
    pub(super) y_root: c_double,
    pub(super) delta_x: c_double,
    pub(super) delta_y: c_double,
    pub(super) is_stop: c_uint,
}

#[repr(C)]
pub(super) struct GdkEventCrossing {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) subwindow: gpointer,
    pub(super) time: u32,
    pub(super) x: c_double,
    pub(super) y: c_double,
    pub(super) x_root: c_double,
    pub(super) y_root: c_double,
    pub(super) mode: c_int,
    pub(super) detail: c_int,
    pub(super) focus: gboolean,
    pub(super) state: c_uint,
}

#[repr(C)]
pub(super) struct GdkEventFocus {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) focused: i16,
}

#[repr(C)]
pub(super) struct GdkEventSelection {
    pub(super) kind: c_int,
    pub(super) window: gpointer,
    pub(super) send_event: i8,
    pub(super) selection: gpointer,
}

// The offsets GDK 3 documents for these events on 64-bit Linux.
const _: () = {
    use std::mem::{offset_of, size_of};
    assert!(offset_of!(GdkEventKey, time) == 20);
    assert!(offset_of!(GdkEventKey, hardware_keycode) == 48);
    assert!(size_of::<GdkEventKey>() == 56);
    assert!(offset_of!(GdkEventButton, x) == 24);
    assert!(offset_of!(GdkEventButton, device) == 56);
    assert!(size_of::<GdkEventButton>() == 80);
    assert!(offset_of!(GdkEventMotion, device) == 56);
    assert!(offset_of!(GdkEventScroll, device) == 48);
    assert!(offset_of!(GdkEventScroll, delta_y) == 80);
    assert!(offset_of!(GdkEventCrossing, time) == 32);
    assert!(offset_of!(GdkEventCrossing, state) == 84);
    assert!(offset_of!(GdkEventFocus, focused) == 18);
    assert!(offset_of!(GdkEventSelection, selection) == 24);
};

pub(super) type SourceFunc = unsafe extern "C" fn(gpointer) -> gboolean;
pub(super) type DestroyNotify = unsafe extern "C" fn(gpointer);
/// A signal handler, cast from its real signature as `G_CALLBACK` does.
pub(super) type Callback = unsafe extern "C" fn();
pub(super) type SchemeCallback = unsafe extern "C" fn(gpointer, gpointer);
pub(super) type EventFunc = unsafe extern "C" fn(gpointer, gpointer);
pub(super) type AsyncReadyCallback = unsafe extern "C" fn(gpointer, gpointer, gpointer);

/// Declares the functions bound from each library, and a caller for each.
macro_rules! libraries {
    ($($library:literal { $(fn $name:ident($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)* })*) => {
        pub(super) struct Api {
            $($($name: unsafe extern "C" fn($($kind),*) $(-> $result)?,)*)*
            _libraries: Vec<Library>,
        }

        impl Api {
            fn bind() -> Result<Self, libloading::Error> {
                let mut libraries = Vec::new();
                // SAFETY: Loading runs the libraries' initializers, which only
                // set up their own state. Each symbol is the C function of that
                // name, with the signature its headers give it.
                unsafe {
                    $(
                        let library = Library::new($library)?;
                        $(let $name = *library.get(concat!(stringify!($name), "\0").as_bytes())?;)*
                        libraries.push(library);
                    )*
                    Ok(Self {
                        $($($name,)*)*
                        _libraries: libraries,
                    })
                }
            }

            $($(
                /// # Safety
                ///
                /// The arguments are what the C function of this name
                /// requires, and it is called on the thread that runs GTK.
                #[allow(clippy::too_many_arguments)]
                pub(super) unsafe fn $name(&self, $($argument: $kind),*) $(-> $result)? {
                    // SAFETY: Upheld by the caller.
                    unsafe { (self.$name)($($argument),*) }
                }
            )*)*
        }
    };
}

libraries! {
    "libglib-2.0.so.0" {
        fn g_free(memory: gpointer);
        fn g_set_prgname(name: *const c_char);
        fn g_idle_add_full(
            priority: c_int,
            function: SourceFunc,
            data: gpointer,
            notify: Option<DestroyNotify>
        ) -> c_uint;
        fn g_bytes_new_with_free_func(
            data: *const c_void,
            size: usize,
            free: Option<DestroyNotify>,
            user_data: gpointer
        ) -> gpointer;
        fn g_bytes_unref(bytes: gpointer);
        fn g_error_new_literal(domain: GQuark, code: c_int, message: *const c_char) -> *mut GError;
        fn g_error_free(error: *mut GError);
    }
    "libgobject-2.0.so.0" {
        fn g_object_ref(object: gpointer) -> gpointer;
        fn g_object_unref(object: gpointer);
        fn g_object_new_with_properties(
            kind: GType,
            count: c_uint,
            names: *const *const c_char,
            values: *const GValue
        ) -> gpointer;
        fn g_value_init(value: *mut GValue, kind: GType) -> *mut GValue;
        fn g_value_set_object(value: *mut GValue, object: gpointer);
        fn g_value_set_string(value: *mut GValue, string: *const c_char);
        fn g_value_unset(value: *mut GValue);
        fn g_signal_stop_emission_by_name(instance: gpointer, signal: *const c_char);
        fn g_signal_connect_data(
            instance: gpointer,
            signal: *const c_char,
            handler: Callback,
            data: gpointer,
            destroy: gpointer,
            flags: c_uint
        ) -> c_ulong;
    }
    "libgio-2.0.so.0" {
        fn g_memory_input_stream_new_from_bytes(bytes: gpointer) -> gpointer;
        fn g_io_error_quark() -> GQuark;
    }
    "libcairo.so.2" {
        fn cairo_surface_flush(surface: gpointer);
        fn cairo_surface_map_to_image(surface: gpointer, extents: gpointer) -> gpointer;
        fn cairo_surface_unmap_image(surface: gpointer, image: gpointer);
        fn cairo_surface_status(surface: gpointer) -> c_int;
        fn cairo_image_surface_get_data(image: gpointer) -> *mut u8;
        fn cairo_image_surface_get_format(image: gpointer) -> c_int;
        fn cairo_image_surface_get_width(image: gpointer) -> c_int;
        fn cairo_image_surface_get_height(image: gpointer) -> c_int;
        fn cairo_image_surface_get_stride(image: gpointer) -> c_int;
    }
    "libgdk-3.so.0" {
        fn gdk_error_trap_push();
        fn gdk_event_handler_set(handler: EventFunc, data: gpointer, notify: Option<DestroyNotify>);
        fn gdk_event_new(kind: c_int) -> gpointer;
        fn gdk_event_free(event: gpointer);
        fn gdk_event_get_coords(event: gpointer, x: *mut c_double, y: *mut c_double) -> gboolean;
        fn gdk_event_set_device(event: gpointer, device: gpointer);
        fn gdk_display_get_default() -> gpointer;
        fn gdk_display_get_default_seat(display: gpointer) -> gpointer;
        fn gdk_seat_get_keyboard(seat: gpointer) -> gpointer;
        fn gdk_seat_get_pointer(seat: gpointer) -> gpointer;
        fn gdk_keymap_get_for_display(display: gpointer) -> gpointer;
        fn gdk_keymap_get_entries_for_keyval(
            keymap: gpointer,
            keyval: c_uint,
            keys: *mut *mut GdkKeymapKey,
            count: *mut c_int
        ) -> gboolean;
        fn gdk_unicode_to_keyval(character: u32) -> c_uint;
        fn gdk_keyval_from_name(name: *const c_char) -> c_uint;
        fn gdk_screen_get_rgba_visual(screen: gpointer) -> gpointer;
    }
    "libgtk-3.so.0" {
        fn gtk_disable_setlocale();
        fn gtk_init_check(count: *mut c_int, arguments: *mut *mut *mut c_char) -> gboolean;
        fn gtk_main();
        fn gtk_main_do_event(event: gpointer);
        fn gtk_offscreen_window_new() -> gpointer;
        fn gtk_offscreen_window_get_surface(window: gpointer) -> gpointer;
        fn gtk_container_add(container: gpointer, widget: gpointer);
        fn gtk_widget_show(widget: gpointer);
        fn gtk_widget_realize(widget: gpointer);
        fn gtk_widget_hide(widget: gpointer);
        fn gtk_widget_destroy(widget: gpointer);
        fn gtk_widget_set_size_request(widget: gpointer, width: c_int, height: c_int);
        fn gtk_window_resize(window: gpointer, width: c_int, height: c_int);
        fn gtk_window_get_focus(window: gpointer) -> gpointer;
        fn gtk_widget_get_allocated_width(widget: gpointer) -> c_int;
        fn gtk_widget_get_allocated_height(widget: gpointer) -> c_int;
        fn gtk_widget_translate_coordinates(source: gpointer, destination: gpointer, x: c_int, y: c_int, translated_x: *mut c_int, translated_y: *mut c_int) -> gboolean;
        fn gtk_widget_get_window(widget: gpointer) -> gpointer;
        fn gtk_widget_get_scale_factor(widget: gpointer) -> c_int;
        fn gtk_widget_grab_focus(widget: gpointer);
        fn gtk_widget_get_screen(widget: gpointer) -> gpointer;
        fn gtk_widget_set_visual(widget: gpointer, visual: gpointer);
        fn gtk_widget_set_app_paintable(widget: gpointer, paintable: gboolean);
    }
    "libsoup-3.0.so.0" {
        fn soup_message_headers_new(kind: c_int) -> gpointer;
        fn soup_message_headers_append(headers: gpointer, name: *const c_char, value: *const c_char);
    }
    "libjavascriptcoregtk-4.1.so.0" {
        fn jsc_value_get_context(value: gpointer) -> gpointer;
        fn jsc_context_get_exception(context: gpointer) -> gpointer;
        fn jsc_context_clear_exception(context: gpointer);
        fn jsc_value_new_from_json(context: gpointer, json: *const c_char) -> gpointer;
        fn jsc_value_to_json(value: gpointer, indent: c_uint) -> *mut c_char;
        fn jsc_value_to_string(value: gpointer) -> *mut c_char;
        fn jsc_value_is_object(value: gpointer) -> gboolean;
        fn jsc_value_is_string(value: gpointer) -> gboolean;
        fn jsc_value_object_get_property(value: gpointer, name: *const c_char) -> gpointer;
    }
    "libwebkit2gtk-4.1.so.0" {
        fn webkit_web_context_new_ephemeral() -> gpointer;
        fn webkit_web_context_get_cookie_manager(context: gpointer) -> gpointer;
        fn webkit_cookie_manager_set_persistent_storage(manager: gpointer, filename: *const c_char, kind: c_int);
        fn webkit_web_view_get_title(view: gpointer) -> *const c_char;
        fn webkit_web_view_get_estimated_load_progress(view: gpointer) -> c_double;
        fn webkit_web_view_can_go_back(view: gpointer) -> gboolean;
        fn webkit_web_view_can_go_forward(view: gpointer) -> gboolean;
        fn webkit_web_view_go_back(view: gpointer);
        fn webkit_web_view_go_forward(view: gpointer);
        fn webkit_web_view_reload(view: gpointer);
        fn webkit_web_view_get_settings(view: gpointer) -> gpointer;
        fn webkit_settings_set_hardware_acceleration_policy(settings: gpointer, policy: c_int);
        fn webkit_settings_set_enable_developer_extras(settings: gpointer, enabled: gboolean);
        fn webkit_web_view_get_inspector(view: gpointer) -> gpointer;
        fn webkit_web_inspector_get_web_view(inspector: gpointer) -> gpointer;
        fn webkit_web_inspector_is_attached(inspector: gpointer) -> gboolean;
        fn webkit_web_inspector_show(inspector: gpointer);
        fn webkit_web_inspector_close(inspector: gpointer);
        fn webkit_context_menu_get_n_items(menu: gpointer) -> c_uint;
        fn webkit_navigation_action_is_user_gesture(action: gpointer) -> gboolean;
        fn webkit_web_context_get_type() -> GType;
        fn webkit_web_view_get_type() -> GType;
        fn webkit_user_content_manager_get_type() -> GType;
        fn webkit_website_data_manager_get_type() -> GType;
        fn webkit_web_context_new() -> gpointer;
        fn webkit_web_context_new_with_website_data_manager(manager: gpointer) -> gpointer;
        fn webkit_web_context_set_sandbox_enabled(context: gpointer, enabled: gboolean);
        fn webkit_web_context_register_uri_scheme(
            context: gpointer,
            scheme: *const c_char,
            callback: SchemeCallback,
            data: gpointer,
            destroy: Option<DestroyNotify>
        );
        fn webkit_user_content_manager_new() -> gpointer;
        fn webkit_user_content_manager_register_script_message_handler_with_reply(
            manager: gpointer,
            name: *const c_char,
            world: *const c_char
        ) -> gboolean;
        fn webkit_user_content_manager_add_script(manager: gpointer, script: gpointer);
        fn webkit_user_content_manager_remove_all_scripts(manager: gpointer);
        fn webkit_user_script_new(
            source: *const c_char,
            frames: c_int,
            time: c_int,
            allow: *const *const c_char,
            block: *const *const c_char
        ) -> gpointer;
        fn webkit_user_script_unref(script: gpointer);
        fn webkit_user_script_new_for_world(source: *const c_char, frames: c_int, time: c_int, world: *const c_char, allow: *const *const c_char, block: *const *const c_char) -> gpointer;
        fn webkit_user_content_manager_add_style_sheet(manager: gpointer, sheet: gpointer);
        fn webkit_user_style_sheet_new(
            source: *const c_char,
            frames: c_int,
            level: c_int,
            allow: *const *const c_char,
            block: *const *const c_char
        ) -> gpointer;
        fn webkit_user_style_sheet_unref(sheet: gpointer);
        fn webkit_script_dialog_get_dialog_type(dialog: gpointer) -> c_int;
        fn webkit_script_dialog_confirm_set_confirmed(dialog: gpointer, confirmed: gboolean);
        fn webkit_web_view_load_uri(view: gpointer, uri: *const c_char);
        fn webkit_web_view_stop_loading(view: gpointer);
        fn webkit_web_view_get_uri(view: gpointer) -> *const c_char;
        fn webkit_web_view_evaluate_javascript(
            view: gpointer,
            script: *const c_char,
            length: isize,
            world: *const c_char,
            source: *const c_char,
            cancellable: gpointer,
            callback: Option<AsyncReadyCallback>,
            data: gpointer
        );
        fn webkit_web_view_evaluate_javascript_finish(
            view: gpointer,
            result: gpointer,
            error: *mut *mut GError
        ) -> gpointer;
        fn webkit_web_view_set_background_color(view: gpointer, color: *const GdkRGBA);
        fn webkit_web_view_set_zoom_level(view: gpointer, zoom: c_double);
        fn webkit_web_view_set_input_method_context(view: gpointer, context: gpointer);
        fn webkit_uri_scheme_request_get_uri(request: gpointer) -> *const c_char;
        fn webkit_uri_scheme_request_get_web_view(request: gpointer) -> gpointer;
        fn webkit_uri_scheme_request_finish_error(request: gpointer, error: *mut GError);
        fn webkit_uri_scheme_request_finish_with_response(request: gpointer, response: gpointer);
        fn webkit_uri_scheme_response_new(stream: gpointer, length: i64) -> gpointer;
        fn webkit_uri_scheme_response_set_content_type(response: gpointer, kind: *const c_char);
        fn webkit_uri_scheme_response_set_http_headers(response: gpointer, headers: gpointer);
        fn webkit_script_message_reply_ref(reply: gpointer) -> gpointer;
        fn webkit_script_message_reply_unref(reply: gpointer);
        fn webkit_script_message_reply_return_value(reply: gpointer, value: gpointer);
        fn webkit_script_message_reply_return_error_message(reply: gpointer, message: *const c_char);
        fn webkit_policy_decision_use(decision: gpointer);
        fn webkit_policy_decision_ignore(decision: gpointer);
        fn webkit_navigation_policy_decision_get_navigation_action(decision: gpointer) -> gpointer;
        fn webkit_navigation_action_get_request(action: gpointer) -> gpointer;
        fn webkit_uri_request_get_uri(request: gpointer) -> *const c_char;
        fn webkit_response_policy_decision_is_mime_type_supported(decision: gpointer) -> gboolean;
        fn webkit_network_error_quark() -> GQuark;
        fn webkit_policy_error_quark() -> GQuark;
        fn webkit_permission_request_deny(request: gpointer);
        fn webkit_file_chooser_request_cancel(request: gpointer);
        fn webkit_color_chooser_request_cancel(request: gpointer);
        fn webkit_hit_test_result_get_context(result: gpointer) -> c_uint;
        fn webkit_option_menu_get_n_items(menu: gpointer) -> c_uint;
        fn webkit_option_menu_get_item(menu: gpointer, index: c_uint) -> gpointer;
        fn webkit_option_menu_activate_item(menu: gpointer, index: c_uint);
        fn webkit_option_menu_close(menu: gpointer);
        fn webkit_option_menu_item_get_label(item: gpointer) -> *const c_char;
        fn webkit_option_menu_item_is_enabled(item: gpointer) -> gboolean;
        fn webkit_option_menu_item_is_selected(item: gpointer) -> gboolean;
        fn webkit_option_menu_item_is_group_label(item: gpointer) -> gboolean;
    }
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Binds the libraries on first use. A desktop without them gets the reason.
pub(super) fn load() -> Result<&'static Api, String> {
    API.get_or_init(|| {
        Api::bind().map_err(|error| {
            tr!(
                "Extension pages need WebKitGTK 4.1, which could not be loaded: %@",
                error.to_string()
            )
            .to_string()
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// The libraries, once [`load`] has bound them.
pub(super) fn loaded() -> Option<&'static Api> {
    API.get().and_then(|api| api.as_ref().ok())
}
