//! `JavaScriptCore` contexts confined to dedicated script threads.

#![allow(
    deprecated,
    reason = "Use the C API without Objective-C JSContext wrappers"
)]

use std::cell::RefCell;
use std::ptr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use objc2_javascript_core::{
    JSContextGetGlobalObject, JSContextRef, JSEvaluateScript, JSGlobalContextCreate,
    JSGlobalContextRef, JSGlobalContextRelease, JSObjectMakeFunctionWithCallback, JSObjectRef,
    JSObjectSetProperty, JSStringCreateWithCharacters, JSStringGetCharactersPtr, JSStringGetLength,
    JSStringRef, JSStringRelease, JSValueMakeString, JSValueRef, JSValueToStringCopy,
    kJSPropertyAttributeDontDelete, kJSPropertyAttributeReadOnly,
};

#[derive(Debug)]
pub struct Call {
    pub request: String,
    pub reply: mpsc::SyncSender<String>,
}

#[derive(Debug)]
pub enum Event {
    Call(Call),
    Error(String),
}

#[derive(Debug)]
pub struct Script {
    input: mpsc::Sender<String>,
    cancelled: Arc<AtomicBool>,
}

impl Script {
    pub fn start(source: String) -> std::io::Result<(Self, async_channel::Receiver<Event>)> {
        let (input, scripts) = mpsc::channel::<String>();
        let (events, receiver) = async_channel::bounded(64);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        std::thread::Builder::new()
            .name("extension-script".into())
            .spawn(move || {
                HOST.with(|host| {
                    *host.borrow_mut() = Some(Host {
                        events: events.clone(),
                        cancelled: stop.clone(),
                    });
                });
                let context = Context::new();
                if let Err(error) = context.evaluate(&source) {
                    let _ = events.try_send(Event::Error(error));
                }
                while !stop.load(Ordering::Acquire) {
                    match scripts.recv_timeout(Duration::from_millis(16)) {
                        Ok(script) => {
                            if let Err(error) = context.evaluate(&script) {
                                let _ = events.try_send(Event::Error(error));
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let _ = context.evaluate("globalThis.__muxyTick?.()");
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                HOST.with(|host| host.borrow_mut().take());
            })?;
        Ok((Self { input, cancelled }, receiver))
    }

    pub fn evaluate(&self, source: String) {
        let _ = self.input.send(source);
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

struct Host {
    events: async_channel::Sender<Event>,
    cancelled: Arc<AtomicBool>,
}

thread_local! { static HOST: RefCell<Option<Host>> = const { RefCell::new(None) }; }

struct JsString(JSStringRef);

impl JsString {
    fn new(text: &str) -> Self {
        let text: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: JavaScriptCore copies this valid UTF-16 slice.
        Self(unsafe { JSStringCreateWithCharacters(text.as_ptr(), text.len()) })
    }

    fn text(&self) -> String {
        // SAFETY: The string is retained for the lifetime of this wrapper.
        unsafe {
            let length = JSStringGetLength(self.0);
            if length == 0 {
                return String::new();
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(
                JSStringGetCharactersPtr(self.0),
                length,
            ))
        }
    }
}

impl Drop for JsString {
    fn drop(&mut self) {
        // SAFETY: This wrapper owns exactly one retained JavaScriptCore string.
        unsafe {
            JSStringRelease(self.0);
        }
    }
}

struct Context(JSGlobalContextRef);

impl Context {
    fn new() -> Self {
        // SAFETY: Context and all its values remain on this thread. A null class uses the default global object.
        unsafe {
            let context = Self(JSGlobalContextCreate(ptr::null_mut()));
            let name = JsString::new("__muxyNative");
            let callback = JSObjectMakeFunctionWithCallback(context.0, name.0, Some(dispatch));
            JSObjectSetProperty(
                context.0,
                JSContextGetGlobalObject(context.0),
                name.0,
                callback,
                kJSPropertyAttributeReadOnly | kJSPropertyAttributeDontDelete,
                ptr::null_mut(),
            );
            context
        }
    }

    fn evaluate(&self, source: &str) -> Result<(), String> {
        let source = JsString::new(source);
        let mut exception = ptr::null();
        // SAFETY: Context and strings are live on the owning thread; exceptions are inspected before the next evaluation.
        unsafe {
            JSEvaluateScript(
                self.0,
                source.0,
                ptr::null_mut(),
                ptr::null_mut(),
                1,
                &raw mut exception,
            );
            if exception.is_null() {
                Ok(())
            } else {
                Err(value_text(self.0, exception))
            }
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: No values escape this thread, and it has stopped evaluating scripts.
        unsafe {
            JSGlobalContextRelease(self.0);
        }
    }
}

unsafe fn value_text(context: JSContextRef, value: JSValueRef) -> String {
    // SAFETY: The callback/evaluator supplies a live context and value.
    let string = unsafe { JSValueToStringCopy(context, value, ptr::null_mut()) };
    if string.is_null() {
        String::new()
    } else {
        JsString(string).text()
    }
}

unsafe extern "C-unwind" fn dispatch(
    context: JSContextRef,
    _: JSObjectRef,
    _: JSObjectRef,
    count: usize,
    arguments: *mut JSValueRef,
    _: *mut JSValueRef,
) -> JSValueRef {
    let response = if count == 1 {
        // SAFETY: JavaScriptCore provides `count` valid arguments for this callback.
        let request = unsafe { value_text(context, *arguments) };
        HOST.with(|host| {
            let host = host.borrow();
            let host = host.as_ref()?;
            if request.len() > 8 * 1024 * 1024 || host.cancelled.load(Ordering::Acquire) {
                return None;
            }
            let (reply, result) = mpsc::sync_channel(1);
            host.events
                .try_send(Event::Call(Call { request, reply }))
                .ok()?;
            loop {
                match result.recv_timeout(Duration::from_millis(50)) {
                    Ok(response) => return Some(response),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if host.cancelled.load(Ordering::Acquire) =>
                    {
                        return None;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                }
            }
        })
    } else {
        None
    };
    let response = JsString::new(
        response
            .as_deref()
            .unwrap_or(r#"{"ok":false,"error":"script is no longer active"}"#),
    );
    // SAFETY: JavaScriptCore copies the live string into the callback's context.
    unsafe { JSValueMakeString(context, response.0) }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    #[test]
    fn synchronous_host_calls_and_later_callbacks_share_the_script_context() {
        let (script, events) =
            Script::start("globalThis.answer = JSON.parse(__muxyNative('first')).value;".into())
                .unwrap();
        let Event::Call(call) = events.recv_blocking().unwrap() else {
            panic!("expected host call")
        };
        assert_eq!(call.request, "first");
        call.reply.send(r#"{"ok":true,"value":42}"#.into()).unwrap();
        script.evaluate("__muxyNative(String(answer + 1))".into());
        let Event::Call(call) = events.recv_blocking().unwrap() else {
            panic!("expected callback")
        };
        assert_eq!(call.request, "43");
        call.reply.send("null".into()).unwrap();
    }
}
