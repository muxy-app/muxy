use super::root::{Root, Target};
use super::{Result, error};

#[cfg(any(target_os = "linux", test))]
mod freedesktop;

#[cfg(target_os = "linux")]
pub(super) fn move_to_trash(root: &Root, source: &Target) -> Result<()> {
    freedesktop::move_to_trash(root, source)
}

#[cfg(target_os = "macos")]
pub(super) fn move_to_trash(root: &Root, source: &Target) -> Result<()> {
    macos_trash(root, source).map(|_| ())
}

#[cfg(target_os = "macos")]
fn macos_trash(root: &Root, source: &Target) -> Result<std::path::PathBuf> {
    use objc2_foundation::{NSData, NSFileManager, NSURL};
    use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

    let expected = source.stat()?;
    let absolute = root.path.join(&source.relative);
    let mut bytes = b"file://".to_vec();
    bytes.extend_from_slice(percent_encode(absolute.as_os_str().as_bytes()).as_bytes());
    let url = NSURL::URLWithDataRepresentation_relativeToURL(&NSData::with_bytes(&bytes), None);
    let reference = url
        .fileReferenceURL()
        .ok_or_else(|| error("Cannot create a file reference for Trash"))?;
    let resolved = reference
        .filePathURL()
        .and_then(|url| url.path())
        .ok_or_else(|| error("Cannot resolve the Trash file reference"))?;
    let metadata = std::fs::symlink_metadata(resolved.to_string()).map_err(error)?;
    if metadata.ino() != expected.st_ino
        || metadata.dev() != u64::try_from(expected.st_dev).map_err(error)?
    {
        return Err(error("Path changed before moving to Trash; retry"));
    }
    let mut result = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&reference, Some(&mut result))
        .map_err(error)?;
    result
        .and_then(|url| url.path())
        .map(|path| std::path::PathBuf::from(path.to_string()))
        .ok_or_else(|| error("File was moved to Trash but its new location is unavailable"))
}

fn percent_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in bytes {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            encoded.push(char::from(*byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Moves a temporary file through the native macOS Trash and restores it"]
    fn native_trash_preserves_file_content() -> Result<()> {
        let workspace = crate::files::tests::Workspace::new();
        let path = workspace.path.join(format!(
            "muxy-trash-test-{}",
            muxy_protocol::OperationId::new()
        ));
        std::fs::write(&path, "recoverable").map_err(error)?;
        let root = Root::open(&workspace.path)?;
        let source = root.mutation(std::path::Path::new(
            path.file_name().ok_or_else(|| error("Missing name"))?,
        ))?;
        let trashed = macos_trash(&root, &source)?;
        let contents = std::fs::read_to_string(&trashed).map_err(error);
        std::fs::rename(&trashed, &path).map_err(error)?;
        assert_eq!(contents?, "recoverable");
        Ok(())
    }
}
