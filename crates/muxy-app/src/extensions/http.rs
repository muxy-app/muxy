//! `muxy.http.fetch`: main's request validation, its private-network guard,
//! and the request itself. Blocking; callers run it on the network pool.

use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value, json};

const MAX_BODY: u64 = 10_485_760;
const MAX_TIMEOUT_MS: f64 = 120_000.0;
const METHODS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];

#[derive(Debug)]
pub(crate) struct Request {
    pub url: reqwest::Url,
    pub method: String,
    pub host: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    timeout: Duration,
}

pub(crate) fn validate(args: &Value) -> Result<Request, String> {
    let url = args["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .ok_or("http: http requires a url")?;
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| "http: invalid URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("http: only http and https URLs are allowed".into());
    }
    let method = args["method"]
        .as_str()
        .map(str::trim)
        .filter(|method| !method.is_empty())
        .unwrap_or("GET")
        .to_uppercase();
    if !METHODS.contains(&method.as_str()) {
        return Err(format!("http: unsupported method '{method}'"));
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or("http: URL has no host")?
        .to_owned();
    let headers = args["headers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| {
            !["host", "content-length", "connection"].contains(&name.to_lowercase().as_str())
        })
        .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
        .collect();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The timeout is clamped to 1..=120000 milliseconds first"
    )]
    let timeout = Duration::from_millis(
        args["timeoutMs"]
            .as_f64()
            .filter(|value| value.is_finite())
            .unwrap_or(30_000.0)
            .clamp(1.0, MAX_TIMEOUT_MS) as u64,
    );
    Ok(Request {
        url: parsed,
        method,
        host,
        headers,
        body: args["body"]
            .as_str()
            .filter(|body| !body.is_empty())
            .map(str::to_owned),
        timeout,
    })
}

fn private(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [a, b, ..] = address.octets();
            a == 0
                || a == 10
                || a == 127
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
        }
        IpAddr::V6(address) => {
            let first = address.segments()[0];
            address.is_loopback()
                || address.is_unspecified()
                || first & 0xffc0 == 0xfe80
                || first & 0xfe00 == 0xfc00
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|mapped| private(IpAddr::V4(mapped)))
        }
    }
}

/// Loopback, link-local, and private hosts are refused before any prompt.
/// Names are resolved first; a name that does not resolve is refused too.
pub(crate) fn blocked(host: &str) -> bool {
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host
            .strip_suffix("local")
            .is_some_and(|name| name.ends_with('.'))
    {
        return true;
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        return private(address);
    }
    public_addresses(&host).is_err()
}

fn public_addresses(host: &str) -> Result<Vec<SocketAddr>, String> {
    let addresses: Vec<_> = (host, 0)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|address| private(address.ip())) {
        return Err(format!(
            "blocked request to private or loopback host '{host}'"
        ));
    }
    Ok(addresses)
}

/// Resolves names when connecting and refuses private addresses there too, so
/// a host cannot pass the check before consent and then resolve elsewhere.
struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolved = public_addresses(name.as_str())
            .map(|addresses| Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
            .map_err(Into::into);
        Box::pin(std::future::ready(resolved))
    }
}

pub(crate) fn fetch(request: Request) -> Result<Value, String> {
    let failed = |error: reqwest::Error| format!("http request failed: {error}");
    let client = reqwest::blocking::Client::builder()
        .timeout(request.timeout)
        .user_agent("Muxy/2 Extensions")
        .no_proxy()
        .dns_resolver(Arc::new(PublicOnly))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                attempt.error("too many redirects")
            } else if attempt.url().host_str().is_none_or(blocked) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(failed)?;
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| format!("http request failed: {error}"))?;
    let head = method == reqwest::Method::HEAD;
    let mut builder = client.request(method, request.url);
    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }
    let response = builder.send().map_err(failed)?;
    let status = response.status().as_u16();
    let mut headers = Map::new();
    for (name, value) in response.headers() {
        let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
        headers
            .entry(name.as_str())
            .and_modify(|existing| {
                if let Value::String(existing) = existing {
                    existing.push_str(", ");
                    existing.push_str(&value);
                }
            })
            .or_insert_with(|| Value::String(value.clone()));
    }
    let mut bytes = Vec::new();
    if !head {
        response
            .take(MAX_BODY + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("http request failed: {error}"))?;
    }
    let truncated = bytes.len() as u64 > MAX_BODY;
    bytes.truncate(usize::try_from(MAX_BODY).unwrap_or(usize::MAX));
    Ok(json!({
        "status": status,
        "headers": headers,
        "body": String::from_utf8(bytes).unwrap_or_default(),
        "truncated": truncated,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_validated_in_main_order() {
        let error = |args: Value| validate(&args).unwrap_err();
        assert_eq!(error(json!({})), "http: http requires a url");
        assert_eq!(error(json!({"url":"not a url"})), "http: invalid URL");
        assert_eq!(
            error(json!({"url":"file:///etc/passwd"})),
            "http: only http and https URLs are allowed"
        );
        assert_eq!(
            error(json!({"url":"https://example.com","method":"trace"})),
            "http: unsupported method 'TRACE'"
        );
        let request = validate(&json!({
            "url":" https://api.github.com/repos ",
            "method":"post",
            "headers":{"Host":"evil","Accept":"application/json","X-Count":3},
            "timeoutMs":999_999
        }))
        .expect("request");
        assert_eq!(
            (request.method.as_str(), request.host.as_str()),
            ("POST", "api.github.com")
        );
        assert_eq!(
            request.headers,
            [("Accept".to_owned(), "application/json".to_owned())]
        );
        assert_eq!(request.timeout, Duration::from_secs(120));
    }

    #[test]
    fn private_and_loopback_hosts_are_blocked_before_any_request() {
        for host in [
            "localhost",
            "app.localhost",
            "printer.local",
            "127.0.0.1",
            "10.1.2.3",
            "172.20.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "[::1]",
            "::",
            "fe80::1",
            "fd00::1",
            "::ffff:10.0.0.1",
            "",
        ] {
            assert!(blocked(host), "{host} must be blocked");
        }
        for host in ["8.8.8.8", "172.32.0.1", "2606:4700::1111", "100.64.0.1"] {
            assert!(!blocked(host), "{host} must be allowed");
        }
    }
}
