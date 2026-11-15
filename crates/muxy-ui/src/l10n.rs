use std::borrow::Cow;

use gpui::SharedString;

pub use muxy_core::tr_key;

#[macro_export]
macro_rules! tr {
    ($($input:tt)+) => {
        $crate::l10n::shared($crate::l10n::core::tr!($($input)+))
    };
}

#[doc(hidden)]
pub use muxy_core as core;

pub fn translate(key: &'static str) -> SharedString {
    shared(muxy_core::l10n::translate(key))
}

#[doc(hidden)]
pub fn shared(text: Cow<'static, str>) -> SharedString {
    text.into()
}
