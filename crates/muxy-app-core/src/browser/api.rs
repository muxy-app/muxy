use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::time::Duration;

use muxy_protocol::transport::LocalStream;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const SOCKET: &str = "app.sock";
pub const MAX_REQUEST_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_REPLY_BYTES: u64 = 128 * 1024 * 1024;
pub const NOT_READY: &str = "Muxy hasn't loaded that terminal's project yet";

pub fn socket_path(muxy_dir: &Path) -> PathBuf {
    muxy_dir.join(SOCKET)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub verb: String,
    #[serde(default)]
    pub args: Map<String, Value>,
}

impl Request {
    pub fn new(verb: impl Into<String>, args: Map<String, Value>) -> Self {
        Self {
            verb: verb.into(),
            args,
        }
    }

    pub fn read(stream: impl Read) -> io::Result<Self> {
        let mut bytes = Vec::new();
        stream.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request is too large",
            ));
        }
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }
}

pub fn encode_reply(result: &Result<Value, String>) -> Vec<u8> {
    let reply = match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    reply.to_string().into_bytes()
}

pub fn decode_reply(bytes: &[u8]) -> Result<Value, String> {
    let mut reply: Value =
        serde_json::from_slice(bytes).map_err(|_| "invalid reply from Muxy".to_owned())?;
    if reply["ok"].as_bool() == Some(true) {
        Ok(reply["value"].take())
    } else {
        Err(reply["error"]
            .as_str()
            .unwrap_or("Muxy did not handle the request")
            .to_owned())
    }
}

#[derive(Debug)]
pub enum CallError {
    NotRunning,
    Io(io::Error),
    Failed(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => formatter.write_str("the Muxy desktop app is not running"),
            Self::Io(error) if error.kind() == io::ErrorKind::WouldBlock => {
                formatter.write_str("no response from Muxy")
            }
            Self::Io(error) => write!(formatter, "could not reach Muxy: {error}"),
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for CallError {}

pub fn call(socket: &Path, request: &Request, timeout: Duration) -> Result<Value, CallError> {
    let mut stream = LocalStream::connect(socket).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => CallError::NotRunning,
        _ => CallError::Io(error),
    })?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .and_then(|()| {
            serde_json::to_writer(&mut stream, request).map_err(io::Error::other)?;
            stream.flush()?;
            stream.shutdown(Shutdown::Write)
        })
        .map_err(CallError::Io)?;
    let mut reply = Vec::new();
    stream
        .take(MAX_REPLY_BYTES)
        .read_to_end(&mut reply)
        .map_err(CallError::Io)?;
    if reply.is_empty() {
        return Err(CallError::Io(io::Error::new(
            io::ErrorKind::WouldBlock,
            "empty reply",
        )));
    }
    decode_reply(&reply).map_err(CallError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_round_trip() {
        let ok = encode_reply(&Ok(json!({"title": "Docs"})));
        assert_eq!(decode_reply(&ok), Ok(json!({"title": "Docs"})));
        let failed = encode_reply(&Err("browser tab not found x".into()));
        assert_eq!(
            decode_reply(&failed),
            Err("browser tab not found x".to_owned())
        );
        assert!(decode_reply(b"not json").is_err());
    }

    #[test]
    fn replies_keep_their_wire_format() {
        let ok = r#"{"ok":true,"value":{"title":"Docs"}}"#;
        let failed = r#"{"ok":false,"error":"nope"}"#;
        let encoded = |result| serde_json::from_slice::<Value>(&encode_reply(&result)).ok();
        let literal = |text| serde_json::from_str::<Value>(text).ok();
        assert_eq!(encoded(Ok(json!({"title": "Docs"}))), literal(ok));
        assert_eq!(encoded(Err("nope".into())), literal(failed));
        assert_eq!(decode_reply(ok.as_bytes()), Ok(json!({"title": "Docs"})));
        assert_eq!(decode_reply(failed.as_bytes()), Err("nope".to_owned()));
    }

    #[test]
    fn not_ready_keeps_its_text() {
        assert_eq!(NOT_READY, "Muxy hasn't loaded that terminal's project yet");
    }

    #[test]
    fn requests_keep_their_wire_format() -> io::Result<()> {
        let literal = br#"{"verb":"session.show","args":{"session":"1","project":"p"}}"#;
        let request = Request::read(literal.as_slice())?;
        assert_eq!(request.verb, "session.show");
        assert_eq!(request.args["session"], json!("1"));
        assert_eq!(request.args["project"], json!("p"));
        Ok(())
    }

    #[test]
    fn requests_are_read_to_the_end() -> io::Result<()> {
        let request = Request::new("browser.list", Map::new());
        let bytes = serde_json::to_vec(&request)?;
        assert_eq!(Request::read(bytes.as_slice())?, request);
        let defaulted = Request::read(br#"{"verb":"browser.list"}"#.as_slice())?;
        assert!(defaulted.args.is_empty());
        Ok(())
    }
}
