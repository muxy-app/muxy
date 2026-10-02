//! Unicode terminal-width helpers.

use crate::ffi;

/// Returns the terminal display width of a Unicode codepoint in terminal grid cells.
///
/// This is the same width table the terminal itself uses when laying out
/// printed text, so callers can predict column layout that exactly matches what
/// the terminal will do when the text is actually written to it.
///
/// Returns 0 for zero-width characters, 2 for wide characters, and 1 for
/// everything else. This operates on a single character only and therefore
/// cannot account for grapheme-cluster-level width rules. For cluster-accurate
/// widths, use [`grapheme_width`]. Summing per-character widths is only correct
/// when mode 2027, grapheme clustering, is disabled.
///
/// This function is pure, allocates nothing, and is thread-safe.
#[must_use]
pub fn codepoint_width(codepoint: char) -> u8 {
    unsafe { ffi::ghostty_unicode_codepoint_width(u32::from(codepoint)) }
}

/// Measures the terminal display width of the first grapheme cluster in a sequence.
///
/// This uses the exact same grapheme segmentation and cluster width rules the
/// terminal itself uses when printing text with grapheme clustering enabled,
/// mode 2027, so callers can predict column layout that exactly matches what
/// the terminal will do when the text is actually written to it. Unlike
/// [`codepoint_width`], this accounts for cluster-level rules: emoji variation
/// selectors, ZWJ sequences, combining marks, and skin tone modifiers.
///
/// Reads characters until the terminal would consider the grapheme cluster
/// complete and returns the number of characters consumed with the cluster's
/// total width in cells. Returns `(0, 0)` if and only if the input is empty.
///
/// This is not a streaming API. The provided sequence must contain a complete
/// first grapheme cluster, or the logical end of the string. If input arrives in
/// chunks, keep buffering while this function consumes all available characters
/// and the stream may still continue; a later character could still extend the
/// cluster and change its width.
///
/// Mode dependence: this models mode 2027, grapheme clustering, enabled. When
/// mode 2027 is disabled, clusters never combine and variation selectors never
/// change width; predict layout in that case by summing [`codepoint_width`] over
/// each character instead.
#[must_use]
pub fn grapheme_width(chars: &[char]) -> (usize, u8) {
    let codepoints = chars.iter().copied().map(u32::from).collect::<Vec<_>>();
    let mut width = 0;
    let consumed = unsafe {
        ffi::ghostty_unicode_grapheme_width(codepoints.as_ptr(), codepoints.len(), &raw mut width)
    };
    (consumed, width)
}
