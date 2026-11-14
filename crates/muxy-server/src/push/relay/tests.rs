use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use muxy_protocol::{PushEnvironment, PushPlatform, PushToken};

use super::*;

const ID: &str = "01jc8q4m6x2v7s0t3n5r9k1b2d";
const SECRET: &str = "q5XxAbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_A";

#[derive(Debug)]
struct Received {
    request_line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Received {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_default()
    }
}

/// Answers one request with `reply`, and hands back what it received.
/// An HTTP reply with `head`'s status line and headers, and `body`.
fn reply(head: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {head}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

fn serve(reply: String) -> (HttpRelay, mpsc::Receiver<Received>) {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        panic!("no loopback port");
    };
    let port = listener
        .local_addr()
        .map(|address| address.port())
        .unwrap_or(0);
    let (sender, received) = mpsc::channel();
    thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        let _ = reader.read_line(&mut request_line);
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            if let Some((key, value)) = line.trim_end().split_once(':') {
                headers.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        }
        let length = headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0; length];
        let _ = reader.read_exact(&mut body);
        let _ = sender.send(Received {
            request_line: request_line.trim_end().to_owned(),
            headers,
            body,
        });
        let mut stream = reader.into_inner();
        let _ = stream.write_all(reply.as_bytes());
    });
    (
        HttpRelay::new(Some(format!("http://127.0.0.1:{port}"))),
        received,
    )
}

fn received(receiver: &mpsc::Receiver<Received>) -> Received {
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(request) => request,
        Err(error) => panic!("the relay received nothing: {error}"),
    }
}

fn credentials() -> Credentials {
    match Credentials::new(ID.into(), SECRET.into()) {
        Some(credentials) => credentials,
        None => panic!("sample credentials are invalid"),
    }
}

fn apns() -> PushToken {
    PushToken {
        platform: PushPlatform::Apns,
        token: "ab".repeat(32),
        environment: Some(PushEnvironment::Sandbox),
    }
}

#[test]
fn registering_posts_the_token_and_keeps_the_returned_id_and_secret() {
    let (relay, receiver) = serve(reply(
        "201 Created\r\nContent-Type: application/json",
        r#"{"data":{"id":"01jc8q4m6x2v7s0t3n5r9k1b2d","platform":"apns","environment":"sandbox","active":true,"invalidated_at":null,"last_delivered_at":null,"created_at":"2026-11-08T12:00:00.000000Z","secret":"q5XxAbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_A"}}"#,
    ));
    assert_eq!(relay.register(&apns()), Ok(credentials()));
    let request = received(&receiver);
    assert_eq!(
        request.request_line,
        "POST /api/push/registrations HTTP/1.1"
    );
    assert_eq!(request.header("accept"), Some("application/json"));
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(
        request.json(),
        serde_json::json!({"platform": "apns", "token": "ab".repeat(32), "environment": "sandbox"})
    );
}

#[test]
fn fcm_tokens_are_sent_without_an_environment() {
    let (relay, receiver) = serve(reply("200 OK", ""));
    let token = PushToken {
        platform: PushPlatform::Fcm,
        token: "fcm:token_1".repeat(4),
        environment: None,
    };
    assert_eq!(relay.replace(&credentials(), &token), Ok(()));
    let request = received(&receiver);
    assert_eq!(
        request.request_line,
        format!("PUT /api/push/registrations/{ID} HTTP/1.1")
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(
        request.json(),
        serde_json::json!({"platform": "fcm", "token": "fcm:token_1".repeat(4)})
    );
}

#[test]
fn notifications_and_deletes_are_authorized_with_the_secret() {
    let (relay, receiver) = serve(reply("202 Accepted", ""));
    let notification = Outgoing {
        kind: "attention",
        payload: "AQ3x".into(),
        collapse_id: "Jx3kQ9bLmW2pRt0a".into(),
        thread_id: "c8VnE1qYzT4uH7sK".into(),
        ttl: 3600,
    };
    assert_eq!(relay.send(&credentials(), &notification), Ok(()));
    let request = received(&receiver);
    assert_eq!(
        request.request_line,
        format!("POST /api/push/registrations/{ID}/notifications HTTP/1.1")
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    assert_eq!(
        request.json(),
        serde_json::json!({
            "kind": "attention",
            "payload": "AQ3x",
            "collapse_id": "Jx3kQ9bLmW2pRt0a",
            "thread_id": "c8VnE1qYzT4uH7sK",
            "ttl": 3600
        })
    );

    let (relay, receiver) = serve(reply("204 No Content", ""));
    assert_eq!(relay.delete(&credentials()), Ok(()));
    let request = received(&receiver);
    assert_eq!(
        request.request_line,
        format!("DELETE /api/push/registrations/{ID} HTTP/1.1")
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
}

#[test]
fn every_status_maps_to_what_the_server_does_next() {
    for (head, expected) in [
        ("401 Unauthorized", RelayError::Unauthorized),
        ("404 Not Found", RelayError::NotFound),
        ("410 Gone", RelayError::Gone),
        ("422 Unprocessable Content", RelayError::Refused),
        ("400 Bad Request", RelayError::Refused),
        (
            "302 Found\r\nLocation: https://example.com/",
            RelayError::Refused,
        ),
        (
            "429 Too Many Requests\r\nRetry-After: 30",
            RelayError::RateLimited(Some(Duration::from_secs(30))),
        ),
        ("429 Too Many Requests", RelayError::RateLimited(None)),
        (
            "503 Service Unavailable\r\nRetry-After: 120",
            RelayError::Unavailable(Some(Duration::from_secs(120))),
        ),
        ("500 Internal Server Error", RelayError::Unavailable(None)),
    ] {
        let (relay, receiver) = serve(reply(head, ""));
        assert_eq!(relay.delete(&credentials()), Err(expected), "{head}");
        received(&receiver);
    }
}

#[test]
fn a_success_without_usable_credentials_is_retried_later() {
    let (relay, receiver) = serve(reply(
        "201 Created",
        r#"{"data":{"id":"../../admin","secret":"short"}}"#,
    ));
    assert_eq!(relay.register(&apns()), Err(RelayError::Unavailable(None)));
    received(&receiver);
}

#[test]
fn no_answer_is_unavailable_and_a_missing_relay_sends_nothing() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
        panic!("no loopback port");
    };
    let port = listener
        .local_addr()
        .map(|address| address.port())
        .unwrap_or(0);
    drop(listener);
    let relay = HttpRelay::new(Some(format!("http://127.0.0.1:{port}")));
    assert_eq!(
        relay.delete(&credentials()),
        Err(RelayError::Unavailable(None))
    );
    assert_eq!(
        HttpRelay::new(None).delete(&credentials()),
        Err(RelayError::Unavailable(None))
    );
}

#[test]
fn the_relay_is_https_or_this_computer_and_only_an_origin() {
    for (text, expected) in [
        ("https://muxy.app", Some("https://muxy.app")),
        ("https://muxy.app/", Some("https://muxy.app")),
        (
            "https://relay.example.com:8443",
            Some("https://relay.example.com:8443"),
        ),
        ("http://localhost:8000", Some("http://localhost:8000")),
        ("http://127.0.0.1:8000/", Some("http://127.0.0.1:8000")),
        ("http://[::1]:8000", Some("http://[::1]:8000")),
        ("http://muxy.app", None),
        ("http://192.168.1.5:8000", None),
        ("ftp://muxy.app", None),
        ("https://muxy.app/api/push", None),
        ("https://muxy.app?x=1", None),
        ("https://user:pass@muxy.app", None),
        ("muxy.app", None),
        ("", None),
    ] {
        assert_eq!(base_url(text).as_deref(), expected, "{text}");
    }
}

#[test]
fn relay_credentials_have_the_relays_shapes() {
    assert!(Credentials::new(ID.into(), SECRET.into()).is_some());
    for (id, secret) in [
        ("01JC8Q4M6X2V7S0T3N5R9K1B2D", SECRET),
        ("01jc8q4m6x2v7s0t3n5r9k1b2", SECRET),
        ("01jc8q4m6x2v7s0t3n5r9k/b2d", SECRET),
        (ID, "short"),
        (ID, "q5XxAbCdEfGhIjKlMnOpQrStUvWxYz0123456789+/A"),
    ] {
        assert!(
            Credentials::new(id.into(), secret.into()).is_none(),
            "{id} {secret}"
        );
    }
    assert!(!format!("{:?}", credentials()).contains(SECRET));
}
