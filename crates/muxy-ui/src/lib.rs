//! Reusable GPUI components. Shared shortcut definitions come from muxy-core;
//! app settings resolve them through the registration interface.

pub mod assets;
pub mod command_palette;
pub mod components;
pub mod controls;
#[cfg(target_os = "macos")]
pub mod dialog;
pub mod form;
pub mod icon;
#[cfg(target_os = "macos")]
pub mod keyboard;
pub mod motion;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod native_scroll;
pub mod navigation;
pub mod panel;
pub mod picker;
pub mod popover;
pub mod scrollbar;
pub mod shortcuts;
pub mod symbols;
pub mod text_input;
pub mod theme;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod window_drag;

#[allow(unsafe_code)]
pub mod quick_terminal;

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "Native notification delegate and Objective-C initialization"
)]
pub mod notifications;
