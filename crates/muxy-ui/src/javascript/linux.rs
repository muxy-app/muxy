#![allow(
    non_snake_case,
    non_upper_case_globals,
    reason = "Names follow the JavaScriptCore C API, which the macOS bindings share"
)]

use std::ffi::{c_double, c_int, c_uint};
use std::sync::OnceLock;

use libloading::Library;

use crate::tr;

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

macro_rules! functions {
    ($(fn $name:ident($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        struct Api {
            $($name: unsafe extern "C" fn($($kind),*) $(-> $result)?,)*
            _library: Library,
        }

        impl Api {
            fn bind(library: Library) -> Result<Self, libloading::Error> {
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
            pub(super) unsafe fn $name($($argument: $kind),*) $(-> $result)? {
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

pub(super) type JSShouldTerminateCallback =
    Option<unsafe extern "C" fn(JSContextRef, *mut std::ffi::c_void) -> bool>;

type JSContextGroupRef = *const std::ffi::c_void;

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

pub(super) unsafe fn set_execution_time_limit(
    context: JSContextRef,
    limit: f64,
    callback: JSShouldTerminateCallback,
    data: *mut std::ffi::c_void,
) -> bool {
    let watchdog = WATCHDOG.get_or_init(|| unsafe {
        let library = Library::new(LIBRARY).ok()?;
        let group = *library.get(b"JSContextGetGroup\0").ok()?;
        let set_limit = *library.get(b"JSContextGroupSetExecutionTimeLimit\0").ok()?;
        Some(Watchdog {
            group,
            set_limit,
            _library: library,
        })
    });
    let Some(watchdog) = watchdog else {
        return false;
    };
    unsafe {
        (watchdog.set_limit)((watchdog.group)(context), limit, callback, data);
    }
    true
}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

pub(super) fn load() -> Result<(), String> {
    API.get_or_init(|| {
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
