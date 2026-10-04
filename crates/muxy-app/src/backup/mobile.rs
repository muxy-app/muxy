use std::path::Path;

use muxy_app_core::backup::{ImportReport, Result};
use muxy_protocol::RemoteAccessSettings;
use serde_json::{Map, Value};

use super::{Files, archive};

pub(super) const FILE: &str = "mobile-settings.json";

pub(super) fn parse(source: &[u8]) -> Result<RemoteAccessSettings> {
    let value: Map<String, Value> = serde_json::from_slice(source)?;
    if value
        .keys()
        .any(|key| !matches!(key.as_str(), "enabled" | "port"))
    {
        return Err("Mobile settings may contain only enabled and port".into());
    }
    let settings: RemoteAccessSettings = serde_json::from_value(Value::Object(value))?;
    settings.validate().map_err(|_| "Invalid Mobile port")?;
    Ok(settings)
}

pub(super) fn current(profile: &Path) -> Result<RemoteAccessSettings> {
    if profile.join(FILE).try_exists()? {
        return parse(&archive::read_file(&profile.join(FILE))?);
    }
    let path = profile.join("remote.json");
    if !path.try_exists()? {
        return Ok(RemoteAccessSettings::default());
    }
    let value: Value = serde_json::from_slice(&archive::read_file(&path)?)?;
    parse(&serde_json::to_vec(&serde_json::json!({
        "enabled": value["enabled"], "port": value["port"]
    }))?)
}

pub(super) fn migrate(
    profile: &Path,
    source: &mut Map<String, Value>,
    files: &mut Files,
    report: &mut ImportReport,
) -> Result<()> {
    let mut settings = current(profile)?;
    let mut changed = false;
    for (key, field) in [
        ("app.muxy.mobile.serverEnabled", "enabled"),
        ("app.muxy.mobile.serverPort", "port"),
    ] {
        let Some(value) = source.remove(key) else {
            continue;
        };
        let mut candidate = serde_json::to_value(settings)?;
        candidate[field] = value;
        match parse(&serde_json::to_vec(&candidate)?) {
            Ok(candidate) => {
                settings = candidate;
                changed = true;
                report.imported += 1;
            }
            Err(_) => report.skipped.push(key.into()),
        }
    }
    if changed {
        files.insert(FILE.into(), serde_json::to_vec_pretty(&settings)?);
    }
    Ok(())
}

/// Apply through the server so local pairing credentials are preserved or generated.
pub(crate) fn apply_mobile_settings(
    profile: &Path,
    apply: impl FnOnce(RemoteAccessSettings) -> Result<()>,
) -> Result<()> {
    let path = profile.join(FILE);
    if path.try_exists()? {
        apply(parse(&archive::read_file(&path)?)?)?;
        std::fs::remove_file(path)?;
    }
    Ok(())
}
