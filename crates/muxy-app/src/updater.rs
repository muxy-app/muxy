#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod replacement;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unsupported;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) use unsupported::{CAPABILITY, Installation, PreparedUpdate};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::io::{Read, Write};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::time::Duration;

use muxy_core::release;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use muxy_core::release::Channel;
#[cfg(any(target_os = "macos", target_os = "linux", test))]
use muxy_ui::tr;

#[cfg(target_os = "linux")]
pub(crate) use linux::{CAPABILITY, Installation, PreparedUpdate};
#[cfg(target_os = "macos")]
pub(crate) use macos::{CAPABILITY, Installation, PreparedUpdate};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) const RELEASES: &str = "https://github.com/muxy-app/muxy/releases/download";
#[cfg(any(target_os = "macos", target_os = "linux", test))]
const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024 * 1024;

/// Betas share a rolling feed; each stable release carries its own, and the latest one wins.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn feed(channel: Channel) -> &'static str {
    match channel {
        Channel::Beta => "https://github.com/muxy-app/muxy/releases/download/beta-2.x/update.json",
        Channel::Stable => "https://github.com/muxy-app/muxy/releases/latest/download/update.json",
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Release {
    pub(crate) version: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

impl Release {
    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    pub(crate) fn fixture(version: &str) -> Self {
        Self {
            version: version.into(),
            url: String::new(),
            size: 1,
            sha256: None,
        }
    }

    pub(crate) fn is_newer_than(&self, version: &str) -> bool {
        release::is_newer(&self.version, version)
    }

    #[cfg(any(target_os = "macos", target_os = "linux", test))]
    fn parse(bytes: &[u8], current: &str, platform: &str, arch: &str) -> Result<Option<Self>> {
        let metadata: serde_json::Value = serde_json::from_slice(bytes)?;
        if metadata["schema"].as_u64() != Some(1) {
            return Err(tr!("Unsupported update feed").to_string().into());
        }
        let version = metadata["version"]
            .as_str()
            .ok_or_else(|| tr!("Missing update version").to_string())?;
        let next = release::Version::parse(version)
            .ok_or_else(|| tr!("Invalid update version").to_string())?;
        let current = release::Version::parse(current)
            .ok_or_else(|| tr!("This is not a released version").to_string())?;
        if !next.is_newer_than(current) {
            return Ok(None);
        }
        let asset = &metadata["platforms"][platform];
        let url = asset["url"]
            .as_str()
            .ok_or_else(|| tr!("No update for this platform").to_string())?;
        let filename = match platform {
            "macos-aarch64" | "macos-x86_64" => format!("Muxy-{version}-{arch}.dmg"),
            "linux-aarch64" | "linux-x86_64" => {
                format!("muxy-desktop-{version}-linux-{arch}.tar.gz")
            }
            _ => return Err(tr!("No update for this platform").to_string().into()),
        };
        if url != format!("{RELEASES}/v{version}/{filename}") {
            return Err(tr!("Unexpected update download location")
                .to_string()
                .into());
        }
        let size = asset["size"]
            .as_u64()
            .ok_or_else(|| tr!("Missing update size").to_string())?;
        if size == 0 || size > MAX_DOWNLOAD {
            return Err(tr!("Invalid update download size").to_string().into());
        }
        let sha256 = asset["sha256"].as_str().map(str::to_ascii_lowercase);
        if (platform.starts_with("linux-") && sha256.is_none())
            || sha256.as_ref().is_some_and(|hash| {
                hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err(tr!("Invalid update checksum").to_string().into());
        }
        Ok(Some(Self {
            version: version.into(),
            url: url.into(),
            size,
            sha256,
        }))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn client() -> Result<reqwest::blocking::Client> {
    Ok(crate::proxy::configure(
        reqwest::blocking::Client::builder()
            .user_agent(concat!("Muxy/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(600)),
    )?
    .build()?)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn latest(
    client: &reqwest::blocking::Client,
    platform: &str,
    arch: &str,
) -> Result<Option<Release>> {
    let mut bytes = Vec::new();
    client
        .get(feed(Channel::current()))
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()?
        .error_for_status()?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err(tr!("Update feed is too large").to_string().into());
    }
    Release::parse(&bytes, env!("CARGO_PKG_VERSION"), platform, arch)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn download(
    client: &reqwest::blocking::Client,
    release: &Release,
    path: &std::path::Path,
) -> Result<()> {
    let response = client.get(&release.url).send()?.error_for_status()?;
    let mut output = std::fs::File::create(path)?;
    let count = std::io::copy(&mut response.take(release.size + 1), &mut output)?;
    if count != release.size {
        return Err(
            tr!("The update download is incomplete or has an unexpected size")
                .to_string()
                .into(),
        );
    }
    output.flush()?;
    if let Some(expected) = &release.sha256 {
        let mut command = if cfg!(target_os = "macos") {
            let mut command = std::process::Command::new("/usr/bin/shasum");
            command.args(["-a", "256"]);
            command
        } else {
            std::process::Command::new("/usr/bin/sha256sum")
        };
        let result = command.arg(path).output()?;
        if !result.status.success()
            || String::from_utf8_lossy(&result.stdout)
                .split_whitespace()
                .next()
                .is_none_or(|hash| !hash.eq_ignore_ascii_case(expected))
        {
            return Err(tr!("Update download checksum mismatch").to_string().into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(version: &str) -> serde_json::Value {
        serde_json::json!({"schema": 1, "version": version, "platforms": {
            "macos-aarch64": {"url": format!("{RELEASES}/v{version}/Muxy-{version}-arm64.dmg"), "size": 123}
        }})
    }

    #[cfg(any(target_os = "macos", test))]
    fn parse(feed: &serde_json::Value) -> Result<Option<Release>> {
        parse_from(feed, "2.0.0-beta.9")
    }

    fn parse_from(feed: &serde_json::Value, current: &str) -> Result<Option<Release>> {
        Release::parse(
            feed.to_string().as_bytes(),
            current,
            "macos-aarch64",
            "arm64",
        )
    }

    #[test]
    fn updates_only_to_a_newer_release_of_the_same_channel() {
        for version in ["2.0.0-beta.10", "2.1.0-beta.10"] {
            assert!(parse(&feed(version)).is_ok_and(|release| release.is_some()));
        }
        for version in ["2.0.0-beta.8", "2.0.0-beta.9", "2.0.0", "3.0.0"] {
            assert!(parse(&feed(version)).is_ok_and(|release| release.is_none()));
        }
        assert!(parse_from(&feed("2.0.1"), "2.0.0").is_ok_and(|release| release.is_some()));
        for version in ["2.0.0", "1.9.9", "2.0.1-beta.10"] {
            assert!(parse_from(&feed(version), "2.0.0").is_ok_and(|release| release.is_none()));
        }
        assert!(parse_from(&feed("2.0.1"), "2.0.0-beta-0").is_err());
        for version in [
            "2.0.0-alpha.99",
            "2.0-beta.99",
            "2.0.0.0-beta.99",
            "2.x.0-beta.99",
            "2.0.0-beta.0",
            "2.0.0-beta.01",
            "2.0.0-beta.+10",
            "2.0.0-beta.99-beta.100",
            "2.0.0-beta.999999999999999999999",
        ] {
            assert!(parse(&feed(version)).is_err(), "{version}");
        }
    }

    #[test]
    fn rejects_wrong_schema_platform_location_and_size() {
        let mut data = feed("2.0.0-beta.10");
        data["schema"] = 2.into();
        assert!(parse(&data).is_err());
        for url in [
            "http://github.com/file.dmg",
            "https://example.com/file.dmg",
            "https://github.com/muxy-app/muxy/releases/download/v2.0.0-beta.9/Muxy-2.0.0-beta.9-arm64.dmg",
        ] {
            let mut data = feed("2.0.0-beta.10");
            data["platforms"]["macos-aarch64"]["url"] = url.into();
            assert!(parse(&data).is_err());
        }
        for size in [0, MAX_DOWNLOAD + 1] {
            let mut data = feed("2.0.0-beta.10");
            data["platforms"]["macos-aarch64"]["size"] = size.into();
            assert!(parse(&data).is_err());
        }
        assert!(
            Release::parse(
                feed("2.0.0-beta.10").to_string().as_bytes(),
                "2.0.0-beta.9",
                "windows-x86_64",
                "x86_64"
            )
            .is_err()
        );
    }

    #[test]
    fn linux_updates_require_the_desktop_archive_and_checksum() {
        let version = "2.0.0-beta.10";
        let url = format!("{RELEASES}/v{version}/muxy-desktop-{version}-linux-arm64.tar.gz");
        let mut data = serde_json::json!({"schema": 1, "version": version, "platforms": {
            "linux-aarch64": {"url": url, "size": 100, "sha256": "a".repeat(64)}
        }});
        let parse = |data: &serde_json::Value| {
            Release::parse(
                data.to_string().as_bytes(),
                "2.0.0-beta.9",
                "linux-aarch64",
                "arm64",
            )
        };
        assert!(parse(&data).is_ok_and(|release| release.is_some()));
        for checksum in [
            serde_json::Value::Null,
            "abcd".into(),
            "z".repeat(64).into(),
        ] {
            data["platforms"]["linux-aarch64"]["sha256"] = checksum;
            assert!(parse(&data).is_err());
        }
        data["platforms"]["linux-aarch64"]["sha256"] = "a".repeat(64).into();
        data["platforms"]["linux-aarch64"]["url"] =
            format!("{RELEASES}/v{version}/muxy-{version}-linux-arm64.tar.gz").into();
        assert!(parse(&data).is_err());
    }
}
