//! The app language for UI code: `tr!` returns a `SharedString`, which GPUI
//! elements take directly. See `muxy_core::l10n` for the format rules.

use std::borrow::Cow;

use gpui::SharedString;

pub use muxy_core::tr_key;

/// Translates an English literal through the app language as a
/// `SharedString`: `tr!("Settings")`, `tr!("%lld changes", count)`.
#[macro_export]
macro_rules! tr {
    ($($input:tt)+) => {
        $crate::l10n::shared($crate::l10n::core::tr!($($input)+))
    };
}

#[doc(hidden)]
pub use muxy_core as core;

/// The active translation of a key kept elsewhere, such as a `tr_key!` label.
pub fn translate(key: &'static str) -> SharedString {
    shared(muxy_core::l10n::translate(key))
}

#[doc(hidden)]
pub fn shared(text: Cow<'static, str>) -> SharedString {
    text.into()
}
