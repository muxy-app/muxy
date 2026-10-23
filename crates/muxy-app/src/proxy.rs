//! The app's proxy beyond terminals: its own web requests, and ssh to other
//! computers. ssh runs the app's own executable as its `ProxyCommand`, which
//! reads the proxy from its environment, connects through it, and relays
//! ssh's bytes.

use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Shutdown, TcpStream, ToSocketAddrs};
use std::process::ExitCode;
use std::sync::{PoisonError, RwLock};
use std::thread;
use std::time::Duration;

use muxy_app_core::settings::{ProxyKind, ProxySettings};
use muxy_client::ProxyCommand;

/// The app runs as ssh's proxy command when this is its first argument.
pub(crate) const TUNNEL: &str = "--muxy-proxy-tunnel";
/// Which proxy the tunnel uses. It is in the environment of the app's ssh
/// runs, so only processes of this user can read it.
const SETTINGS: &str = "MUXY_PROXY";
/// Covers connecting to the proxy and its handshake.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Longest proxy reply header accepted.
const MAX_REPLY: usize = 16 * 1024;

/// The proxy the app's own requests use, when it applies to the whole app.
#[cfg(not(test))]
static APP: RwLock<Option<ProxySettings>> = RwLock::new(None);

#[cfg(test)]
thread_local! {
    /// Tests run side by side, so each keeps its own.
    static APP: RwLock<Option<ProxySettings>> = const { RwLock::new(None) };
}

fn with_app<T>(use_proxy: impl FnOnce(&RwLock<Option<ProxySettings>>) -> T) -> T {
    #[cfg(not(test))]
    return use_proxy(&APP);
    #[cfg(test)]
    APP.with(use_proxy)
}

pub(crate) fn use_for_app(proxy: Option<&ProxySettings>) {
    with_app(|app| {
        *app.write().unwrap_or_else(PoisonError::into_inner) =
            proxy.filter(|proxy| proxy.global).cloned();
    });
}

fn app() -> Option<ProxySettings> {
    with_app(|app| app.read().unwrap_or_else(PoisonError::into_inner).clone())
}

/// Sends the client's requests through the app's proxy, if there is one. A
/// proxy that can't be used fails the request instead of going around it.
pub(crate) fn configure(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::Result<reqwest::blocking::ClientBuilder> {
    let Some(proxy) = app() else {
        return Ok(builder);
    };
    let mut route = reqwest::Proxy::all(proxy.url_with_remote_names())?.no_proxy(
        reqwest::NoProxy::from_string(&proxy.bypass_hosts().join(",")),
    );
    if !proxy.username.is_empty() && proxy.password.is_empty() {
        route = route.basic_auth(&proxy.username, "");
    }
    Ok(builder.proxy(route))
}

/// How ssh reaches other computers through `proxy`.
pub(crate) fn ssh_command(proxy: &ProxySettings) -> io::Result<ProxyCommand> {
    Ok(ProxyCommand {
        program: std::env::current_exe()?,
        arguments: vec![TUNNEL.into()],
        environment: vec![(
            SETTINGS.into(),
            serde_json::to_string(proxy)
                .map_err(io::Error::other)?
                .into(),
        )],
    })
}

/// Runs as ssh's proxy command for `<host> <port>`.
pub(crate) fn run_tunnel(mut arguments: impl Iterator<Item = std::ffi::OsString>) -> ExitCode {
    let result = (|| {
        let host = arguments
            .next()
            .and_then(|host| host.into_string().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing host"))?;
        let port = arguments
            .next()
            .and_then(|port| port.to_str()?.parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing port"))?;
        let proxy: ProxySettings = serde_json::from_str(
            &std::env::var(SETTINGS)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "missing proxy"))?,
        )
        .map_err(io::Error::other)?;
        relay(&connect(&proxy, &host, port)?)
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "Muxy proxy: {error}");
            ExitCode::FAILURE
        }
    }
}

/// A connection to `host:port` through `proxy`, or straight there when
/// `proxy` bypasses it, ready for the caller's bytes.
pub(crate) fn connect(proxy: &ProxySettings, host: &str, port: u16) -> io::Result<TcpStream> {
    if proxy.bypasses(host) {
        return open(host, port);
    }
    let mut stream = open(&proxy.host, proxy.port)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    match proxy.kind {
        ProxyKind::Http => http_connect(&mut stream, proxy, host, port)?,
        ProxyKind::Socks5 => socks5_connect(&mut stream, proxy, host, port)?,
    }
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    Ok(stream)
}

fn open(host: &str, port: u16) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, format!("{host} has no address"));
    for address in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&address, TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = error,
        }
    }
    Err(io::Error::new(
        last.kind(),
        format!("could not reach {host}:{port}: {last}"),
    ))
}

fn http_connect(
    stream: &mut TcpStream,
    proxy: &ProxySettings,
    host: &str,
    port: u16,
) -> io::Result<()> {
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if !proxy.username.is_empty() {
        let credentials = base64(format!("{}:{}", proxy.username, proxy.password).as_bytes());
        let _ = write!(request, "Proxy-Authorization: Basic {credentials}\r\n");
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes())?;
    let mut reply = Vec::new();
    while !reply.ends_with(b"\r\n\r\n") {
        if reply.len() == MAX_REPLY {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the proxy's reply is too long",
            ));
        }
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        reply.push(byte[0]);
    }
    let status = String::from_utf8_lossy(&reply)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let code = status.split_whitespace().nth(1).unwrap_or_default();
    if status.starts_with("HTTP/1.") && code.len() == 3 && code.starts_with('2') {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("the proxy refused {authority}: {status}"),
        ))
    }
}

fn socks5_connect(
    stream: &mut TcpStream,
    proxy: &ProxySettings,
    host: &str,
    port: u16,
) -> io::Result<()> {
    let invalid = |message: &str| io::Error::new(io::ErrorKind::InvalidData, message.to_owned());
    let login = !proxy.username.is_empty();
    stream.write_all(if login { &[5, 2, 0, 2] } else { &[5, 1, 0] })?;
    let mut choice = [0; 2];
    stream.read_exact(&mut choice)?;
    match choice {
        [5, 0] => {}
        [5, 2] if login => {
            let (user, password) = (proxy.username.as_bytes(), proxy.password.as_bytes());
            let (Ok(user_length), Ok(password_length)) =
                (u8::try_from(user.len()), u8::try_from(password.len()))
            else {
                return Err(invalid("the proxy user name or password is too long"));
            };
            let mut request = vec![1, user_length];
            request.extend_from_slice(user);
            request.push(password_length);
            request.extend_from_slice(password);
            stream.write_all(&request)?;
            let mut status = [0; 2];
            stream.read_exact(&mut status)?;
            if status[1] != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the proxy refused the user name or password",
                ));
            }
        }
        [5, _] => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the proxy needs a login it wasn't given",
            ));
        }
        _ => return Err(invalid("the proxy doesn't speak SOCKS5")),
    }
    let mut request = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            request.push(1);
            request.extend_from_slice(&address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            request.push(4);
            request.extend_from_slice(&address.octets());
        }
        Err(_) => {
            let length =
                u8::try_from(host.len()).map_err(|_| invalid("the host name is too long"))?;
            request.push(3);
            request.push(length);
            request.extend_from_slice(host.as_bytes());
        }
    }
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request)?;
    let mut reply = [0; 4];
    stream.read_exact(&mut reply)?;
    if reply[0] != 5 {
        return Err(invalid("the proxy doesn't speak SOCKS5"));
    }
    if reply[1] != 0 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!(
                "the proxy refused {host}:{port}: {}",
                socks5_failure(reply[1])
            ),
        ));
    }
    let address = match reply[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut length = [0];
            stream.read_exact(&mut length)?;
            usize::from(length[0])
        }
        _ => return Err(invalid("the proxy sent an unknown address type")),
    };
    let mut bound = vec![0; address + 2];
    stream.read_exact(&mut bound)
}

fn socks5_failure(code: u8) -> &'static str {
    match code {
        2 => "not allowed by its rules",
        3 => "network unreachable",
        4 => "host unreachable",
        5 => "connection refused",
        6 => "timed out",
        _ => "connection failed",
    }
}

/// Copies standard input to `stream` and `stream` to standard output until
/// the computer closes the connection.
fn relay(stream: &TcpStream) -> io::Result<()> {
    let mut upstream = stream.try_clone()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut upstream);
        let _ = upstream.shutdown(Shutdown::Write);
    });
    let mut stdout = io::stdout().lock();
    let mut buffer = [0; 16 * 1024];
    let mut stream = stream;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        stdout.write_all(&buffer[..read])?;
        stdout.flush()?;
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | u32::from(*byte) << (16 - 8 * index)
            });
        for index in 0..4 {
            encoded.push(if index <= chunk.len() {
                char::from(ALPHABET[(value >> (18 - 6 * index) & 63) as usize])
            } else {
                '='
            });
        }
    }
    encoded
}

#[cfg(test)]
mod tests;
