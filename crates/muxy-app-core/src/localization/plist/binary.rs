use std::collections::BTreeMap;

use super::{MAX_DEPTH, Value};

const MAX_DECODED_BYTES: usize = 16 * 1024 * 1024;
const INVALID: &str = "invalid binary property list";

pub(super) fn parse(bytes: &[u8]) -> Result<Value, String> {
    let trailer_start = bytes.len().checked_sub(32).ok_or(INVALID)?;
    if bytes.get(..8) != Some(b"bplist00") || trailer_start < 8 {
        return Err(INVALID.into());
    }
    let trailer = &bytes[trailer_start..];
    let offset_size = usize::from(trailer[6]);
    let reference_size = usize::from(trailer[7]);
    let count = integer(&trailer[8..16])?;
    let root = integer(&trailer[16..24])?;
    let table_start = integer(&trailer[24..32])?;
    if !(1..=8).contains(&offset_size)
        || !(1..=8).contains(&reference_size)
        || root >= count
        || !(8..=trailer_start).contains(&table_start)
    {
        return Err(INVALID.into());
    }
    let table_length = count.checked_mul(offset_size).ok_or(INVALID)?;
    let offsets = bytes[table_start..trailer_start]
        .get(..table_length)
        .ok_or(INVALID)?;
    Binary {
        objects: &bytes[..table_start],
        offsets,
        offset_size,
        reference_size,
        remaining: MAX_DECODED_BYTES,
    }
    .value(root, 0)
}

struct Binary<'a> {
    objects: &'a [u8],
    offsets: &'a [u8],
    offset_size: usize,
    reference_size: usize,
    remaining: usize,
}

impl<'a> Binary<'a> {
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or("the binary property list expands beyond its size limit")?;
        Ok(())
    }

    fn object(&self, reference: usize) -> Result<(u8, &'a [u8]), String> {
        let start = reference.checked_mul(self.offset_size).ok_or(INVALID)?;
        let offset = integer(
            self.offsets
                .get(start..)
                .and_then(|rest| rest.get(..self.offset_size))
                .ok_or(INVALID)?,
        )?;
        if offset < 8 {
            return Err(INVALID.into());
        }
        let (&marker, rest) = self
            .objects
            .get(offset..)
            .and_then(|rest| rest.split_first())
            .ok_or(INVALID)?;
        Ok((marker, rest))
    }

    fn value(&mut self, reference: usize, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("the property list is nested too deeply".into());
        }
        self.charge(64)?;
        let (marker, rest) = self.object(reference)?;
        match marker >> 4 {
            5 | 7 => {
                let bytes = sized(marker, rest, 1)?;
                if marker >> 4 == 5 && !bytes.is_ascii() {
                    return Err(INVALID.into());
                }
                self.charge(bytes.len())?;
                let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
                Ok(Value::String(text.to_owned()))
            }
            6 => {
                let bytes = sized(marker, rest, 2)?;
                self.charge(bytes.len().checked_mul(2).ok_or(INVALID)?)?;
                let text = char::decode_utf16(
                    bytes
                        .chunks_exact(2)
                        .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
                )
                .collect::<Result<String, _>>()
                .map_err(|error| error.to_string())?;
                Ok(Value::String(text))
            }
            13 => {
                let references = sized(marker, rest, self.reference_size * 2)?;
                let (keys, values) = references.split_at(references.len() / 2);
                let mut entries = BTreeMap::new();
                for (key, value) in keys
                    .chunks_exact(self.reference_size)
                    .zip(values.chunks_exact(self.reference_size))
                {
                    let Value::String(key) = self.value(integer(key)?, depth + 1)? else {
                        return Err("a dictionary entry needs a string key".into());
                    };
                    entries.insert(key, self.value(integer(value)?, depth + 1)?);
                }
                Ok(Value::Dictionary(entries))
            }
            4 => {
                sized(marker, rest, 1)?;
                Ok(Value::Other)
            }
            10 | 12 => {
                for reference in
                    sized(marker, rest, self.reference_size)?.chunks_exact(self.reference_size)
                {
                    self.charge(1)?;
                    self.object(integer(reference)?)?;
                }
                Ok(Value::Other)
            }
            _ => {
                let size = match marker {
                    0x00 | 0x08 | 0x09 => 0,
                    0x10..=0x14 => 1 << (marker & 15),
                    0x22 => 4,
                    0x23 | 0x33 => 8,
                    0x80..=0x8f => usize::from(marker & 15) + 1,
                    _ => return Err(INVALID.into()),
                };
                rest.get(..size).ok_or(INVALID)?;
                Ok(Value::Other)
            }
        }
    }
}

fn integer(bytes: &[u8]) -> Result<usize, String> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err(INVALID.into());
    }
    bytes
        .iter()
        .try_fold(0usize, |value, byte| {
            value.checked_mul(256)?.checked_add(usize::from(*byte))
        })
        .ok_or_else(|| INVALID.into())
}

fn sized(marker: u8, mut bytes: &[u8], unit: usize) -> Result<&[u8], String> {
    let mut count = usize::from(marker & 15);
    if count == 15 {
        let (&length_marker, rest) = bytes.split_first().ok_or(INVALID)?;
        if !(0x10..=0x13).contains(&length_marker) {
            return Err(INVALID.into());
        }
        let width = 1 << (length_marker & 15);
        count = integer(rest.get(..width).ok_or(INVALID)?)?;
        bytes = &rest[width..];
    }
    let length = count.checked_mul(unit).ok_or(INVALID)?;
    bytes.get(..length).ok_or_else(|| INVALID.into())
}
