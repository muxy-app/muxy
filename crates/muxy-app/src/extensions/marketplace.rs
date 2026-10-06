use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use muxy_app_core::extensions::Extension;
use muxy_ui::tr;
use serde_json::Value;

mod version;
pub(crate) use version::is_update;

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
        return Err(tr!("marketplace response is too large").into());
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(tr!("marketplace response is too large").into());
    }
    Ok(bytes)
}

/// One page of published extensions, optionally limited to a marketplace
/// category such as `localization`.
pub(crate) fn list(search: &str, page: u32, category: Option<&str>) -> Result<Value> {
    let page = page.to_string();
    let mut query = vec![
        ("search", search),
        ("sort", "all"),
        ("per_page", "24"),
        ("page", &page),
    ];
    query.extend(category.map(|category| ("category", category)));
    let response = client()?
        .get(format!("{BASE}/api/extensions"))
        .query(&query)
        .send()
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes(response, 2 * 1024 * 1024)?).map_err(|error| error.to_string())
}

pub(crate) fn detail(name: &str) -> Result<Value> {
    let mut url = reqwest::Url::parse(&format!("{BASE}/api/extensions/"))
        .map_err(|error| error.to_string())?;
    url.path_segments_mut()
        .map_err(|()| tr!("invalid marketplace URL").to_string())?
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
        .ok_or_else(|| tr!("invalid marketplace details").into())
}

pub(crate) fn versions(names: &[String]) -> Result<BTreeMap<String, String>> {
    let client = client()?;
    versions_with(names, |batch| {
        let response = client
            .post(format!("{BASE}/api/extensions/versions"))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .body(serde_json::json!({"names":batch}).to_string())
            .send()
            .map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes(response, 2 * 1024 * 1024)?)
            .map_err(|error| error.to_string())
    })
}

fn versions_with(
    names: &[String],
    mut request: impl FnMut(&[String]) -> Result<BTreeMap<String, Option<String>>>,
) -> Result<BTreeMap<String, String>> {
    let mut versions = BTreeMap::new();
    for batch in names.chunks(100) {
        versions.extend(
            request(batch)?
                .into_iter()
                .filter_map(|(name, version)| version.map(|version| (name, version))),
        );
    }
    Ok(versions)
}

/// Download and validate into a sibling staging directory; callers decide when to activate it.
pub(crate) fn download(details: &Value, directory: &Path) -> Result<tempfile::TempDir> {
    let url = reqwest::Url::parse(
        details["download_url"]
            .as_str()
            .ok_or_else(|| tr!("missing archive URL").to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("muxy.app")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(tr!("untrusted marketplace download URL").into());
    }
    let size = details["size"]
        .as_u64()
        .filter(|size| *size > 0 && *size <= MAX_ARCHIVE)
        .ok_or_else(|| tr!("invalid archive size").to_string())?;
    let expected = details["sha256"]
        .as_str()
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| tr!("invalid archive checksum").to_string())?;
    let data = bytes(
        client()?
            .get(url)
            .send()
            .map_err(|error| error.to_string())?,
        size,
    )?;
    if data.len() as u64 != size {
        return Err(tr!("archive size does not match the marketplace").into());
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
        return Err(tr!("archive checksum does not match the marketplace").into());
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
            return Err(tr!("archive must contain one extension package").into());
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
        return Err(tr!("archive identity does not match the marketplace").into());
    }
    let permissions: std::collections::BTreeSet<String> =
        serde_json::from_value(details["permissions"].clone())
            .map_err(|error| error.to_string())?;
    if extension.manifest.permissions != permissions {
        return Err(tr!("archive permissions do not match the marketplace").into());
    }
    Ok(stage)
}

fn extract(data: &[u8], directory: &Path) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;
    if archive.len() > 40000 {
        return Err(tr!("too many files in extension archive").into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.name().starts_with('/')
            || entry.name().contains('\\')
            || entry.name().contains(':')
        {
            return Err(tr!("unsafe path in extension archive").into());
        }
        if entry.is_symlink() {
            return Err(tr!("extension archives cannot contain symbolic links").into());
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| tr!("unsafe path in extension archive").to_string())?;
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(tr!("unsafe path in extension archive").into());
        }
        let path = directory.join(relative);
        total = total
            .checked_add(entry.size())
            .filter(|total| *total <= MAX_EXPANDED)
            .ok_or_else(|| tr!("extension archive expands beyond its limit").to_string())?;
        if entry.is_dir() {
            std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
            continue;
        }
        let parent = path
            .parent()
            .ok_or_else(|| tr!("invalid archive path").to_string())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let size = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)
            .map_err(|error| error.to_string())?;
        if copied != size {
            return Err(tr!("archive entry size mismatch").into());
        }
        file.flush().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn install(stage: &tempfile::TempDir, directory: &Path, name: &str) -> Result<()> {
    let package = stage.path().join("package");
    let extension = Extension::load(&package)?;
    if extension.name != name {
        return Err(tr!("extension identity changed during installation").into());
    }
    let destination = directory.join(name);
    if destination.exists() {
        return Err(tr!(
            "extension is already installed; uninstall it before installing this version"
        )
        .into());
    }
    std::fs::rename(package, destination).map_err(|error| error.to_string())
}

pub(crate) fn update(
    stage: &tempfile::TempDir,
    directory: &Path,
    expected: &Extension,
) -> Result<()> {
    let package = stage.path().join("package");
    let next = Extension::load(&package)?;
    let destination = directory.join(&expected.name);
    let current = Extension::load(&destination)?;
    if next.name != expected.name
        || current.name != expected.name
        || current.version != expected.version
        || !is_update(&current.version, &next.version)
    {
        return Err(tr!("The extension changed. Check for updates and try again.").into());
    }
    let log = current.directory.join("logs/output.log");
    if log.is_file() {
        let logs = next.directory.join("logs");
        std::fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
        std::fs::copy(log, logs.join("output.log")).map_err(|error| error.to_string())?;
    }
    let backup = tempfile::Builder::new()
        .prefix(".update-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    replace(&destination, &package, backup)
}

fn replace(current: &Path, next: &Path, backup: tempfile::TempDir) -> Result<()> {
    replace_with(current, next, backup, |from, to| std::fs::rename(from, to))
}

fn replace_with(
    current: &Path,
    next: &Path,
    backup: tempfile::TempDir,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let previous = backup.path().join("package");
    rename(current, &previous).map_err(|error| error.to_string())?;
    if let Err(error) = rename(next, current) {
        if let Err(rollback) = rename(&previous, current) {
            let saved = backup.keep().join("package");
            return Err(tr!(
                "Could not update the extension (%@) or restore it (%@). The previous version is saved at %@",
                error.to_string(),
                rollback.to_string(),
                saved.display().to_string()
            ).into());
        }
        return Err(tr!(
            "Could not update the extension; the previous version was restored: %@",
            error.to_string()
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    #[test]
    fn version_checks_batch_installed_names_and_skip_unpublished_extensions() {
        let names: Vec<_> = (0..205).map(|index| format!("extension-{index}")).collect();
        let mut batches = Vec::new();
        let versions = versions_with(&names, |batch| {
            batches.push(batch.to_vec());
            Ok(batch
                .iter()
                .map(|name| {
                    (
                        name.clone(),
                        (name != "extension-5").then(|| "1.1.0".into()),
                    )
                })
                .collect())
        })
        .unwrap();
        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            [100, 100, 5]
        );
        assert_eq!(batches.concat(), names);
        assert_eq!(versions.len(), 204);
        assert!(!versions.contains_key("extension-5"));
        assert!(
            versions_with(&[], |_| panic!("empty list must not make a request"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            versions_with(&names, |_| Err("Offline".into())),
            Err("Offline".into())
        );
    }

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

    fn package(root: &Path, name: &str, version: &str) -> Extension {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("package.json"),
            serde_json::json!({
                "name": name, "version": version, "muxy": {}
            })
            .to_string(),
        )
        .unwrap();
        Extension::load(root).unwrap()
    }

    #[test]
    fn updates_replace_code_and_preserve_logs_across_resource_layout_changes() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader/dist"), "reader", "1.0.0");
        std::fs::write(installed.directory.join("old.js"), "old code").unwrap();
        std::fs::create_dir(installed.directory.join("logs")).unwrap();
        std::fs::write(
            installed.directory.join("logs/output.log"),
            "earlier output",
        )
        .unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        let next = package(&stage.path().join("package"), "reader", "1.1.0");
        std::fs::write(next.directory.join("new.js"), "new code").unwrap();

        update(&stage, root.path(), &installed).unwrap();

        let installed = Extension::load(&root.path().join("reader")).unwrap();
        assert_eq!(installed.version, "1.1.0");
        assert_eq!(
            std::fs::read_to_string(installed.directory.join("new.js")).unwrap(),
            "new code"
        );
        assert_eq!(
            std::fs::read_to_string(installed.directory.join("logs/output.log")).unwrap(),
            "earlier output"
        );
        assert!(!installed.directory.join("dist").exists());
    }

    #[test]
    fn updates_reject_wrong_identity_older_versions_and_stale_installs() {
        let root = tempfile::tempdir().unwrap();
        let installed = package(&root.path().join("reader"), "reader", "1.1.0");
        for (name, version) in [("other", "2.0.0"), ("reader", "1.0.0"), ("reader", "1.1.0")] {
            let stage = tempfile::tempdir_in(root.path()).unwrap();
            package(&stage.path().join("package"), name, version);
            assert!(update(&stage, root.path(), &installed).is_err());
            assert_eq!(
                Extension::load(&root.path().join("reader"))
                    .unwrap()
                    .version,
                "1.1.0"
            );
        }
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        package(&stage.path().join("package"), "reader", "2.0.0");
        package(&root.path().join("reader"), "reader", "1.2.0");
        assert!(update(&stage, root.path(), &installed).is_err());
        assert_eq!(
            Extension::load(&root.path().join("reader"))
                .unwrap()
                .version,
            "1.2.0"
        );
    }

    #[test]
    fn failed_replacement_restores_the_installed_extension() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("reader");
        package(&current, "reader", "1.0.0");
        let backup = tempfile::tempdir_in(root.path()).unwrap();
        assert!(replace(&current, &root.path().join("missing"), backup).is_err());
        assert_eq!(Extension::load(&current).unwrap().version, "1.0.0");
    }

    #[test]
    fn failed_rollback_keeps_the_backup_and_reports_its_location() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("reader");
        package(&current, "reader", "1.0.0");
        let backup = tempfile::tempdir_in(root.path()).unwrap();
        let saved = backup.path().join("package");
        let mut calls = 0;
        let result = replace_with(
            &current,
            &root.path().join("missing"),
            backup,
            |from, to| {
                calls += 1;
                if calls == 1 {
                    std::fs::rename(from, to)
                } else {
                    Err(std::io::Error::other("test failure"))
                }
            },
        );
        assert!(result.unwrap_err().contains(&saved.display().to_string()));
        assert_eq!(Extension::load(&saved).unwrap().version, "1.0.0");
    }
}
