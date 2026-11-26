use std::path::PathBuf;

const MAC_KEY: &[u8] = b":mac=";
const MAC_LENGTH: usize = 64;
const DIGITS: &[u8; 16] = b"0123456789abcdef";
const MAX_ARGUMENTS: usize = 64;
const KNOWN_KEYS: [&str; 7] = ["pid", "time", "argv", "cwd", "clear", "need", "mac"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    Resume {
        argv: Vec<String>,
        cwd: Option<PathBuf>,
    },
    Clear,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Report {
    pub(crate) pid: u32,
    pub(crate) time: u64,
    pub(crate) request: Request,
}

pub(crate) fn mac(signed: &[u8], token: &str) -> String {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    context.update(signed);
    context.update(b":");
    context.update(token.as_bytes());
    let mut hex = String::with_capacity(MAC_LENGTH);
    for byte in context.finish().as_ref() {
        hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    hex
}

pub(crate) fn verify(body: &[u8], token: &str) -> Option<Report> {
    let split = body.len().checked_sub(MAC_KEY.len() + MAC_LENGTH)?;
    let (signed, rest) = body.split_at(split);
    let given = rest.strip_prefix(MAC_KEY)?;
    if !given
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return None;
    }
    let expected = mac(signed, token);
    let matches = expected
        .bytes()
        .zip(given)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0;
    if !matches {
        return None;
    }
    let signed = std::str::from_utf8(signed).ok()?;
    let pairs: Vec<(&str, &str)> = signed
        .split(':')
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
        .collect();
    let field = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    };
    let pid = field("pid")
        .and_then(number)
        .and_then(|pid| u32::try_from(pid).ok())?;
    let time = field("time").and_then(number)?;
    let request = request(signed, &pairs).unwrap_or(Request::Rejected);
    Some(Report { pid, time, request })
}

fn request(signed: &str, pairs: &[(&str, &str)]) -> Option<Request> {
    let mut seen = Vec::with_capacity(pairs.len());
    for pair in signed.split(':') {
        let (key, value) = pair.split_once('=')?;
        if key.is_empty()
            || !key.bytes().all(|byte| byte.is_ascii_lowercase())
            || !value.bytes().all(value_byte)
            || seen.contains(&key)
            || key == "mac"
        {
            return None;
        }
        seen.push(key);
    }
    let field = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    };
    if let Some(need) = field("need")
        && !need
            .split(',')
            .filter(|key| !key.is_empty())
            .all(|key| KNOWN_KEYS.contains(&key))
    {
        return None;
    }
    match (field("argv"), field("clear")) {
        (None, Some("1")) => {
            if field("cwd").is_some() {
                return None;
            }
            Some(Request::Clear)
        }
        (Some(argv), None) => {
            let argv = arguments(argv)?;
            let cwd = match field("cwd") {
                Some(value) => Some(directory(value)?),
                None => None,
            };
            Some(Request::Resume { argv, cwd })
        }
        _ => None,
    }
}

fn number(text: &str) -> Option<u64> {
    let mut bytes = text.bytes();
    if !matches!(bytes.next(), Some(b'1'..=b'9')) || !bytes.all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn value_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b',' | b'+' | b'/' | b'=' | b'-')
}

fn arguments(value: &str) -> Option<Vec<String>> {
    let elements: Vec<&str> = value.split(',').collect();
    if elements.len() > MAX_ARGUMENTS {
        return None;
    }
    let argv = elements
        .into_iter()
        .map(text)
        .collect::<Option<Vec<String>>>()?;
    let program = argv.first()?;
    let valid = if program.contains('/') {
        program.starts_with('/')
    } else {
        !program.is_empty() && !program.starts_with('-')
    };
    valid.then_some(argv)
}

fn directory(value: &str) -> Option<PathBuf> {
    let path = text(value)?;
    path.starts_with('/').then(|| PathBuf::from(path))
}

fn text(value: &str) -> Option<String> {
    let padded = if value.contains('=') {
        value.to_owned()
    } else {
        match value.len() % 4 {
            0 => value.to_owned(),
            2 => format!("{value}=="),
            3 => format!("{value}="),
            _ => return None,
        }
    };
    let bytes = if padded.is_empty() {
        Vec::new()
    } else {
        muxy_core::base64::decode(&padded)?
    };
    let text = String::from_utf8(bytes).ok()?;
    (!text.chars().any(char::is_control)).then_some(text)
}

#[cfg(test)]
#[path = "report/tests.rs"]
mod tests;
