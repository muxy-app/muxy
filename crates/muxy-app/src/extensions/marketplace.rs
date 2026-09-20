use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use muxy_app_core::extensions::Extension;
use serde_json::Value;

const BASE: &str = "https://muxy.app";
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_EXPANDED: u64 = 512 * 1024 * 1024;

type Result<T> = std::result::Result<T, String>;

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Muxy/2 Extensions")
        .build()
        .map_err(|error| error.to_string())
}

fn bytes(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>> {
    let response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    if response.content_length().is_some_and(|size| size > limit) {
        return Err("marketplace response is too large".into());
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("marketplace response is too large".into());
    }
    Ok(bytes)
}

pub(crate) fn list(search: &str, page: u32) -> Result<Value> {
    let response = client()?
        .get(format!("{BASE}/api/extensions"))
        .query(&[
            ("search", search),
            ("sort", "all"),
            ("per_page", "24"),
            ("page", &page.to_string()),
        ])
        .send()
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes(response, 2 * 1024 * 1024)?).map_err(|error| error.to_string())
}

pub(crate) fn detail(name: &str) -> Result<Value> {
    let mut url = reqwest::Url::parse(&format!("{BASE}/api/extensions/"))
        .map_err(|error| error.to_string())?;
    url.path_segments_mut()
        .map_err(|()| "invalid marketplace URL")?
        .pop_if_empty()
        .push(name);
    let response = client()?
        .get(url)
        .send()
        .map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_slice(&bytes(response, 2 * 1024 * 1024)?)
        .map_err(|error| error.to_string())?;
    value
        .get("data")
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| "invalid marketplace details".into())
}

/// Download and validate into a sibling staging directory; callers decide when to activate it.
pub(crate) fn download(details: &Value, directory: &Path) -> Result<tempfile::TempDir> {
    let url = reqwest::Url::parse(
        details["download_url"]
            .as_str()
            .ok_or("missing archive URL")?,
    )
    .map_err(|error| error.to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("muxy.app")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("untrusted marketplace download URL".into());
    }
    let size = details["size"]
        .as_u64()
        .filter(|size| *size > 0 && *size <= MAX_ARCHIVE)
        .ok_or("invalid archive size")?;
    let expected = details["sha256"]
        .as_str()
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("invalid archive checksum")?;
    let data = bytes(
        client()?
            .get(url)
            .send()
            .map_err(|error| error.to_string())?,
        size,
    )?;
    if data.len() as u64 != size {
        return Err("archive size does not match the marketplace".into());
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    let archive = stage.path().join("download.zip");
    std::fs::write(&archive, &data).map_err(|error| error.to_string())?;
    let hash = std::process::Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(&archive)
        .output()
        .map_err(|error| error.to_string())?;
    if !hash.status.success()
        || String::from_utf8_lossy(&hash.stdout)
            .split_whitespace()
            .next()
            .is_none_or(|hash| !hash.eq_ignore_ascii_case(expected))
    {
        return Err("archive checksum does not match the marketplace".into());
    }
    let extracted = stage.path().join("package");
    extract(&data, &extracted)?;
    if !extracted.join("package.json").is_file() {
        let roots: Vec<_> = std::fs::read_dir(&extracted)
            .map_err(|e| e.to_string())?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join("package.json").is_file())
            .collect();
        if roots.len() != 1 {
            return Err("archive must contain one extension package".into());
        }
        let nested = stage.path().join("nested");
        std::fs::rename(&roots[0], &nested).map_err(|e| e.to_string())?;
        std::fs::remove_dir_all(&extracted).map_err(|e| e.to_string())?;
        std::fs::rename(nested, &extracted).map_err(|e| e.to_string())?;
    }
    let extension = Extension::load(&extracted)?;
    if Some(extension.name.as_str()) != details["name"].as_str()
        || Some(extension.version.as_str()) != details["current_version"].as_str()
    {
        return Err("archive identity does not match the marketplace".into());
    }
    let permissions: std::collections::BTreeSet<String> =
        serde_json::from_value(details["permissions"].clone())
            .map_err(|error| error.to_string())?;
    if extension.manifest.permissions != permissions {
        return Err("archive permissions do not match the marketplace".into());
    }
    Ok(stage)
}

fn extract(data: &[u8], directory: &Path) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;
    if archive.len() > 40000 {
        return Err("too many files in extension archive".into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.name().starts_with('/')
            || entry.name().contains('\\')
            || entry.name().contains(':')
        {
            return Err("unsafe path in extension archive".into());
        }
        if entry.is_symlink() {
            return Err("extension archives cannot contain symbolic links".into());
        }
        let relative = entry
            .enclosed_name()
            .ok_or("unsafe path in extension archive")?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("unsafe path in extension archive".into());
        }
        let path = directory.join(relative);
        total = total
            .checked_add(entry.size())
            .filter(|total| *total <= MAX_EXPANDED)
            .ok_or("extension archive expands beyond its limit")?;
        if entry.is_dir() {
            std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
            continue;
        }
        std::fs::create_dir_all(path.parent().ok_or("invalid archive path")?)
            .map_err(|error| error.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let size = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)
            .map_err(|error| error.to_string())?;
        if copied != size {
            return Err("archive entry size mismatch".into());
        }
        file.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn install(stage: &tempfile::TempDir, directory: &Path, name: &str) -> Result<()> {
    let package = stage.path().join("package");
    let extension = Extension::load(&package)?;
    if extension.name != name {
        return Err("extension identity changed during installation".into());
    }
    let destination = directory.join(name);
    if destination.exists() {
        return Err(
            "extension is already installed; uninstall it before installing this version".into(),
        );
    }
    std::fs::rename(package, destination).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    fn archive(path: &str, mode: Option<u32>) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        if mode == Some(0o120_777) {
            writer
                .add_symlink(
                    path,
                    "/tmp/outside",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
        } else {
            writer
                .start_file(path, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"hello").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn archives_cannot_escape_staging_or_install_symlinks() {
        let root = tempfile::tempdir().unwrap();
        assert!(extract(&archive("../outside", None), root.path()).is_err());
        assert!(extract(&archive("/tmp/outside", None), root.path()).is_err());
        assert!(extract(&archive("link", Some(0o120_777)), root.path()).is_err());
        extract(&archive("panel/index.html", None), root.path()).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("panel/index.html")).unwrap(),
            b"hello"
        );
    }
}
