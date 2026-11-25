use std::sync::mpsc;

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

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[cfg_attr(target_os = "macos", path = "javascript/macos.rs")]
#[cfg_attr(target_os = "linux", path = "javascript/linux.rs")]
mod api;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod jsc;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unsupported;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use jsc::{CAPABILITY, Script, is_async_function_body, probe};
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub use unsupported::{CAPABILITY, Script, is_async_function_body, probe};
