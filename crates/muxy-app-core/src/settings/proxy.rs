//! The app's one proxy. It lives in `proxy.toml`, apart from `settings.toml`,
//! because it can hold a password: only this user can read it, and backups
//! leave it out.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings::{Error, Result};

/// Hosts a new proxy reaches directly.
pub const DEFAULT_PROXY_BYPASS: &str = "localhost, 127.0.0.1, ::1";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    #[default]
    Http,
    Socks5,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProxySettings {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Hosts reached directly, as a `no_proxy` list.
    pub bypass: String,
    /// New terminals on this computer, the app's own requests, and ssh to
    /// other computers all use the proxy.
    pub global: bool,
}

impl ProxySettings {
    /// No file means no proxy.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::new(path.display().to_string(), error)),
        };
        let proxy: Self = toml::from_str(&source).map_err(|error: toml::de::Error| {
            let message = error
                .span()
                .and_then(|span| source.get(..span.start))
                .map_or_else(
                    || "invalid proxy settings".to_owned(),
                    |prefix| {
                        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
                        format!("invalid proxy settings at line {line}")
                    },
                );
            Error::new(path.display().to_string(), message)
        })?;
        proxy.validate()?;
        Ok(Some(proxy))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let context = || path.display().to_string();
        let source = toml::to_string_pretty(self).map_err(|error| Error::new(context(), error))?;
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        let result = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .and_then(|mut file| {
                file.write_all(source.as_bytes())?;
                file.sync_all()
            })
            .and_then(|()| fs::rename(&temporary, path));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(Error::new(context(), error));
        }
        Ok(())
    }

    pub fn remove(path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                Err(Error::new(path.display().to_string(), error))
            }
            _ => Ok(()),
        }
    }

    pub fn validate(&self) -> Result<()> {
        let host = self.host.trim();
        if host.is_empty()
            || host != self.host
            || host.chars().any(|character| {
                character.is_whitespace() || character.is_control() || "/@?#[]".contains(character)
            })
            || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
        {
            return Err(Error::new("proxy.host", "enter a host name or address"));
        }
        if self.port == 0 {
            return Err(Error::new("proxy.port", "enter a port"));
        }
        if self.username.is_empty() && !self.password.is_empty() {
            return Err(Error::new("proxy.username", "enter the user name"));
        }
        for (name, value) in [
            ("username", &self.username),
            ("password", &self.password),
            ("bypass", &self.bypass),
        ] {
            if value.chars().any(char::is_control) {
                return Err(Error::new(
                    format!("proxy.{name}"),
                    "must not contain control characters",
                ));
            }
        }
        Ok(())
    }

    /// As programs read it from `http_proxy` and the like.
    pub fn url(&self) -> String {
        self.url_with_scheme(match self.kind {
            ProxyKind::Http => "http",
            ProxyKind::Socks5 => "socks5",
        })
    }

    /// Like [`Self::url`], but a SOCKS proxy also looks up host names, so
    /// names this computer can't resolve still work.
    pub fn url_with_remote_names(&self) -> String {
        self.url_with_scheme(match self.kind {
            ProxyKind::Http => "http",
            ProxyKind::Socks5 => "socks5h",
        })
    }

    fn url_with_scheme(&self, scheme: &str) -> String {
        let credentials = match (self.username.as_str(), self.password.as_str()) {
            ("", _) => String::new(),
            (user, "") => format!("{}@", percent_encode(user)),
            (user, password) => format!("{}:{}@", percent_encode(user), percent_encode(password)),
        };
        format!(
            "{scheme}://{credentials}{}:{}",
            self.bracketed_host(),
            self.port
        )
    }

    /// An IPv6 address needs brackets before its port.
    pub fn bracketed_host(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }

    /// The hosts reached directly, one per entry.
    pub fn bypass_hosts(&self) -> Vec<&str> {
        self.bypass
            .split([',', ' ', '\t', '\n'])
            .filter(|host| !host.is_empty())
            .collect()
    }

    pub fn bypasses(&self, host: &str) -> bool {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let address = host.parse::<IpAddr>().ok();
        self.bypass_hosts().into_iter().any(|entry| {
            let domain = entry.trim_start_matches('.');
            entry == "*"
                || address.is_some_and(|address| subnet_contains(entry, address))
                || host.eq_ignore_ascii_case(domain)
                || host
                    .len()
                    .checked_sub(domain.len() + 1)
                    .is_some_and(|start| {
                        host.as_bytes()[start] == b'.'
                            && host[start + 1..].eq_ignore_ascii_case(domain)
                    })
        })
    }

    /// Variables that send a terminal's programs through the proxy. Both
    /// spellings, since programs disagree on which one they read.
    pub fn environment(&self) -> BTreeMap<String, String> {
        let url = self.url();
        let mut environment = BTreeMap::new();
        for name in ["http_proxy", "https_proxy", "all_proxy"] {
            environment.insert(name.to_owned(), url.clone());
            environment.insert(name.to_uppercase(), url.clone());
        }
        let bypass = self.bypass_hosts().join(",");
        environment.insert("no_proxy".to_owned(), bypass.clone());
        environment.insert("NO_PROXY".to_owned(), bypass);
        environment
    }
}

fn subnet_contains(entry: &str, address: IpAddr) -> bool {
    let Some((network, prefix)) = entry.split_once('/') else {
        return false;
    };
    let (Ok(network), Ok(prefix)) = (network.parse::<IpAddr>(), prefix.parse::<u32>()) else {
        return false;
    };
    let (address, network, bits) = match (address, network) {
        (IpAddr::V4(address), IpAddr::V4(network)) => (
            u128::from(u32::from(address)),
            u128::from(u32::from(network)),
            32_u32,
        ),
        (IpAddr::V6(address), IpAddr::V6(network)) => {
            (u128::from(address), u128::from(network), 128_u32)
        }
        _ => return false,
    };
    bits.checked_sub(prefix)
        .is_some_and(|shift| (address ^ network).checked_shr(shift).unwrap_or(0) == 0)
}

/// Keeps only unreserved URL characters as they are.
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn proxy() -> ProxySettings {
        ProxySettings {
            host: "proxy.local".into(),
            port: 8080,
            ..ProxySettings::default()
        }
    }

    #[test]
    fn urls_escape_credentials_and_bracket_ipv6_hosts() {
        assert_eq!(proxy().url(), "http://proxy.local:8080");
        let socks = ProxySettings {
            kind: ProxyKind::Socks5,
            host: "::1".into(),
            port: 1080,
            username: "me@work".into(),
            password: "p:ss/word".into(),
            ..ProxySettings::default()
        };
        assert_eq!(socks.url(), "socks5://me%40work:p%3Ass%2Fword@[::1]:1080");
        assert_eq!(
            socks.url_with_remote_names(),
            "socks5h://me%40work:p%3Ass%2Fword@[::1]:1080"
        );
        let user_only = ProxySettings {
            username: "me".into(),
            ..proxy()
        };
        assert_eq!(user_only.url(), "http://me@proxy.local:8080");
    }

    #[test]
    fn terminals_get_both_spellings_and_the_bypass_list() {
        let proxy = ProxySettings {
            bypass: "localhost, 127.0.0.1  .internal".into(),
            ..proxy()
        };
        let environment = proxy.environment();
        for name in [
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
        ] {
            assert_eq!(environment[name], "http://proxy.local:8080");
        }
        assert_eq!(environment["no_proxy"], "localhost,127.0.0.1,.internal");
        assert_eq!(environment["NO_PROXY"], "localhost,127.0.0.1,.internal");
        let cleared = self::proxy().environment();
        assert_eq!(cleared["no_proxy"], "");
        assert_eq!(cleared["NO_PROXY"], "");
    }

    #[test]
    fn bypassed_hosts_match_as_no_proxy_does() {
        let proxy = ProxySettings {
            bypass: "localhost, 127.0.0.1 ::1 .internal example.com".into(),
            ..proxy()
        };
        for host in [
            "localhost",
            "127.0.0.1",
            "::1",
            "[::1]",
            "box.internal",
            "internal",
            "example.com",
            "git.EXAMPLE.com",
        ] {
            assert!(proxy.bypasses(host), "{host}");
        }
        for host in ["box.example", "notexample.com", "127.0.0.2", "internal.box"] {
            assert!(!proxy.bypasses(host), "{host}");
        }
        let everything = ProxySettings {
            bypass: "*".into(),
            ..self::proxy()
        };
        assert!(everything.bypasses("anything"));
        assert!(!self::proxy().bypasses("localhost"));
    }

    #[test]
    fn cidr_bypasses_respect_address_families_and_subnet_boundaries() {
        for (bypass, host, expected) in [
            ("10.20.30.0/24", "10.20.30.255", true),
            ("10.20.30.100/24", "10.20.30.1", true),
            ("10.20.30.0/24", "10.20.31.0", false),
            ("10.20.30.0/32", "10.20.30.0", true),
            ("10.20.30.0/32", "10.20.30.1", false),
            ("0.0.0.0/0", "192.0.2.1", true),
            ("0.0.0.0/0", "::1", false),
            ("2001:db8::/33", "2001:db8:7fff::1", true),
            ("2001:db8::/33", "2001:db8:8000::1", false),
            ("2001:db8::1/128", "[2001:db8::1]", true),
            ("2001:db8::1/128", "2001:db8::2", false),
            ("::/0", "2001:db8::1", true),
            ("::/0", "127.0.0.1", false),
            ("127.0.0.0/8", "localhost", false),
            ("127.0.0.1/33", "127.0.0.1", false),
            ("::1/129", "::1", false),
            ("::1/-1", "::1", false),
            ("invalid/8", "127.0.0.1", false),
        ] {
            let proxy = ProxySettings {
                bypass: bypass.into(),
                ..proxy()
            };
            assert_eq!(proxy.bypasses(host), expected, "{bypass}: {host}");
        }
    }

    #[test]
    fn malformed_settings_do_not_expose_credentials() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("proxy.toml");
        for source in [
            "username = \"me\"\npassword = \"FAKE_REVIEW_SECRET\" extra\n",
            "port = \"FAKE_REVIEW_SECRET\"\n",
        ] {
            fs::write(&path, source).map_err(|error| Error::new("test", error))?;
            let error = ProxySettings::load(&path)
                .expect_err("invalid settings")
                .to_string();
            assert!(!error.contains("FAKE_REVIEW_SECRET"));
            assert!(error.contains("invalid proxy settings"));
            assert!(error.contains("proxy.toml"));
        }
        Ok(())
    }

    #[test]
    fn invalid_proxies_are_refused() {
        for invalid in [
            ProxySettings {
                host: String::new(),
                ..proxy()
            },
            ProxySettings {
                host: "http://proxy".into(),
                ..proxy()
            },
            ProxySettings {
                host: " proxy".into(),
                ..proxy()
            },
            ProxySettings { port: 0, ..proxy() },
            ProxySettings {
                host: "proxy.local:3128".into(),
                ..proxy()
            },
            ProxySettings {
                password: "secret".into(),
                ..proxy()
            },
            ProxySettings {
                username: "me\n".into(),
                ..proxy()
            },
        ] {
            assert!(invalid.validate().is_err(), "{invalid:?}");
        }
        assert!(proxy().validate().is_ok());
        assert!(
            ProxySettings {
                host: "fe80::1".into(),
                ..proxy()
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn the_file_is_private_and_missing_means_no_proxy() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("proxy.toml");
        assert_eq!(ProxySettings::load(&path)?, None);
        let saved = ProxySettings {
            username: "me".into(),
            password: "secret".into(),
            global: true,
            ..proxy()
        };
        saved.save(&path)?;
        let mode = fs::metadata(&path)
            .map_err(|error| Error::new("test", error))?
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(ProxySettings::load(&path)?, Some(saved));
        ProxySettings::remove(&path)?;
        ProxySettings::remove(&path)?;
        assert_eq!(ProxySettings::load(&path)?, None);
        Ok(())
    }
}
