//! The property lists language packs use: old-style `.strings` files and XML
//! `.stringsdict` and `Info.plist` files, in UTF-8 or UTF-16.

use std::collections::BTreeMap;

/// Catalogs nest a few levels; deeper input is rejected before it can
/// exhaust the stack.
const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    String(String),
    Dictionary(BTreeMap<String, Value>),
    /// Numbers, dates, data, booleans, and arrays: never a translation.
    Other,
}

/// Reads a property list whose root is a dictionary, as a `.strings` file's is.
pub(super) fn dictionary(bytes: &[u8]) -> Result<BTreeMap<String, Value>, String> {
    if bytes.starts_with(b"bplist") {
        return Err("binary property lists are not supported".into());
    }
    let text = decode(bytes)?;
    let text = text.trim_start_matches('\u{feff}');
    let trimmed = text.trim_start();
    let value = if trimmed.starts_with("<?xml")
        || trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<plist")
    {
        xml(text)?
    } else {
        OldStyle::new(text).document()?
    };
    match value {
        Value::Dictionary(entries) => Ok(entries),
        _ => Err("the property list is not a dictionary".into()),
    }
}

fn decode(bytes: &[u8]) -> Result<String, String> {
    let utf16 = |little: bool, bytes: &[u8]| {
        if !bytes.len().is_multiple_of(2) {
            return Err("invalid UTF-16 text".to_owned());
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| {
                let pair = [pair[0], pair[1]];
                if little {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|error| error.to_string())
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(true, rest),
        [0xFE, 0xFF, rest @ ..] => utf16(false, rest),
        [_, 0, ..] => utf16(true, bytes),
        [0, _, ..] => utf16(false, bytes),
        _ => String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string()),
    }
}

fn xml(text: &str) -> Result<Value, String> {
    if text.contains("<!ENTITY") {
        return Err("property lists must not declare entities".into());
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let document =
        roxmltree::Document::parse_with_options(text, options).map_err(|e| e.to_string())?;
    let root = document.root_element();
    if root.tag_name().name() != "plist" {
        return Err("the XML document is not a property list".into());
    }
    let mut values = root.children().filter(roxmltree::Node::is_element);
    match (values.next(), values.next()) {
        (Some(value), None) => xml_value(value, 0),
        _ => Err("a property list holds exactly one value".into()),
    }
}

fn xml_value(node: roxmltree::Node<'_, '_>, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("the property list is nested too deeply".into());
    }
    match node.tag_name().name() {
        "string" => Ok(Value::String(
            node.children()
                .filter_map(|child| child.text())
                .collect::<String>(),
        )),
        "dict" => {
            let mut entries = BTreeMap::new();
            let mut children = node.children().filter(roxmltree::Node::is_element);
            while let Some(key) = children.next() {
                if key.tag_name().name() != "key" {
                    return Err("a dictionary entry needs a key".into());
                }
                let value = children.next().ok_or("a dictionary key needs a value")?;
                let key = key.children().filter_map(|child| child.text()).collect();
                entries.insert(key, xml_value(value, depth + 1)?);
            }
            Ok(Value::Dictionary(entries))
        }
        "array" | "data" | "date" | "integer" | "real" | "true" | "false" => Ok(Value::Other),
        other => Err(format!("unknown property list element <{other}>")),
    }
}

/// The old-style (`OpenStep`) format of `.strings` files: `"key" = "value";` entries,
/// optionally inside braces, with C comments.
struct OldStyle<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> OldStyle<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().peekable(),
        }
    }

    fn document(&mut self) -> Result<Value, String> {
        self.skip()?;
        let value = if self.chars.peek() == Some(&'{') {
            self.chars.next();
            let entries = self.entries(Some('}'), 0)?;
            Value::Dictionary(entries)
        } else {
            Value::Dictionary(self.entries(None, 0)?)
        };
        self.skip()?;
        if self.chars.peek().is_some() {
            return Err("unexpected text after the property list".into());
        }
        Ok(value)
    }

    fn entries(
        &mut self,
        end: Option<char>,
        depth: usize,
    ) -> Result<BTreeMap<String, Value>, String> {
        if depth > MAX_DEPTH {
            return Err("the property list is nested too deeply".into());
        }
        let mut entries = BTreeMap::new();
        loop {
            self.skip()?;
            match (self.chars.peek().copied(), end) {
                (None, None) => return Ok(entries),
                (Some(next), Some(end)) if next == end => {
                    self.chars.next();
                    return Ok(entries);
                }
                (None, Some(_)) => return Err("unterminated dictionary".into()),
                _ => {}
            }
            let key = self.string()?;
            self.skip()?;
            let value = if self.chars.peek() == Some(&'=') {
                self.chars.next();
                self.skip()?;
                self.value(depth)?
            } else {
                Value::String(key.clone())
            };
            self.skip()?;
            if self.chars.next() != Some(';') {
                return Err(format!("missing ';' after \"{key}\""));
            }
            entries.insert(key, value);
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if self.chars.peek() == Some(&'{') {
            self.chars.next();
            return Ok(Value::Dictionary(self.entries(Some('}'), depth + 1)?));
        }
        Ok(Value::String(self.string()?))
    }

    fn skip(&mut self) -> Result<(), String> {
        loop {
            match self.chars.peek() {
                Some(c) if c.is_whitespace() => {
                    self.chars.next();
                }
                Some('/') => {
                    let mut ahead = self.chars.clone();
                    ahead.next();
                    match ahead.next() {
                        Some('/') => {
                            for c in self.chars.by_ref() {
                                if c == '\n' {
                                    break;
                                }
                            }
                        }
                        Some('*') => {
                            self.chars.next();
                            self.chars.next();
                            let mut star = false;
                            loop {
                                match self.chars.next() {
                                    Some('/') if star => break,
                                    Some(c) => star = c == '*',
                                    None => return Err("unterminated comment".into()),
                                }
                            }
                        }
                        _ => return Ok(()),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        match self.chars.peek().copied() {
            Some(quote @ ('"' | '\'')) => {
                self.chars.next();
                self.quoted(quote)
            }
            Some(c) if unquoted(c) => {
                let mut text = String::new();
                while let Some(c) = self.chars.peek().copied().filter(|c| unquoted(*c)) {
                    text.push(c);
                    self.chars.next();
                }
                Ok(text)
            }
            Some(c) => Err(format!("unexpected '{c}'")),
            None => Err("unexpected end of the property list".into()),
        }
    }

    fn quoted(&mut self, quote: char) -> Result<String, String> {
        let mut text = String::new();
        let mut surrogate = None;
        loop {
            let c = self.chars.next().ok_or("unterminated string")?;
            if c == quote {
                return Ok(text);
            }
            if c != '\\' {
                text.push(c);
                continue;
            }
            let escaped = self.chars.next().ok_or("unterminated string")?;
            let unit = match escaped {
                'U' => {
                    let (unit, _) = self.digits(16, 4);
                    match (surrogate.take(), unit) {
                        (None, 0xD800..=0xDBFF) => {
                            surrogate = Some(unit);
                            continue;
                        }
                        (Some(high), 0xDC00..=0xDFFF) => {
                            0x10000 + ((high - 0xD800) << 10) + (unit - 0xDC00)
                        }
                        (_, unit) => unit,
                    }
                }
                '0'..='7' => {
                    let (rest, count) = self.digits(8, 2);
                    let first = u32::from(escaped) - u32::from('0');
                    first * 8u32.pow(count) + rest
                }
                'a' => 0x07,
                'b' => 0x08,
                'f' => 0x0C,
                'n' => 0x0A,
                'r' => 0x0D,
                't' => 0x09,
                'v' => 0x0B,
                other => u32::from(other),
            };
            text.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
        }
    }

    /// The value of up to `limit` digits in `radix`, and how many were read.
    fn digits(&mut self, radix: u32, limit: u32) -> (u32, u32) {
        let mut value = 0;
        let mut count = 0;
        while count < limit {
            let Some(digit) = self.chars.peek().and_then(|c| c.to_digit(radix)) else {
                break;
            };
            value = value * radix + digit;
            count += 1;
            self.chars.next();
        }
        (value, count)
    }
}

fn unquoted(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_$+/:.-".contains(c)
}
