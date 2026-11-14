//! Base64 text: standard and padded, or URL-safe without padding.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn encode(bytes: &[u8]) -> String {
    encode_with(bytes, ALPHABET, true)
}

/// The URL-safe alphabet without `=` padding, as in RFC 4648 §5.
pub fn encode_url(bytes: &[u8]) -> String {
    encode_with(bytes, URL_ALPHABET, false)
}

/// Reads standard, padded text; `None` for anything else.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end();
    if !text.len().is_multiple_of(4) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    for chunk in text.as_bytes().chunks(4) {
        let padding = chunk.iter().rev().take_while(|byte| **byte == b'=').count();
        if padding > 2 || (padding > 0 && bytes.len() + 3 < text.len() / 4 * 3) {
            return None;
        }
        let mut value = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            let digit = if index >= 4 - padding {
                0
            } else {
                u32::try_from(ALPHABET.iter().position(|letter| letter == byte)?).ok()?
            };
            value |= digit << (18 - 6 * index);
        }
        let decoded = value.to_be_bytes();
        bytes.extend_from_slice(&decoded[1..4 - padding]);
    }
    Some(bytes)
}

fn encode_with(bytes: &[u8], alphabet: &[u8; 64], padded: bool) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | (u32::from(*byte) << (16 - 8 * index))
            });
        for index in 0..4 {
            if index <= chunk.len() {
                text.push(char::from(
                    alphabet[(value >> (18 - 6 * index)) as usize & 63],
                ));
            } else if padded {
                text.push('=');
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_standard_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), encoded);
        }
        assert_eq!(encode(&[0xfb, 0xff, 0xbf]), "+/+/");
    }

    #[test]
    fn decoding_reverses_encoding_and_refuses_other_text() {
        for plain in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            assert_eq!(decode(&encode(plain.as_bytes())), Some(plain.into()));
        }
        assert_eq!(decode("+/+/"), Some(vec![0xfb, 0xff, 0xbf]));
        assert_eq!(decode("Zm9vYg==\n"), Some(b"foob".to_vec()));
        for bad in ["Zg=", "Z===", "Zg==Zm8=", "Zm9*", "-_-_"] {
            assert_eq!(decode(bad), None, "{bad}");
        }
    }

    #[test]
    fn url_text_swaps_the_last_two_characters_and_drops_padding() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg"),
            ("fo", "Zm8"),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg"),
            ("fooba", "Zm9vYmE"),
        ] {
            assert_eq!(encode_url(plain.as_bytes()), encoded);
        }
        assert_eq!(encode_url(&[0xfb, 0xff, 0xbf]), "-_-_");
    }
}
