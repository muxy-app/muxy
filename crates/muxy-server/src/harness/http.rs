//! A blocking HTTP/1.1 GET for an agent's service on this computer.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_RESPONSE: u64 = 4 * 1024 * 1024;

/// The loopback address in `url`, such as `http://127.0.0.1:4096`. Other
/// hosts are refused so the service password never leaves the computer.
pub(super) fn loopback(url: &str) -> Option<SocketAddr> {
    let authority = url.strip_prefix("http://")?.split('/').next()?;
    let address: SocketAddr = authority.replace("localhost", "127.0.0.1").parse().ok()?;
    address.ip().is_loopback().then_some(address)
}

/// The body of a `200 OK` reply to `GET path`.
pub(super) fn get(address: SocketAddr, path: &str, authorization: &str) -> Option<Vec<u8>> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Basic {authorization}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = Vec::new();
    stream.take(MAX_RESPONSE).read_to_end(&mut response).ok()?;
    body(&response)
}

pub(super) fn body(response: &[u8]) -> Option<Vec<u8>> {
    let end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&response[..end]).ok()?;
    let rest = &response[end + 4..];
    let mut lines = head.split("\r\n");
    let status = lines.next()?;
    if status.split(' ').nth(1) != Some("200") {
        return None;
    }
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.parse::<usize>().ok()?);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.eq_ignore_ascii_case("chunked");
        }
    }
    if chunked {
        return dechunk(rest);
    }
    match length {
        Some(length) => rest.get(..length).map(<[u8]>::to_vec),
        None => Some(rest.to_vec()),
    }
}

fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line = rest.windows(2).position(|window| window == b"\r\n")?;
        let size = std::str::from_utf8(&rest[..line]).ok()?;
        let size = usize::from_str_radix(size.split(';').next()?.trim(), 16).ok()?;
        rest = &rest[line + 2..];
        if size == 0 {
            return Some(body);
        }
        body.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size..)?.strip_prefix(b"\r\n")?;
    }
}

/// `value` for a URL query, with every byte outside the unreserved set escaped.
pub(super) fn query(value: &str) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
