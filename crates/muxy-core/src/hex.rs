//! Lowercase hexadecimal text for bytes such as tokens and digests.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

pub fn encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

/// Exactly `N` bytes, or `None` when `text` has another length or a
/// character that isn't a hex digit.
pub fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut bytes = [0; N];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *byte = pair_value(pair)?;
    }
    Some(bytes)
}

pub fn decode_vec(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes().chunks_exact(2).map(pair_value).collect()
}

fn pair_value(pair: &[u8]) -> Option<u8> {
    let high = digit_value(pair[0])?;
    let low = digit_value(pair[1])?;
    Some(high << 4 | low)
}

fn digit_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_byte() {
        let bytes: Vec<u8> = (0..=u8::MAX).collect();
        let text = encode(&bytes);
        assert_eq!(&text[..8], "00010203");
        assert_eq!(&text[text.len() - 4..], "feff");
        assert_eq!(decode_vec(&text), Some(bytes.clone()));
        assert_eq!(decode_vec(&text.to_uppercase()), Some(bytes));
        assert_eq!(decode::<2>("0aFf"), Some([0x0a, 0xff]));
    }

    #[test]
    fn rejects_text_that_is_not_hex_of_the_expected_length() {
        assert_eq!(decode::<2>("0a0"), None);
        assert_eq!(decode::<2>("0a0b0c"), None);
        assert_eq!(decode::<2>("0g0b"), None);
        assert_eq!(decode::<1>("+1"), None);
        assert_eq!(decode_vec("abc"), None);
        assert_eq!(decode_vec("éé"), None);
    }
}
