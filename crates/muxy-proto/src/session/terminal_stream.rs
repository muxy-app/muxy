use std::ops::Range;

pub const ALTERNATE_SCREEN_ENTER_SEQUENCES: [&[u8]; 3] =
    [b"\x1b[?47h", b"\x1b[?1047h", b"\x1b[?1049h"];
pub const ALTERNATE_SCREEN_LEAVE_SEQUENCES: [&[u8]; 3] =
    [b"\x1b[?47l", b"\x1b[?1047l", b"\x1b[?1049l"];
pub const SCREEN_CONTROL_TAIL_LENGTH: usize = 8;

pub fn safe_replay_start(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .position(|byte| matches!(byte, b'\n' | b'\r'))
        .map_or(0, |index| index + 1)
}

pub fn leading_safe_index(bytes: &[u8]) -> usize {
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if (0x80..=0xbf).contains(&byte) {
            index += 1;
            continue;
        }
        if byte == b']' {
            if !is_likely_bare_osc_body(bytes, index) {
                return index;
            }
            let Some(end) = osc_terminator(bytes, index + 1) else {
                return bytes.len();
            };
            index = end;
            continue;
        }
        if byte == b'[' {
            if !is_likely_bare_csi_fragment(bytes, index) {
                return index;
            }
            let Some(end) = csi_terminator(bytes, index + 1) else {
                return bytes.len();
            };
            index = end;
            continue;
        }
        return index;
    }
    index
}

pub fn trailing_safe_end(bytes: &[u8]) -> usize {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            index += 1;
            continue;
        }
        let Some(end) = escape_terminator(bytes, index) else {
            return trailing_utf8_safe_end(bytes, index);
        };
        index = end;
    }
    trailing_utf8_safe_end(bytes, bytes.len())
}

pub fn trailing_utf8_safe_end(bytes: &[u8], end: usize) -> usize {
    if end == 0 {
        return 0;
    }
    let mut sequence_start = end - 1;
    let mut continuation_count = 0;
    while sequence_start > 0
        && is_utf8_continuation(bytes[sequence_start])
        && continuation_count < 3
    {
        sequence_start -= 1;
        continuation_count += 1;
    }
    let Some(expected_length) = utf8_sequence_length(bytes[sequence_start]) else {
        return end;
    };
    let actual_length = end - sequence_start;
    if actual_length >= expected_length
        || !bytes[sequence_start + 1..end]
            .iter()
            .copied()
            .all(is_utf8_continuation)
    {
        return end;
    }
    sequence_start
}

pub fn next_alternate_screen_sequence(
    bytes: &[u8],
    from: usize,
    entering: bool,
) -> Option<Range<usize>> {
    let sequences = if entering {
        &ALTERNATE_SCREEN_ENTER_SEQUENCES
    } else {
        &ALTERNATE_SCREEN_LEAVE_SEQUENCES
    };
    sequences
        .iter()
        .filter_map(|sequence| first_range(sequence, bytes, from))
        .min_by_key(|range| range.start)
}

pub fn escape_terminator(bytes: &[u8], index: usize) -> Option<usize> {
    let next = index + 1;
    let byte = *bytes.get(next)?;
    match byte {
        b']' => osc_terminator(bytes, next + 1),
        b'P' | b'X' | b'^' | b'_' => string_control_terminator(bytes, next + 1),
        b'[' => csi_terminator(bytes, next + 1),
        0x20..=0x2f => bytes[next + 1..]
            .iter()
            .position(|byte| (0x30..=0x7e).contains(byte))
            .map(|offset| next + offset + 2),
        _ => Some(next + 1),
    }
}

fn is_likely_bare_osc_body(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index + 1;
    let mut saw_digit = false;
    while bytes.get(cursor).is_some_and(|byte| byte.is_ascii_digit()) {
        saw_digit = true;
        cursor += 1;
    }
    saw_digit && bytes.get(cursor) == Some(&b';')
}

fn is_likely_bare_csi_fragment(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index + 1)
        .is_some_and(|byte| (0x30..=0x3f).contains(byte) || (0x20..=0x2f).contains(byte))
}

fn osc_terminator(bytes: &[u8], from: usize) -> Option<usize> {
    let mut cursor = from;
    while cursor < bytes.len() {
        if bytes[cursor] == 0x07 {
            return Some(cursor + 1);
        }
        if bytes[cursor] == 0x1b && bytes.get(cursor + 1) == Some(&b'\\') {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

fn string_control_terminator(bytes: &[u8], from: usize) -> Option<usize> {
    let mut cursor = from;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == 0x1b && bytes[cursor + 1] == b'\\' {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

fn csi_terminator(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|byte| (0x40..=0x7e).contains(byte))
        .map(|offset| from + offset + 1)
}

fn first_range(needle: &[u8], bytes: &[u8], from: usize) -> Option<Range<usize>> {
    if needle.is_empty() || from > bytes.len() || bytes.len() - from < needle.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| {
            let start = from + offset;
            start..start + needle.len()
        })
}

fn utf8_sequence_length(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

fn is_utf8_continuation(byte: u8) -> bool {
    (0x80..=0xbf).contains(&byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_stream_finds_safe_utf8_and_escape_boundaries() {
        assert_eq!(trailing_safe_end(&[b'a', 0xe2, 0x82]), 1);
        assert_eq!(trailing_safe_end("a€".as_bytes()), 4);
        assert_eq!(trailing_safe_end(b"text\x1b[31"), 4);
        assert_eq!(trailing_safe_end(b"text\x1b[31mred"), 12);
        assert_eq!(leading_safe_index(&[0x82, 0xac, b'o', b'k']), 2);
        assert_eq!(leading_safe_index(b"]0;partial"), 10);
        assert_eq!(safe_replay_start(b"partial\ncomplete"), 8);
    }

    #[test]
    fn terminal_stream_detects_all_alternate_screen_forms() {
        for sequence in ALTERNATE_SCREEN_ENTER_SEQUENCES {
            assert_eq!(
                next_alternate_screen_sequence(sequence, 0, true),
                Some(0..sequence.len())
            );
        }
        for sequence in ALTERNATE_SCREEN_LEAVE_SEQUENCES {
            assert_eq!(
                next_alternate_screen_sequence(sequence, 0, false),
                Some(0..sequence.len())
            );
        }
    }
}
