//! Apple-style format strings, the placeholder syntax of `Localizable.strings`
//! keys: `%@`, `%lld`, `%1$@`, `%.1f`, `%%`, and `%#@name@` plural references.

use std::borrow::Cow;

/// The argument type a placeholder reads. Length modifiers that read the same
/// type, such as `%ld` and `%lld`, share a kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Object,
    CString,
    UnicharString,
    Pointer,
    Unichar,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Double,
    LongDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spec<'a> {
    /// The 1-based `%N$` position, when written.
    pub position: Option<usize>,
    pub flags: &'a str,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub conversion: char,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Token<'a> {
    Text(&'a str),
    Percent,
    Argument(Spec<'a>),
    /// A `%#@name@` reference to a plural variable.
    Variable(&'a str),
}

/// Splits a format into tokens; `None` when a placeholder is malformed or
/// takes its width or precision from an argument.
pub fn tokens(format: &str) -> Option<Vec<Token<'_>>> {
    let bytes = format.as_bytes();
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if start < index {
            tokens.push(Token::Text(&format[start..index]));
        }
        index += 1;
        let (token, end) = match bytes.get(index)? {
            b'%' => (Token::Percent, index + 1),
            b'#' => variable(format, index)?,
            _ => specifier(format, index)?,
        };
        tokens.push(token);
        index = end;
        start = end;
    }
    if start < bytes.len() {
        tokens.push(Token::Text(&format[start..]));
    }
    Some(tokens)
}

fn variable(format: &str, start: usize) -> Option<(Token<'_>, usize)> {
    let rest = format.get(start + 1..)?.strip_prefix('@')?;
    let length = rest.find('@').filter(|length| *length > 0)?;
    Some((Token::Variable(&rest[..length]), start + 2 + length + 1))
}

fn specifier(format: &str, start: usize) -> Option<(Token<'_>, usize)> {
    let bytes = format.as_bytes();
    let digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let mut index = start;
    let mut position = None;
    let after_digits = digits(index);
    if after_digits > index && bytes.get(after_digits) == Some(&b'$') {
        position = Some(
            format[index..after_digits]
                .parse()
                .ok()
                .filter(|p| *p >= 1)?,
        );
        index = after_digits + 1;
    }
    let flags_start = index;
    while bytes
        .get(index)
        .is_some_and(|byte| b"-+ #0'".contains(byte))
    {
        index += 1;
    }
    let flags = &format[flags_start..index];
    let field = |index: usize| -> Option<(Option<usize>, usize)> {
        if bytes.get(index) == Some(&b'*') {
            return None;
        }
        let end = digits(index);
        Some((format[index..end].parse().ok(), end))
    };
    let (width, after_width) = field(index)?;
    index = after_width;
    let mut precision = None;
    if bytes.get(index) == Some(&b'.') {
        let (value, end) = field(index + 1)?;
        precision = Some(value.unwrap_or(0));
        index = end;
    }
    let length_start = index;
    while bytes
        .get(index)
        .is_some_and(|byte| b"hlLqjzt".contains(byte))
    {
        index += 1;
    }
    let length = &format[length_start..index];
    let conversion = char::from(*bytes.get(index)?);
    let kind = kind(length, conversion)?;
    Some((
        Token::Argument(Spec {
            position,
            flags,
            width,
            precision,
            conversion,
            kind,
        }),
        index + 1,
    ))
}

fn kind(length: &str, conversion: char) -> Option<Kind> {
    let integer = |small: Kind, large: Kind| match length {
        "" | "h" | "hh" => Some(small),
        "l" | "ll" | "q" | "j" | "z" | "t" => Some(large),
        _ => None,
    };
    let plain = |kind: Kind| length.is_empty().then_some(kind);
    match conversion {
        '@' => plain(Kind::Object),
        'd' | 'i' => integer(Kind::Int32, Kind::Int64),
        'u' | 'x' | 'X' | 'o' => integer(Kind::UInt32, Kind::UInt64),
        'f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A' => match length {
            "" | "l" => Some(Kind::Double),
            "L" => Some(Kind::LongDouble),
            _ => None,
        },
        'c' => plain(Kind::Int32),
        'C' => plain(Kind::Unichar),
        's' => plain(Kind::CString),
        'S' => plain(Kind::UnicharString),
        'p' => plain(Kind::Pointer),
        _ => None,
    }
}

/// A value passed to a translated format.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg<'a> {
    Text(Cow<'a, str>),
    Int(i64),
    UInt(u64),
    Float(f64),
}

impl<'a, T: AsRef<str> + ?Sized> From<&'a T> for Arg<'a> {
    fn from(text: &'a T) -> Self {
        Self::Text(Cow::Borrowed(text.as_ref()))
    }
}

impl From<String> for Arg<'_> {
    fn from(text: String) -> Self {
        Self::Text(Cow::Owned(text))
    }
}

impl<'a> From<Cow<'a, str>> for Arg<'a> {
    fn from(text: Cow<'a, str>) -> Self {
        Self::Text(text)
    }
}

macro_rules! numbers {
    ($variant:ident, $target:ty: $($source:ty),+) => {
        $(impl From<$source> for Arg<'_> {
            #[allow(clippy::cast_lossless, clippy::cast_possible_wrap, reason = "every source fits")]
            fn from(value: $source) -> Self {
                Self::$variant(value as $target)
            }
        })+
    };
}

numbers!(Int, i64: i8, i16, i32, i64, isize);
numbers!(UInt, u64: u8, u16, u32, u64, usize);
numbers!(Float, f64: f32, f64);

/// Writes one placeholder with C `printf` semantics for its flags, width, and precision.
pub(super) fn write_argument(out: &mut String, spec: &Spec<'_>, arg: Option<&Arg<'_>>) {
    let left = spec.flags.contains('-');
    let integral = "diuxXo".contains(spec.conversion);
    let zero = spec.flags.contains('0')
        && !left
        && (integral || "fFeEgGaA".contains(spec.conversion))
        && !(integral && spec.precision.is_some());
    let body = match (spec.conversion, arg) {
        (_, None) => String::new(),
        ('d' | 'i', Some(Arg::Int(value))) => integer(*value < 0, value.unsigned_abs(), spec),
        ('d' | 'i', Some(Arg::UInt(value))) => integer(false, *value, spec),
        ('u' | 'x' | 'X' | 'o', Some(Arg::Int(value))) => unsigned(value.cast_unsigned(), spec),
        ('u' | 'x' | 'X' | 'o', Some(Arg::UInt(value))) => unsigned(*value, spec),
        ('f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A', Some(arg)) => float(number(arg), spec),
        ('c' | 'C', Some(Arg::Int(value))) => character(value.cast_unsigned()),
        ('c' | 'C', Some(Arg::UInt(value))) => character(*value),
        (_, Some(arg)) => {
            let text = display(arg);
            match spec.precision {
                Some(limit) if matches!(spec.conversion, '@' | 's' | 'S') => {
                    text.chars().take(limit).collect()
                }
                _ => text,
            }
        }
    };
    let width = spec.width.unwrap_or(0);
    let count = body.chars().count();
    if count >= width {
        out.push_str(&body);
    } else if left {
        out.push_str(&body);
        out.extend(std::iter::repeat_n(' ', width - count));
    } else if zero && body.contains(|c: char| c.is_ascii_digit()) {
        let sign = usize::from(body.starts_with(['-', '+', ' ']));
        out.push_str(&body[..sign]);
        out.extend(std::iter::repeat_n('0', width - count));
        out.push_str(&body[sign..]);
    } else {
        out.extend(std::iter::repeat_n(' ', width - count));
        out.push_str(&body);
    }
}

fn character(value: u64) -> String {
    u32::try_from(value)
        .ok()
        .and_then(char::from_u32)
        .map(String::from)
        .unwrap_or_default()
}

fn display(arg: &Arg<'_>) -> String {
    match arg {
        Arg::Text(text) => text.clone().into_owned(),
        Arg::Int(value) => value.to_string(),
        Arg::UInt(value) => value.to_string(),
        Arg::Float(value) => value.to_string(),
    }
}

#[allow(clippy::cast_precision_loss, reason = "printf converts to double too")]
fn number(arg: &Arg<'_>) -> f64 {
    match arg {
        Arg::Int(value) => *value as f64,
        Arg::UInt(value) => *value as f64,
        Arg::Float(value) => *value,
        Arg::Text(text) => text.trim().parse().unwrap_or(0.0),
    }
}

fn sign(negative: bool, spec: &Spec<'_>) -> &'static str {
    if negative {
        "-"
    } else if spec.flags.contains('+') {
        "+"
    } else if spec.flags.contains(' ') {
        " "
    } else {
        ""
    }
}

fn integer(negative: bool, magnitude: u64, spec: &Spec<'_>) -> String {
    let digits = magnitude.to_string();
    let padding = spec
        .precision
        .unwrap_or(0)
        .saturating_sub(digits.chars().count());
    format!("{}{}{digits}", sign(negative, spec), "0".repeat(padding))
}

fn unsigned(value: u64, spec: &Spec<'_>) -> String {
    let digits = match spec.conversion {
        'x' => format!("{value:x}"),
        'X' => format!("{value:X}"),
        'o' => format!("{value:o}"),
        _ => value.to_string(),
    };
    let padding = spec.precision.unwrap_or(0).saturating_sub(digits.len());
    format!("{}{digits}", "0".repeat(padding))
}

fn float(value: f64, spec: &Spec<'_>) -> String {
    if !value.is_finite() {
        let text = if value.is_nan() {
            "nan"
        } else if value > 0.0 {
            "inf"
        } else {
            "-inf"
        };
        return if spec.conversion.is_ascii_uppercase() {
            text.to_ascii_uppercase()
        } else {
            text.into()
        };
    }
    let precision = spec.precision.unwrap_or(6);
    let body = match spec.conversion {
        'e' | 'E' => exponent(value.abs(), precision),
        'g' | 'G' => general(value.abs(), precision, spec.flags.contains('#')),
        'a' | 'A' => value.abs().to_string(),
        _ => format!("{:.*}", precision, value.abs()),
    };
    let body = if spec.conversion.is_ascii_uppercase() {
        body.to_ascii_uppercase()
    } else {
        body
    };
    format!(
        "{}{body}",
        sign(value.is_sign_negative() && value != 0.0, spec)
    )
}

/// `1.500000e+03` rather than Rust's `1.5e3`.
fn exponent(value: f64, precision: usize) -> String {
    let text = format!("{value:.precision$e}");
    let (mantissa, power) = text.split_once('e').unwrap_or((&text, "0"));
    let power: i32 = power.parse().unwrap_or(0);
    format!(
        "{mantissa}e{}{:02}",
        if power < 0 { '-' } else { '+' },
        power.unsigned_abs()
    )
}

fn general(value: f64, precision: usize, keep_zeros: bool) -> String {
    let precision = precision.max(1);
    let power = if value == 0.0 {
        0
    } else {
        format!("{value:.*e}", precision - 1)
            .split_once('e')
            .and_then(|(_, power)| power.parse::<i64>().ok())
            .unwrap_or(0)
    };
    let limit = i64::try_from(precision).unwrap_or(i64::MAX);
    let text = if (-4..limit).contains(&power) {
        let decimals = usize::try_from(limit - 1 - power).unwrap_or(0);
        format!("{value:.decimals$}")
    } else {
        exponent(value, precision - 1)
    };
    if keep_zeros || !text.contains('.') {
        return text;
    }
    let (number, suffix) = text
        .find('e')
        .map_or((text.as_str(), ""), |at| text.split_at(at));
    format!(
        "{}{suffix}",
        number.trim_end_matches('0').trim_end_matches('.')
    )
}
