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

#[allow(
    clippy::unnecessary_wraps,
    reason = "The Linux binding, which shares this signature, can fail"
)]
pub(super) fn load() -> Result<(), String> {
    Ok(())
}

pub(super) type JSShouldTerminateCallback =
    Option<unsafe extern "C" fn(JSContextRef, *mut std::ffi::c_void) -> bool>;

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

pub(super) unsafe fn set_execution_time_limit(
    context: JSContextRef,
    limit: f64,
    callback: JSShouldTerminateCallback,
    data: *mut std::ffi::c_void,
) -> bool {
    unsafe {
        JSContextGroupSetExecutionTimeLimit(JSContextGetGroup(context), limit, callback, data);
    }
    true
}
