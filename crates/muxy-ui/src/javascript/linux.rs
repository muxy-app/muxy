//! `JavaScriptCore` from `WebKitGTK`, bound once an extension is there to need
//! it, so the app starts on a desktop without the library.

#![allow(
    non_snake_case,
    non_upper_case_globals,
    reason = "Names follow the JavaScriptCore C API, which the macOS bindings share"
)]

use std::ffi::{c_double, c_int, c_uint};
use std::sync::OnceLock;

use libloading::Library;

use crate::tr;

/// The library `WebKitGTK` 4.1 links itself, so pages and scripts share one engine.
const LIBRARY: &str = "libjavascriptcoregtk-4.1.so.0";

#[repr(C)]
pub(super) struct OpaqueJSContext {
    _private: [u8; 0],
}
#[repr(C)]
pub(super) struct OpaqueJSString {
    _private: [u8; 0],
}
#[repr(C)]
pub(super) struct OpaqueJSClass {
    _private: [u8; 0],
}
#[repr(C)]
pub(super) struct OpaqueJSValue {
    _private: [u8; 0],
}

pub(super) type JSContextRef = *const OpaqueJSContext;
pub(super) type JSGlobalContextRef = *mut OpaqueJSContext;
pub(super) type JSStringRef = *mut OpaqueJSString;
pub(super) type JSClassRef = *mut OpaqueJSClass;
pub(super) type JSValueRef = *const OpaqueJSValue;
pub(super) type JSObjectRef = *mut OpaqueJSValue;
pub(super) type JSObjectCallAsFunctionCallback = Option<
    unsafe extern "C-unwind" fn(
        JSContextRef,
        JSObjectRef,
        JSObjectRef,
        usize,
        *mut JSValueRef,
        *mut JSValueRef,
    ) -> JSValueRef,
>;

pub(super) const kJSPropertyAttributeReadOnly: c_uint = 1 << 1;
pub(super) const kJSPropertyAttributeDontDelete: c_uint = 1 << 3;

/// Declares the functions bound from the library, and a caller for each.
macro_rules! functions {
    ($(fn $name:ident($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        struct Api {
            $($name: unsafe extern "C" fn($($kind),*) $(-> $result)?,)*
            _library: Library,
        }

        impl Api {
            fn bind(library: Library) -> Result<Self, libloading::Error> {
                // SAFETY: Each symbol is the C function of that name, whose
                // signature is declared here as the library's headers give it.
                unsafe {
                    $(let $name = *library.get(concat!(stringify!($name), "\0").as_bytes())?;)*
                    Ok(Self {
                        $($name,)*
                        _library: library,
                    })
                }
            }
        }

        $(
            /// # Safety
            ///
            /// [`load`] has succeeded, and the arguments are what the C
            /// function of this name requires.
            pub(super) unsafe fn $name($($argument: $kind),*) $(-> $result)? {
                // SAFETY: Upheld by the caller.
                unsafe { (api().$name)($($argument),*) }
            }
        )*
    };
}

functions! {
    fn JSCheckScriptSyntax(context: JSContextRef, script: JSStringRef, source: JSStringRef, line: c_int, exception: *mut JSValueRef) -> bool;
    fn JSGlobalContextCreate(class: JSClassRef) -> JSGlobalContextRef;
    fn JSGlobalContextRelease(context: JSGlobalContextRef);
    fn JSContextGetGlobalObject(context: JSContextRef) -> JSObjectRef;
    fn JSEvaluateScript(
        context: JSContextRef,
        script: JSStringRef,
        this: JSObjectRef,
        source: JSStringRef,
        line: c_int,
        exception: *mut JSValueRef
    ) -> JSValueRef;
    fn JSObjectMakeFunctionWithCallback(
        context: JSContextRef,
        name: JSStringRef,
        callback: JSObjectCallAsFunctionCallback
    ) -> JSObjectRef;
    fn JSObjectSetProperty(
        context: JSContextRef,
        object: JSObjectRef,
        name: JSStringRef,
        value: JSValueRef,
        attributes: c_uint,
        exception: *mut JSValueRef
    );
    fn JSStringCreateWithCharacters(characters: *const u16, length: usize) -> JSStringRef;
    fn JSStringGetCharactersPtr(string: JSStringRef) -> *const u16;
    fn JSStringGetLength(string: JSStringRef) -> usize;
    fn JSStringRelease(string: JSStringRef);
    fn JSValueIsNumber(context: JSContextRef, value: JSValueRef) -> bool;
    fn JSValueMakeString(context: JSContextRef, string: JSStringRef) -> JSValueRef;
    fn JSValueToNumber(
        context: JSContextRef,
        value: JSValueRef,
        exception: *mut JSValueRef
    ) -> c_double;
    fn JSValueToStringCopy(
        context: JSContextRef,
        value: JSValueRef,
        exception: *mut JSValueRef
    ) -> JSStringRef;
}

/// Asked, on the script's thread, whether to end a script that has run for the
/// whole time limit. `false` lets it run for another period.
pub(super) type JSShouldTerminateCallback =
    Option<unsafe extern "C" fn(JSContextRef, *mut std::ffi::c_void) -> bool>;

type JSContextGroupRef = *const std::ffi::c_void;

/// The engine's watchdog, from a private header. It is bound on its own, so a
/// library without it still runs scripts.
struct Watchdog {
    group: unsafe extern "C" fn(JSContextRef) -> JSContextGroupRef,
    set_limit: unsafe extern "C" fn(
        JSContextGroupRef,
        c_double,
        JSShouldTerminateCallback,
        *mut std::ffi::c_void,
    ),
    _library: Library,
}

static WATCHDOG: OnceLock<Option<Watchdog>> = OnceLock::new();

/// Asks `callback` with `data` every `limit` seconds a script in `context`'s
/// group keeps running. Answers whether the engine has a watchdog to ask.
///
/// # Safety
///
/// [`load`] has succeeded, `context` is live, and `data` stays valid for
/// `callback` while the group runs script.
pub(super) unsafe fn set_execution_time_limit(
    context: JSContextRef,
    limit: f64,
    callback: JSShouldTerminateCallback,
    data: *mut std::ffi::c_void,
) -> bool {
    let watchdog = WATCHDOG.get_or_init(|| {
        // SAFETY: The library is already loaded, and each symbol is the C
        // function of that name, with the signature its header gives it.
        unsafe {
            let library = Library::new(LIBRARY).ok()?;
            let group = *library.get(b"JSContextGetGroup\0").ok()?;
            let set_limit = *library.get(b"JSContextGroupSetExecutionTimeLimit\0").ok()?;
            Some(Watchdog {
                group,
                set_limit,
                _library: library,
            })
        }
    });
    let Some(watchdog) = watchdog else {
        return false;
    };
    // SAFETY: Upheld by the caller.
    unsafe {
        (watchdog.set_limit)((watchdog.group)(context), limit, callback, data);
    }
    true
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// Binds the library on first use. A desktop without it gets the reason.
pub(super) fn load() -> Result<(), String> {
    API.get_or_init(|| {
        // SAFETY: Loading runs the library's initializers, which only set up
        // the engine's own state.
        unsafe { Library::new(LIBRARY) }
            .and_then(Api::bind)
            .map_err(|error| {
                tr!(
                    "Extension scripts need WebKitGTK 4.1, which could not be loaded: %@",
                    error.to_string()
                )
                .to_string()
            })
    })
    .as_ref()
    .map(|_| ())
    .map_err(Clone::clone)
}

fn api() -> &'static Api {
    match API.get() {
        Some(Ok(api)) => api,
        _ => unreachable!("JavaScriptCore is called only after it loaded"),
    }
}
