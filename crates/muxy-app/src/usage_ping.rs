//! The once-a-day anonymous ping that counts active installs per version.
//! It sends the app version, operating system, and processor architecture,
//! and nothing that identifies the install or the person using it.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ENDPOINT: &str = "https://muxy.app/api/ping";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Whether `DO_NOT_TRACK` asks for no pings. Unset, empty, `0`, and `false` don't.
pub(crate) fn declined(do_not_track: Option<&str>) -> bool {
    do_not_track.is_some_and(|value| {
        let value = value.trim();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

/// Sends the ping unless one already went out on this UTC day, then saves the
/// day to `record`. Any answer from the server counts as sent; only a request
/// that never reached it is tried again.
pub(crate) fn send_if_due(record: &Path) -> Result<bool> {
    let today = utc_day(SystemTime::now());
    if !due(record, today) {
        return Ok(false);
    }
    send()?;
    std::fs::write(record, today.to_string())?;
    Ok(true)
}

fn due(record: &Path, today: u64) -> bool {
    std::fs::read_to_string(record)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        != Some(today)
}

fn send() -> Result<()> {
    crate::proxy::configure(
        reqwest::blocking::Client::builder()
            .user_agent(concat!("Muxy/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15)),
    )?
    .build()?
    .post(ENDPOINT)
    .header(reqwest::header::CONTENT_TYPE, "application/json")
    .body(body(
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    ))
    .send()?;
    Ok(())
}

fn body(version: &str, os: &str, arch: &str) -> String {
    serde_json::json!({"version": version, "os": os, "arch": arch}).to_string()
}

fn utc_day(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_holds_only_version_os_and_arch() {
        let body: serde_json::Value =
            serde_json::from_str(&body("2.4.0-beta.3", "macos", "aarch64")).expect("json");
        assert_eq!(
            body,
            serde_json::json!({"version": "2.4.0-beta.3", "os": "macos", "arch": "aarch64"})
        );
    }

    #[test]
    fn do_not_track_declines_unless_unset_or_off() {
        for value in [
            None,
            Some(""),
            Some(" "),
            Some("0"),
            Some("false"),
            Some("FALSE"),
        ] {
            assert!(!declined(value), "{value:?}");
        }
        for value in ["1", "true", "yes"] {
            assert!(declined(Some(value)), "{value}");
        }
    }

    #[test]
    fn days_change_at_utc_midnight() {
        let day = |seconds| utc_day(UNIX_EPOCH + Duration::from_secs(seconds));
        assert_eq!(day(0), 0);
        assert_eq!(day(86_399), 0);
        assert_eq!(day(86_400), 1);
        assert_eq!(utc_day(UNIX_EPOCH - Duration::from_secs(1)), 0);
    }

    #[test]
    fn is_due_once_per_recorded_day() {
        let directory = tempfile::tempdir().expect("directory");
        let record = directory.path().join("usage-ping");
        assert!(due(&record, 20_000));
        std::fs::write(&record, "20000\n").expect("record");
        assert!(!due(&record, 20_000));
        assert!(due(&record, 20_001));
        std::fs::write(&record, "garbage").expect("record");
        assert!(due(&record, 20_000));
    }
}
