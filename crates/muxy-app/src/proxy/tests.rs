use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};

use muxy_app_core::settings::{ProxyKind, ProxySettings};

use super::{base64, configure, connect, use_for_app};

const BANNER: &[u8] = b"SSH-2.0-test\r\n";

fn proxy(kind: ProxyKind, port: u16) -> ProxySettings {
    ProxySettings {
        kind,
        host: "127.0.0.1".into(),
        port,
        username: "me".into(),
        password: "secret".into(),
        ..ProxySettings::default()
    }
}

/// A proxy that answers one connection with `serve`, then echoes.
fn fake(
    serve: impl FnOnce(&mut TcpStream) -> io::Result<Vec<u8>> + Send + 'static,
) -> io::Result<(u16, JoinHandle<io::Result<Vec<u8>>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let request = serve(&mut stream)?;
        let mut buffer = [0; 64];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => return Ok(request),
                Ok(read) => stream.write_all(&buffer[..read])?,
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {
                    return Ok(request);
                }
                Err(error) => return Err(error),
            }
        }
    });
    Ok((port, server))
}

fn http_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        request.push(byte[0]);
    }
    Ok(request)
}

fn read(stream: &mut impl Read, length: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// The computer's first bytes arrive with the proxy's reply, so reading the
/// reply must not swallow them.
#[test]
fn http_proxies_tunnel_with_basic_auth_and_keep_the_first_bytes() -> io::Result<()> {
    let (port, server) = fake(|stream| {
        let request = http_request(stream)?;
        let mut reply = b"HTTP/1.1 200 Connection established\r\n\r\n".to_vec();
        reply.extend_from_slice(BANNER);
        stream.write_all(&reply)?;
        Ok(request)
    })?;
    let mut stream = connect(&proxy(ProxyKind::Http, port), "box.example", 22)?;
    assert_eq!(read(&mut stream, BANNER.len())?, BANNER);
    stream.write_all(b"ping")?;
    assert_eq!(read(&mut stream, 4)?, b"ping");
    drop(stream);
    let request = String::from_utf8(server.join().expect("proxy thread")?).expect("text");
    assert!(request.starts_with("CONNECT box.example:22 HTTP/1.1\r\n"));
    assert!(request.contains("\r\nProxy-Authorization: Basic bWU6c2VjcmV0\r\n"));
    Ok(())
}

#[test]
fn http_proxy_refusals_explain_themselves() -> io::Result<()> {
    let (port, server) = fake(|stream| {
        let request = http_request(stream)?;
        stream.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")?;
        Ok(request)
    })?;
    let error = connect(&proxy(ProxyKind::Http, port), "::1", 22).expect_err("refused");
    assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused);
    assert!(error.to_string().contains("407"), "{error}");
    let request = String::from_utf8(server.join().expect("proxy thread")?).expect("text");
    assert!(request.starts_with("CONNECT [::1]:22 HTTP/1.1\r\n"));
    Ok(())
}

#[test]
fn socks5_proxies_log_in_and_connect_by_name() -> io::Result<()> {
    let (port, server) = fake(|stream| {
        let mut seen = read(stream, 4)?;
        stream.write_all(&[5, 2])?;
        seen.extend(read(stream, 2 + 2 + 1 + 6)?);
        stream.write_all(&[1, 0])?;
        seen.extend(read(stream, 5)?);
        let length = usize::from(seen[seen.len() - 1]);
        seen.extend(read(stream, length + 2)?);
        let mut reply = vec![5, 0, 0, 3, 3, b'b', b'o', b'x', 0, 22];
        reply.extend_from_slice(BANNER);
        stream.write_all(&reply)?;
        Ok(seen)
    })?;
    let mut stream = connect(&proxy(ProxyKind::Socks5, port), "box.example", 2222)?;
    assert_eq!(read(&mut stream, BANNER.len())?, BANNER);
    drop(stream);
    let mut expected = vec![5, 2, 0, 2, 1, 2, b'm', b'e', 6];
    expected.extend_from_slice(b"secret");
    expected.extend_from_slice(&[5, 1, 0, 3, 11]);
    expected.extend_from_slice(b"box.example");
    expected.extend_from_slice(&2222_u16.to_be_bytes());
    assert_eq!(server.join().expect("proxy thread")?, expected);
    Ok(())
}

#[test]
fn socks5_refusals_are_errors() -> io::Result<()> {
    let (port, server) = fake(|stream| {
        let seen = read(stream, 4)?;
        stream.write_all(&[5, 2])?;
        read(stream, 2 + 2 + 1 + 6)?;
        stream.write_all(&[1, 1])?;
        Ok(seen)
    })?;
    let error = connect(&proxy(ProxyKind::Socks5, port), "10.0.0.1", 22).expect_err("refused");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    server.join().expect("proxy thread")?;

    let (port, server) = fake(|stream| {
        let seen = read(stream, 3)?;
        stream.write_all(&[5, 0])?;
        read(stream, 4 + 4 + 2)?;
        stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])?;
        Ok(seen)
    })?;
    let anonymous = ProxySettings {
        username: String::new(),
        password: String::new(),
        ..proxy(ProxyKind::Socks5, port)
    };
    let error = connect(&anonymous, "10.0.0.1", 22).expect_err("refused");
    assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused);
    assert!(error.to_string().contains("connection refused"), "{error}");
    assert_eq!(server.join().expect("proxy thread")?, [5, 1, 0]);
    Ok(())
}

#[test]
fn the_apps_own_requests_use_the_proxy_only_when_it_is_global()
-> Result<(), Box<dyn std::error::Error>> {
    let (port, server) = fake(|stream| {
        let request = http_request(stream)?;
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")?;
        stream.shutdown(std::net::Shutdown::Write)?;
        Ok(request)
    })?;
    let mut settings = proxy(ProxyKind::Http, port);
    use_for_app(Some(&settings));
    assert!(super::app().is_none());
    settings.global = true;
    use_for_app(Some(&settings));
    let client = configure(reqwest::blocking::Client::builder())?.build()?;
    use_for_app(None);
    let body = client.get("http://muxy.invalid/update").send()?.text()?;
    assert_eq!(body, "ok");
    let request = String::from_utf8(server.join().expect("proxy thread")?)?;
    assert!(request.starts_with("GET http://muxy.invalid/update HTTP/1.1\r\n"));
    assert!(request.contains("proxy-authorization: Basic bWU6c2VjcmV0\r\n"));
    Ok(())
}

#[test]
fn bypassed_hosts_and_subnets_are_reached_directly_by_http_and_ssh()
-> Result<(), Box<dyn std::error::Error>> {
    for (host, bypass) in [
        ("127.0.0.1", "127.0.0.1"),
        ("127.0.0.1", "127.0.0.0/8"),
        ("::1", "::/64"),
    ] {
        for kind in [ProxyKind::Http, ProxyKind::Socks5] {
            let listener = match TcpListener::bind((host, 0)) {
                Ok(listener) => listener,
                Err(error) if host == "::1" && error.kind() == io::ErrorKind::AddrNotAvailable => {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let address = listener.local_addr()?;
            let settings = ProxySettings {
                bypass: bypass.into(),
                global: true,
                ..proxy(kind, 9)
            };
            let mut stream = connect(&settings, host, address.port())?;
            let (mut accepted, _) = listener.accept()?;
            stream.write_all(b"direct")?;
            assert_eq!(read(&mut accepted, 6)?, b"direct");
            let server = thread::spawn(move || -> io::Result<Vec<u8>> {
                let (mut stream, _) = listener.accept()?;
                let request = http_request(&mut stream)?;
                stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect",
                )?;
                Ok(request)
            });
            use_for_app(Some(&settings));
            let builder = configure(reqwest::blocking::Client::builder());
            use_for_app(None);
            let client = builder?
                .timeout(std::time::Duration::from_secs(2))
                .build()?;
            let body = client
                .get(format!("http://{address}/update"))
                .send()?
                .text()?;
            assert_eq!(body, "direct");
            assert!(
                server
                    .join()
                    .expect("direct server")?
                    .starts_with(b"GET /update HTTP/1.1\r\n")
            );
        }
    }
    Ok(())
}

#[test]
fn base64_matches_the_standard_vectors() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(plain.as_bytes()), encoded);
    }
}
