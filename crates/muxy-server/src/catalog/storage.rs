use std::fs;
use std::io::{self, Write};
use std::path::Path;

use muxy_core::files::{private_file, replace_file, sync_directory};

use super::State;

pub(super) fn save(path: &Path, state: &State, committed: &mut bool) -> io::Result<()> {
    let bytes = serde_json::to_vec(state)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("catalog has no parent"))?;
    let temporary = parent.join(format!(
        ".catalog-{}.tmp",
        muxy_protocol::OperationId::new()
    ));
    let result = (|| {
        let mut file = private_file()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)?;
        *committed = true;
        sync_parent(path)
    })();
    if !*committed {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn sync_parent(path: &Path) -> io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("catalog has no parent"))?;
    if !fs::metadata(directory)?.is_dir() {
        return Err(io::Error::other("catalog parent is not a directory"));
    }
    sync_directory(directory)
}
