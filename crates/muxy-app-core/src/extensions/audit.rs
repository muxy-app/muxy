//! The profile's `extension-audit.log`: one JSON line per gated decision, with
//! main's fields, trimmed to its newest 256 KB once it passes 1 MB.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub const AUDIT_LOG: &str = "extension-audit.log";
const MAX_SIZE: u64 = 1024 * 1024;
const TRIMMED_SIZE: u64 = 256 * 1024;

/// One gated decision: a saved rule applied, a prompt answer, a timeout, or a
/// cancel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub timestamp: String,
    #[serde(rename = "extensionID")]
    pub extension_id: String,
    pub verb: String,
    pub payload_summary: String,
    pub decision: String,
    #[serde(rename = "ruleID", skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(profile: &Path) -> Self {
        Self {
            path: profile.join(AUDIT_LOG),
        }
    }

    pub fn append(&self, entry: &AuditEntry) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(entry)?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)?;
        file.write_all(&line)?;
        if file.metadata()?.len() > MAX_SIZE {
            trim(&self.path)?;
        }
        Ok(())
    }
}

/// Keeps the newest 256 KB, starting at a line boundary.
fn trim(path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TRIMMED_SIZE)))?;
    let mut kept = Vec::new();
    file.read_to_end(&mut kept)?;
    let start = kept
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(kept.len(), |index| index + 1);
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?
        .write_all(&kept[start..])?;
    std::fs::rename(temporary, path)
}

/// `time` as an ISO 8601 UTC timestamp to the second, like main's log.
pub fn timestamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's days-to-civil conversion, in whole 400-year eras.
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    fn entry(summary: &str) -> AuditEntry {
        AuditEntry {
            timestamp: "2026-09-27T20:15:03Z".into(),
            extension_id: "files".into(),
            verb: "exec".into(),
            payload_summary: summary.into(),
            decision: "allow".into(),
            rule_id: None,
            source: "exec".into(),
        }
    }

    #[test]
    fn entries_use_main_field_names_and_omit_a_missing_rule() {
        let directory = tempfile::tempdir().unwrap();
        let log = AuditLog::new(directory.path());
        log.append(&entry("git status")).unwrap();
        log.append(&AuditEntry {
            rule_id: Some("exec:argv:git".into()),
            ..entry("git push")
        })
        .unwrap();
        let text = std::fs::read_to_string(directory.path().join(AUDIT_LOG)).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            lines[0],
            serde_json::json!({
                "timestamp": "2026-09-27T20:15:03Z",
                "extensionID": "files",
                "verb": "exec",
                "payloadSummary": "git status",
                "decision": "allow",
                "source": "exec",
            })
        );
        assert_eq!(lines[1]["ruleID"], "exec:argv:git");
        let mode = std::fs::metadata(directory.path().join(AUDIT_LOG))
            .unwrap()
            .permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );
    }

    #[test]
    fn a_full_log_keeps_its_newest_whole_lines() {
        let directory = tempfile::tempdir().unwrap();
        let log = AuditLog::new(directory.path());
        let summary = "x".repeat(900);
        for _ in 0..1_300 {
            log.append(&entry(&summary)).unwrap();
        }
        let text = std::fs::read_to_string(directory.path().join(AUDIT_LOG)).unwrap();
        assert!((text.len() as u64) < MAX_SIZE);
        assert!(
            text.lines()
                .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
}
