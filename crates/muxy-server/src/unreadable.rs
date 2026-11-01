//! The one policy for a saved file the server can't read: keep it beside the
//! original under a new name, start with defaults, and tell local apps.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{MAX_UNREADABLE_ERROR, UnreadableFile};

/// Renames `path` to `<name>.unreadable-<milliseconds>` so the next start
/// doesn't read it again and nothing overwrites it.
pub(crate) fn set_aside(path: &Path, error: &io::Error) -> io::Result<UnreadableFile> {
    let kept_as = kept_path(path);
    fs::rename(path, &kept_as).map_err(|rename| {
        io::Error::new(
            rename.kind(),
            format!(
                "{} is unreadable ({error}) and could not be moved aside: {rename}",
                path.display()
            ),
        )
    })?;
    log::error!(
        "{} is unreadable ({error}); kept as {} and started with defaults",
        path.display(),
        kept_as.display()
    );
    Ok(UnreadableFile {
        kept_as: kept_as.into(),
        error: truncated(error.to_string()),
    })
}

/// A name no earlier kept copy uses, so none is overwritten.
fn kept_path(path: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut base = path.file_name().unwrap_or_default().to_owned();
    base.push(format!(".unreadable-{millis}"));
    let mut kept = path.with_file_name(&base);
    let mut copy = 0_u64;
    while kept.exists() {
        copy += 1;
        let mut name = base.clone();
        name.push(format!("-{copy}"));
        kept = path.with_file_name(name);
    }
    kept
}

fn truncated(mut error: String) -> String {
    if error.len() > MAX_UNREADABLE_ERROR {
        let mut end = MAX_UNREADABLE_ERROR;
        while !error.is_char_boundary(end) {
            end -= 1;
        }
        error.truncate(end);
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_kept_under_a_new_name_with_its_contents() -> io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-unreadable-{}",
            muxy_protocol::OperationId::new()
        ));
        fs::create_dir(&directory)?;
        let path = directory.join("catalog.json");
        fs::write(&path, "damaged")?;
        let file = set_aside(&path, &io::Error::other("é".repeat(MAX_UNREADABLE_ERROR)))?;
        assert!(!path.exists());
        let kept = PathBuf::from(file.kept_as);
        assert_eq!(kept.parent(), Some(directory.as_path()));
        assert!(
            kept.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("catalog.json.unreadable-"))
        );
        assert_eq!(fs::read_to_string(&kept)?, "damaged");
        assert!(file.error.len() <= MAX_UNREADABLE_ERROR);
        for _ in 0..3 {
            fs::write(&path, "damaged again")?;
            set_aside(&path, &io::Error::other("unreadable"))?;
        }
        assert_eq!(
            fs::read_dir(&directory)?.count(),
            4,
            "no kept copy is replaced"
        );
        assert_eq!(fs::read_to_string(&kept)?, "damaged");
        fs::remove_dir_all(directory)
    }
}
