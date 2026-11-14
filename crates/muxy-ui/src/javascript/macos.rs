//! The system's `JavaScriptCore`.

#![allow(
    deprecated,
    reason = "Use the C API without Objective-C JSContext wrappers"
)]

pub(super) use objc2_javascript_core::{
    JSCheckScriptSyntax, JSContextGetGlobalObject, JSContextRef, JSEvaluateScript,
    JSGlobalContextCreate, JSGlobalContextRef, JSGlobalContextRelease,
    JSObjectMakeFunctionWithCallback, JSObjectRef, JSObjectSetProperty,
    JSStringCreateWithCharacters, JSStringGetCharactersPtr, JSStringGetLength, JSStringRef,
    JSStringRelease, JSValueIsNumber, JSValueMakeString, JSValueRef, JSValueToNumber,
    JSValueToStringCopy, kJSPropertyAttributeDontDelete, kJSPropertyAttributeReadOnly,
};

/// The framework is part of the system, so there is nothing to load.
#[allow(
    clippy::unnecessary_wraps,
    reason = "The Linux binding, which shares this signature, can fail"
)]
pub(super) fn load() -> Result<(), String> {
    Ok(())
}

/// Asked, on the script's thread, whether to end a script that has run for the
/// whole time limit. `false` lets it run for another period.
pub(super) type JSShouldTerminateCallback =
    Option<unsafe extern "C" fn(JSContextRef, *mut std::ffi::c_void) -> bool>;

// The engine's watchdog, which the framework exports from a private header.
#[link(name = "JavaScriptCore", kind = "framework")]
unsafe extern "C" {
    fn JSContextGetGroup(context: JSContextRef) -> objc2_javascript_core::JSContextGroupRef;
    fn JSContextGroupSetExecutionTimeLimit(
        group: objc2_javascript_core::JSContextGroupRef,
        limit: f64,
        callback: JSShouldTerminateCallback,
        data: *mut std::ffi::c_void,
    );
}

/// Asks `callback` with `data` every `limit` seconds a script in `context`'s
/// group keeps running. Answers whether the engine has a watchdog to ask.
///
/// # Safety
///
/// `context` is live, and `data` stays valid for `callback` while the group
/// runs script.
pub(super) unsafe fn set_execution_time_limit(
    context: JSContextRef,
    limit: f64,
    callback: JSShouldTerminateCallback,
    data: *mut std::ffi::c_void,
) -> bool {
    // SAFETY: Upheld by the caller.
    unsafe {
        JSContextGroupSetExecutionTimeLimit(JSContextGetGroup(context), limit, callback, data);
    }
    true
}
