#![cfg_attr(
    target_os = "macos",
    allow(
        deprecated,
        reason = "Use the C API without Objective-C JSContext wrappers"
    )
)]

use std::cell::RefCell;
use std::ptr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use muxy_core::capability::Capability;

use super::api::{
    self, JSCheckScriptSyntax, JSContextGetGlobalObject, JSContextRef, JSEvaluateScript,
    JSGlobalContextCreate, JSGlobalContextRef, JSGlobalContextRelease,
    JSObjectMakeFunctionWithCallback, JSObjectRef, JSObjectSetProperty,
    JSStringCreateWithCharacters, JSStringGetCharactersPtr, JSStringGetLength, JSStringRef,
    JSStringRelease, JSValueIsNumber, JSValueMakeString, JSValueRef, JSValueToNumber,
    JSValueToStringCopy, kJSPropertyAttributeDontDelete, kJSPropertyAttributeReadOnly,
};
use super::{Call, Event};

const RETRY: Duration = Duration::from_millis(16);
const MAX_DELAY_MS: f64 = 86_400_000.0;
const WATCHDOG_PERIOD: f64 = 0.1;

pub const CAPABILITY: Capability = Capability::Supported;

pub fn probe() -> Capability {
    match api::load() {
        Ok(()) => Capability::Supported,
        Err(reason) => Capability::Unavailable(reason),
    }
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
                if let Err(error) = api::load() {
                    let _ = events.try_send(Event::Error(error));
                    return;
                }
                HOST.with(|host| {
                    *host.borrow_mut() = Some(Host {
                        events: events.clone(),
                        cancelled: stop.clone(),
                    });
                });
                let context = Context::new(&stop);
                if let Err(error) = context.evaluate(&source) {
                    let _ = events.try_send(Event::Error(error));
                }
                let mut wait = context.tick();
                while !stop.load(Ordering::Acquire) {
                    let next = match wait {
                        Some(wait) => scripts.recv_timeout(wait),
                        None => scripts
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                    };
                    match next {
                        Ok(script) => {
                            if let Err(error) = context.evaluate(&script) {
                                let _ = events.try_send(Event::Error(error));
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    wait = context.tick();
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

pub fn is_async_function_body(body: &str) -> bool {
    if api::load().is_err() {
        return false;
    }
    let program = JsString::new(&format!("(async () => {{\n{body}\n}});"));
    unsafe {
        let context = JSGlobalContextCreate(ptr::null_mut());
        let parses = JSCheckScriptSyntax(context, program.0, ptr::null_mut(), 1, ptr::null_mut());
        JSGlobalContextRelease(context);
        parses
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
        Self(unsafe { JSStringCreateWithCharacters(text.as_ptr(), text.len()) })
    }

    fn text(&self) -> String {
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
        unsafe {
            JSStringRelease(self.0);
        }
    }
}

struct Context(JSGlobalContextRef);

impl Context {
    fn new(stop: &Arc<AtomicBool>) -> Self {
        unsafe {
            let context = Self(JSGlobalContextCreate(ptr::null_mut()));
            api::set_execution_time_limit(
                context.0,
                WATCHDOG_PERIOD,
                Some(should_terminate),
                Arc::as_ptr(stop).cast_mut().cast(),
            );
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
        self.value(source).map(|_| ())
    }

    fn value(&self, source: &str) -> Result<JSValueRef, String> {
        let source = JsString::new(source);
        let mut exception = ptr::null();
        unsafe {
            let value = JSEvaluateScript(
                self.0,
                source.0,
                ptr::null_mut(),
                ptr::null_mut(),
                1,
                &raw mut exception,
            );
            if exception.is_null() {
                Ok(value)
            } else {
                Err(value_text(self.0, exception))
            }
        }
    }

    fn tick(&self) -> Option<Duration> {
        let Ok(value) = self.value("globalThis.__muxyTick?.()") else {
            return Some(RETRY);
        };
        let milliseconds = unsafe {
            if value.is_null() || !JSValueIsNumber(self.0, value) {
                return None;
            }
            JSValueToNumber(self.0, value, ptr::null_mut())
        };
        (milliseconds >= 0.0)
            .then(|| Duration::from_secs_f64(milliseconds.min(MAX_DELAY_MS) / 1000.0))
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            JSGlobalContextRelease(self.0);
        }
    }
}

unsafe extern "C" fn should_terminate(context: JSContextRef, stop: *mut std::ffi::c_void) -> bool {
    if unsafe { &*stop.cast::<AtomicBool>() }.load(Ordering::Acquire) {
        return true;
    }
    unsafe {
        api::set_execution_time_limit(context, WATCHDOG_PERIOD, Some(should_terminate), stop);
    }
    false
}

unsafe fn value_text(context: JSContextRef, value: JSValueRef) -> String {
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
    unsafe { JSValueMakeString(context, response.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &'static str, stop_after: Option<Duration>) -> Option<bool> {
        let stop = Arc::new(AtomicBool::new(false));
        let (done, finished) = mpsc::channel();
        let flag = stop.clone();
        std::thread::spawn(move || {
            let context = Context::new(&flag);
            let _ = done.send(context.evaluate(source).is_err());
        });
        if let Some(after) = stop_after {
            std::thread::sleep(after);
            stop.store(true, Ordering::Release);
        }
        finished.recv_timeout(Duration::from_secs(10)).ok()
    }

    #[test]
    fn stopping_ends_a_script_that_never_returns() {
        if api::load().is_err() {
            return;
        }
        assert_eq!(
            run("for (;;) {}", Some(Duration::from_millis(300))),
            Some(true)
        );
    }

    #[test]
    fn a_long_script_that_is_not_stopped_runs_to_the_end() {
        if api::load().is_err() {
            return;
        }
        let source = "const end = Date.now() + 400; while (Date.now() < end) {}";
        assert_eq!(run(source, None), Some(false));
    }
}
