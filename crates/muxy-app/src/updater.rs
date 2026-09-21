#[cfg(target_os = "macos")]
mod macos;
mod replacement;

use std::io::{Read, Write};
use std::time::Duration;

#[cfg(target_os = "macos")]
pub(crate) use macos::{Installation, PreparedUpdate};

pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const FEED: &str = "https://github.com/muxy-app/muxy/releases/download/beta-2.x/update.json";
const RELEASES: &str = "https://github.com/muxy-app/muxy/releases/download";
const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024 * 1024;

pub(crate) fn build_number(version: &str) -> Option<u64> {
    let number = version.strip_prefix("2.0.0-beta-")?;
    if number.starts_with('0') || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
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
        build_number(&self.version) > build_number(version)
    }

    fn parse(bytes: &[u8], current: &str, platform: &str, arch: &str) -> Result<Option<Self>> {
        let metadata: serde_json::Value = serde_json::from_slice(bytes)?;
        if metadata["schema"].as_u64() != Some(1) {
            return Err("Unsupported beta update feed".into());
        }
        let version = metadata["version"]
            .as_str()
            .ok_or("Missing update version")?;
        let next = build_number(version).ok_or("Invalid beta update version")?;
        let current = build_number(current).ok_or("This is not a released beta")?;
        if next <= current {
            return Ok(None);
        }
        let asset = &metadata["platforms"][platform];
        let url = asset["url"].as_str().ok_or("No update for this platform")?;
        if url != format!("{RELEASES}/v{version}/Muxy-{version}-{arch}.dmg") {
            return Err("Unexpected beta download location".into());
        }
        let size = asset["size"].as_u64().ok_or("Missing update size")?;
        if size == 0 || size > MAX_DOWNLOAD {
            return Err("Invalid beta download size".into());
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
        .get(FEED)
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()?
        .error_for_status()?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err("Beta update feed is too large".into());
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
        return Err("The beta download is incomplete or has an unexpected size".into());
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
        Release::parse(
            feed.to_string().as_bytes(),
            "2.0.0-beta-9",
            "macos-aarch64",
            "arm64",
        )
    }

    #[test]
    fn updates_only_to_a_newer_beta_using_numeric_order() {
        assert!(parse(&feed("2.0.0-beta-10")).is_ok_and(|release| release.is_some()));
        for version in ["2.0.0-beta-8", "2.0.0-beta-9"] {
            assert!(parse(&feed(version)).is_ok_and(|release| release.is_none()));
        }
        for version in [
            "2.0.0",
            "2.0.0-alpha-99",
            "1.0.0-beta-99",
            "2.0.0-beta-0",
            "2.0.0-beta-01",
            "2.0.0-beta-+10",
            "2.0.0-beta-999999999999999999999",
        ] {
            assert!(parse(&feed(version)).is_err(), "{version}");
        }
    }

    #[test]
    fn rejects_wrong_schema_platform_location_and_size() {
        let mut data = feed("2.0.0-beta-10");
        data["schema"] = 2.into();
        assert!(parse(&data).is_err());
        for url in [
            "http://github.com/file.dmg",
            "https://example.com/file.dmg",
            "https://github.com/muxy-app/muxy/releases/download/v2.0.0-beta-9/Muxy-2.0.0-beta-9-arm64.dmg",
        ] {
            let mut data = feed("2.0.0-beta-10");
            data["platforms"]["macos-aarch64"]["url"] = url.into();
            assert!(parse(&data).is_err());
        }
        for size in [0, MAX_DOWNLOAD + 1] {
            let mut data = feed("2.0.0-beta-10");
            data["platforms"]["macos-aarch64"]["size"] = size.into();
            assert!(parse(&data).is_err());
        }
        assert!(
            Release::parse(
                feed("2.0.0-beta-10").to_string().as_bytes(),
                "2.0.0-beta-9",
                "windows-x86_64",
                "x86_64"
            )
            .is_err()
        );
    }
}
