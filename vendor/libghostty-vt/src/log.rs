use std::sync::{PoisonError, RwLock};

use crate::{error::Result, ffi};

pub trait Logger: Send + Sync + 'static {
    fn log(&self, level: Level, scope: &str, message: &str);
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LogStderr;
impl Logger for LogStderr {
    fn log(&self, level: Level, scope: &str, message: &str) {
        unsafe {
            ffi::ghostty_sys_log_stderr(
                std::ptr::null_mut(),
                level.into(),
                scope.as_ptr(),
                scope.len(),
                message.as_ptr(),
                message.len(),
            );
        }
    }
}

#[cfg(feature = "tracing")]
const TRACING_TARGET: &str = "libghostty_vt::ghostty";

#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TracingLogger;
#[cfg(feature = "tracing")]
impl Logger for TracingLogger {
    fn log(&self, level: Level, scope: &str, message: &str) {
        match level {
            Level::Error => {
                tracing::error!(target: TRACING_TARGET, ghostty_scope = scope, "{message}");
            }
            Level::Warning => {
                tracing::warn!(target: TRACING_TARGET, ghostty_scope = scope, "{message}");
            }
            Level::Info => {
                tracing::info!(target: TRACING_TARGET, ghostty_scope = scope, "{message}");
            }
            Level::Debug => {
                tracing::debug!(target: TRACING_TARGET, ghostty_scope = scope, "{message}");
            }
        }
    }
}

#[cfg(feature = "log")]
impl<L: log::Log + 'static> Logger for L {
    fn log(&self, level: Level, scope: &str, message: &str) {
        let level = match level {
            Level::Error => log::Level::Error,
            Level::Warning => log::Level::Warn,
            Level::Info => log::Level::Info,
            Level::Debug => log::Level::Debug,
        };
        let args = format_args!("{message}");
        let record = log::Record::builder()
            .level(level)
            .target(scope)
            .args(args)
            .build();

        log::Log::log(&self, &record);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Level {
    Error = ffi::SysLogLevel::ERROR,
    Warning = ffi::SysLogLevel::WARNING,
    Info = ffi::SysLogLevel::INFO,
    Debug = ffi::SysLogLevel::DEBUG,
}

static LOGGER: RwLock<Option<Box<dyn Logger>>> = RwLock::new(None);

pub unsafe fn set_logger(f: Option<Box<dyn Logger>>) -> Result<()> {
    unsafe extern "C" fn callback(
        _userdata: *mut std::ffi::c_void,
        level: ffi::SysLogLevel::Type,
        scope: *const u8,
        scope_len: usize,
        message: *const u8,
        message_len: usize,
    ) {
        let scope = unsafe { std::slice::from_raw_parts(scope, scope_len) };
        let Ok(scope) = std::str::from_utf8(scope) else {
            return;
        };
        let message = unsafe { std::slice::from_raw_parts(message, message_len) };
        let Ok(message) = std::str::from_utf8(message) else {
            return;
        };
        let Ok(level) = Level::try_from(level) else {
            return;
        };

        let log = LOGGER.read().unwrap_or_else(PoisonError::into_inner);
        let Some(log) = log.as_deref() else {
            return;
        };
        log.log(level, scope, message);
    }

    let ptr: ffi::SysLogFn = match f {
        None => None,
        Some(_) => Some(callback),
    };
    let _old = std::mem::replace(
        &mut *LOGGER.write().unwrap_or_else(PoisonError::into_inner),
        f,
    );
    crate::sys_set(
        ffi::SysOption::LOG,
        ptr.map_or(std::ptr::null(), |p| p as *const std::ffi::c_void),
    )
}
