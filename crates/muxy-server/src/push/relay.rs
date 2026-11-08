//! The muxy.app push relay's HTTP API.

use std::fmt;
use std::time::Duration;

use muxy_protocol::{PushEnvironment, PushPlatform, PushToken};
use serde::{Deserialize, Serialize};
use ureq::http::{HeaderMap, Uri};

/// Points the server at another relay, such as one on this computer for testing.
pub(crate) const RELAY_VARIABLE: &str = "MUXY_PUSH_RELAY";
const DEFAULT_RELAY: &str = "https://muxy.app";
const PREFIX: &str = "/api/push/registrations";
const MAX_REPLY: u64 = 64 * 1024;
const ID_LENGTH: usize = 26;
const SECRET_LENGTH: usize = 43;

/// What identifies and authorizes one registration at the relay.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Credentials {
    pub(crate) id: String,
    pub(crate) secret: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Credentials {
    /// The relay's shapes, so an id can never reach outside its path.
    pub(crate) fn new(id: String, secret: String) -> Option<Self> {
        let valid = id.len() == ID_LENGTH
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
            && secret.len() == SECRET_LENGTH
            && secret
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        valid.then_some(Self { id, secret })
    }
}

/// One notification as the relay forwards it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Outgoing {
    pub(crate) kind: &'static str,
    pub(crate) payload: String,
    pub(crate) collapse_id: String,
    pub(crate) thread_id: String,
    pub(crate) ttl: u64,
}

/// How the relay answered, by what the server does next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelayError {
    /// `401`: forget the registration.
    Unauthorized,
    /// `404`: register again.
    NotFound,
    /// `410`: APNs or FCM rejected the token.
    Gone,
    /// `422` or another refusal: don't retry as it is.
    Refused,
    /// `429`.
    RateLimited(Option<Duration>),
    /// `503`, another `5xx`, or no answer: try again later.
    Unavailable(Option<Duration>),
}

impl RelayError {
    /// The relay may take the same call later.
    pub(crate) fn retryable(self) -> bool {
        matches!(self, Self::RateLimited(_) | Self::Unavailable(_))
    }

    /// When the relay asked to be called again.
    pub(crate) fn retry_after(self) -> Option<Duration> {
        match self {
            Self::RateLimited(after) | Self::Unavailable(after) => after,
            Self::Unauthorized | Self::NotFound | Self::Gone | Self::Refused => None,
        }
    }
}

pub(crate) trait Relay: Send + Sync {
    fn register(&self, token: &PushToken) -> Result<Credentials, RelayError>;
    fn replace(&self, credentials: &Credentials, token: &PushToken) -> Result<(), RelayError>;
    fn delete(&self, credentials: &Credentials) -> Result<(), RelayError>;
    fn send(&self, credentials: &Credentials, notification: &Outgoing) -> Result<(), RelayError>;
}

#[derive(Serialize)]
struct Registration<'a> {
    platform: &'static str,
    token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<&'static str>,
}

#[derive(Deserialize)]
struct Created {
    data: CreatedData,
}

#[derive(Deserialize)]
struct CreatedData {
    id: String,
    secret: String,
}

/// The relay over HTTPS, trusting the web PKI and honouring the usual proxy
/// variables. Redirects aren't followed, so the token and secret go nowhere else.
pub(crate) struct HttpRelay {
    base: Option<String>,
    agent: ureq::Agent,
}

impl fmt::Debug for HttpRelay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRelay")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl HttpRelay {
    pub(crate) fn from_environment() -> Self {
        let configured = std::env::var(RELAY_VARIABLE).ok();
        let base = configured.as_deref().map_or_else(
            || Some(DEFAULT_RELAY.to_owned()),
            |configured| {
                let base = base_url(configured);
                if base.is_none() {
                    log::error!(
                        "{RELAY_VARIABLE} must be an https:// address, or http:// on this computer; push notifications are paused"
                    );
                }
                base
            },
        );
        Self::new(base)
    }

    /// A plain-HTTP relay is on this computer, and is reached directly so the
    /// secret never crosses a network, not even to a proxy.
    pub(crate) fn new(base: Option<String>) -> Self {
        let local = base
            .as_deref()
            .is_some_and(|base| base.starts_with("http://"));
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .proxy(if local {
                None
            } else {
                ureq::Proxy::try_from_env()
            })
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .user_agent(format!("muxy-server/{}", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            base,
            agent: config.into(),
        }
    }

    fn url(&self, path: &str) -> Result<String, RelayError> {
        self.base
            .as_ref()
            .map(|base| format!("{base}{PREFIX}{path}"))
            .ok_or(RelayError::Unavailable(None))
    }

    fn call(
        &self,
        method: Method,
        path: &str,
        secret: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, RelayError> {
        let url = self.url(path)?;
        let authorization = secret.map(|secret| format!("Bearer {secret}"));
        let result = match (method, body) {
            (Method::Post | Method::Put, body) => {
                let request = if method == Method::Post {
                    self.agent.post(&url)
                } else {
                    self.agent.put(&url)
                };
                let request = request
                    .header("Accept", "application/json")
                    .header("Content-Type", "application/json");
                let request = match &authorization {
                    Some(value) => request.header("Authorization", value),
                    None => request,
                };
                request.send(body.unwrap_or_default())
            }
            (Method::Delete, _) => {
                let request = self.agent.delete(&url).header("Accept", "application/json");
                match &authorization {
                    Some(value) => request.header("Authorization", value),
                    None => request,
                }
                .call()
            }
        };
        let mut response = result.map_err(|error| {
            log::warn!("push relay unreachable: {}", describe(&error));
            RelayError::Unavailable(None)
        })?;
        let status = response.status().as_u16();
        if let Some(error) = failure(status, response.headers()) {
            return Err(error);
        }
        response
            .body_mut()
            .with_config()
            .limit(MAX_REPLY)
            .read_to_vec()
            .map_err(|_| RelayError::Unavailable(None))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Method {
    Post,
    Put,
    Delete,
}

impl Relay for HttpRelay {
    fn register(&self, token: &PushToken) -> Result<Credentials, RelayError> {
        let reply = self.call(Method::Post, "", None, Some(registration(token)?))?;
        let created: Created =
            serde_json::from_slice(&reply).map_err(|_| RelayError::Unavailable(None))?;
        Credentials::new(created.data.id, created.data.secret).ok_or(RelayError::Unavailable(None))
    }

    fn replace(&self, credentials: &Credentials, token: &PushToken) -> Result<(), RelayError> {
        self.call(
            Method::Put,
            &format!("/{}", credentials.id),
            Some(&credentials.secret),
            Some(registration(token)?),
        )
        .map(drop)
    }

    fn delete(&self, credentials: &Credentials) -> Result<(), RelayError> {
        self.call(
            Method::Delete,
            &format!("/{}", credentials.id),
            Some(&credentials.secret),
            None,
        )
        .map(drop)
    }

    fn send(&self, credentials: &Credentials, notification: &Outgoing) -> Result<(), RelayError> {
        let body = serde_json::to_vec(notification).map_err(|_| RelayError::Refused)?;
        self.call(
            Method::Post,
            &format!("/{}/notifications", credentials.id),
            Some(&credentials.secret),
            Some(body),
        )
        .map(drop)
    }
}

/// Names the failure without the address, which holds the registration id.
fn describe(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::Io(error) => error.kind().to_string(),
        ureq::Error::HostNotFound => "host not found".into(),
        ureq::Error::ConnectionFailed => "connection failed".into(),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "TLS failed".into(),
        _ => "request failed".into(),
    }
}

fn registration(token: &PushToken) -> Result<Vec<u8>, RelayError> {
    let platform = match token.platform {
        PushPlatform::Apns => "apns",
        PushPlatform::Fcm => "fcm",
        PushPlatform::Unrecognized(_) => return Err(RelayError::Refused),
    };
    let environment = match token.environment {
        None => None,
        Some(PushEnvironment::Sandbox) => Some("sandbox"),
        Some(PushEnvironment::Production) => Some("production"),
        Some(PushEnvironment::Unrecognized(_)) => return Err(RelayError::Refused),
    };
    serde_json::to_vec(&Registration {
        platform,
        token: &token.token,
        environment,
    })
    .map_err(|_| RelayError::Refused)
}

/// `None` for success.
fn failure(status: u16, headers: &HeaderMap) -> Option<RelayError> {
    let retry_after = || {
        headers
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok())
            .map(Duration::from_secs)
    };
    match status {
        200..=299 => None,
        401 => Some(RelayError::Unauthorized),
        404 => Some(RelayError::NotFound),
        410 => Some(RelayError::Gone),
        429 => Some(RelayError::RateLimited(retry_after())),
        500..=599 => Some(RelayError::Unavailable(retry_after())),
        _ => Some(RelayError::Refused),
    }
}

/// The relay's origin, from `https://host[:port]`, or `http://` for this
/// computer only, so a secret never crosses a network in the clear.
pub(crate) fn base_url(text: &str) -> Option<String> {
    let uri: Uri = text.trim().parse().ok()?;
    let host = uri.host()?;
    let secure = match uri.scheme_str()? {
        "https" => true,
        "http" => false,
        _ => return None,
    };
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1");
    if !secure && !loopback {
        return None;
    }
    if uri.query().is_some() || !matches!(uri.path(), "" | "/") {
        return None;
    }
    let authority = uri.authority()?.as_str();
    if authority.contains('@') {
        return None;
    }
    Some(format!("{}://{authority}", uri.scheme_str()?))
}

#[cfg(test)]
mod tests;
