#[cfg(target_os = "macos")]
mod macos;
mod replacement;

use std::io::{Read, Write};
use std::time::Duration;

use muxy_core::release::{self, Channel};
use muxy_ui::tr;

#[cfg(target_os = "macos")]
pub(crate) use macos::{Installation, PreparedUpdate};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) const RELEASES: &str = "https://github.com/muxy-app/muxy/releases/download";
const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024 * 1024;

/// Betas share a rolling feed; each stable release carries its own, and the latest one wins.
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
}

impl Release {
    #[cfg(test)]
    pub(crate) fn fixture(version: &str) -> Self {
        Self {
            version: version.into(),
            url: String::new(),
            size: 1,
        }
    }

    pub(crate) fn is_newer_than(&self, version: &str) -> bool {
        release::is_newer(&self.version, version)
    }

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
        if url != format!("{RELEASES}/v{version}/Muxy-{version}-{arch}.dmg") {
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
        Ok(Some(Self {
            version: version.into(),
            url: url.into(),
            size,
        }))
    }
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(concat!("Muxy/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(600))
        .build()?)
}

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
}
