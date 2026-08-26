use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temp = stage(path, contents, false)?;
    publish_replacing(&temp, path)
}

#[cfg(unix)]
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = stage(path, contents, true)?;
    if let Err(error) = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600)) {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    publish_replacing(&temp, path)
}

#[cfg(not(unix))]
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_atomic(path, contents)
}

fn stage(path: &Path, contents: &[u8], private: bool) -> std::io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("muxy"))
            .to_os_string();
        name.push(format!(".{}.{}.tmp", std::process::id(), sequence));
        let temp = parent.join(name);
        let opened = open_staging_file(&temp, private);
        let mut file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = file.write_all(contents).and_then(|()| file.sync_all());
        if let Err(error) = result {
            drop(file);
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
        return Ok(temp);
    }
}

#[cfg(unix)]
fn open_staging_file(path: &Path, private: bool) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(if private { 0o600 } else { 0o666 })
        .open(path)
}

#[cfg(not(unix))]
fn open_staging_file(path: &Path, _private: bool) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

fn publish_replacing(temp: &Path, path: &Path) -> std::io::Result<()> {
    match std::fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(temp);
            Err(error)
        }
    }
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let contents = serde_json::to_vec(value)?;
    write_atomic(path, &contents)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn private_staging_is_private_from_creation() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        let staged = super::stage(&path, b"{}", true).expect("stage");
        assert_eq!(mode(&staged), 0o600);
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

    #[test]
    fn write_private_does_not_follow_a_predictable_temporary_symlink() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        let external = dir.path().join("external.json");
        std::fs::write(&external, b"external").expect("external");
        symlink(&external, dir.path().join("private.json.tmp")).expect("symlink");

        super::write_private(&path, b"private").expect("write");
        assert_eq!(std::fs::read(&external).expect("external"), b"external");
        assert_eq!(std::fs::read(&path).expect("private"), b"private");
    }
}
