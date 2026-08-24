use std::io::Write;
use std::path::{Path, PathBuf};

pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temp = stage(path, contents)?;
    std::fs::rename(&temp, path)
}

#[cfg(unix)]
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = stage(path, contents)?;
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&temp, path)
}

#[cfg(not(unix))]
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_atomic(path, contents)
}

fn stage(path: &Path, contents: &[u8]) -> std::io::Result<PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
    ));
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    Ok(temp)
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let contents = serde_json::to_vec(value)?;
    write_atomic(path, &contents)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn write_private_restricts_permissions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        super::write_private(&path, b"{}").expect("write");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(std::fs::read(&path).expect("read"), b"{}");
    }

    #[test]
    fn write_private_downgrades_an_existing_readable_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        super::write_atomic(&path, b"old").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(mode(&path), 0o644);

        super::write_private(&path, b"new").expect("write");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(std::fs::read(&path).expect("read"), b"new");
    }
}
