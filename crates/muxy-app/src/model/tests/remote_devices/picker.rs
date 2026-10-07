use super::*;
use crate::picker::path_service::TypedPathState;

pub(super) fn check_folder_operations(
    folders: &RemoteFolders,
    directory: &std::path::Path,
) -> Result {
    let nested = directory.join("new 'quoted' $(literal)/nested project");
    let nested_path = nested.to_string_lossy();
    assert_eq!(folders.check(&nested_path, 91)?, TypedPathState::Missing);
    folders.create(&nested_path, 92)?;
    assert!(nested.is_dir());
    assert_eq!(folders.check(&nested_path, 93)?, TypedPathState::Directory);
    folders.create(&nested_path, 94)?;
    let file = directory.join("ordinary file");
    std::fs::write(&file, "keep")?;
    assert_eq!(
        folders.check(&file.to_string_lossy(), 95)?,
        TypedPathState::NotDirectory
    );
    assert!(folders.create(&file.to_string_lossy(), 96).is_err());
    assert_eq!(std::fs::read_to_string(&file)?, "keep");
    assert!(folders.create("relative/path", 97).is_err());
    Ok(())
}
