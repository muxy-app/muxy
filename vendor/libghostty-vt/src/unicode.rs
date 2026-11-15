use crate::ffi;

#[must_use]
pub fn codepoint_width(codepoint: char) -> u8 {
    unsafe { ffi::ghostty_unicode_codepoint_width(u32::from(codepoint)) }
}

#[must_use]
pub fn grapheme_width(chars: &[char]) -> (usize, u8) {
    let codepoints = chars.iter().copied().map(u32::from).collect::<Vec<_>>();
    let mut width = 0;
    let consumed = unsafe {
        ffi::ghostty_unicode_grapheme_width(codepoints.as_ptr(), codepoints.len(), &raw mut width)
    };
    (consumed, width)
}
