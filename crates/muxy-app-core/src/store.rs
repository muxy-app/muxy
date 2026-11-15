use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use muxy_core::files::{FileId, private_file, replace_file, sync_directory};

use crate::{AppError, AppState};

pub fn default_path() -> Result<PathBuf, AppError> {
    muxy_core::dirs::muxy_dir()
        .map(|directory| directory.join("desktop-state.json"))
        .map_err(|source| AppError::Io {
            path: PathBuf::from("Muxy state directory"),
            source,
        })
}

pub fn load(path: impl AsRef<Path>) -> Result<AppState, AppError> {
    let path = path.as_ref();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if path
                .file_name()
                .is_some_and(|name| name == "desktop-state.json")
                && path
                    .with_file_name("state.json")
                    .try_exists()
                    .map_err(|source| AppError::Io {
                        path: path.into(),
                        source,
                    })?
            {
                return load(path.with_file_name("state.json"));
            }
            return AppState::bootstrap();
        }
        Err(source) => {
            return Err(AppError::Io {
                path: path.into(),
                source,
            });
        }
    };
    serde_json::from_slice(&bytes).map_err(|source| AppError::Json {
        path: path.into(),
        source,
    })
}

pub fn save(path: impl AsRef<Path>, state: &AppState) -> Result<(), AppError> {
    let path = path.as_ref();
    let mut bytes = serde_json::to_vec_pretty(state).map_err(|source| AppError::Json {
        path: path.into(),
        source,
    })?;
    bytes.push(b'\n');
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut file = open_temporary(&temporary).map_err(|source| AppError::Io {
        path: temporary.clone(),
        source,
    })?;
    let result = file
        .set_len(0)
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .and_then(|()| replace_file(&temporary, path))
        .and_then(|()| sync_directory(path.parent().unwrap_or_else(|| Path::new("."))));
    if let Err(source) = result {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::Io {
            path: path.into(),
            source,
        });
    }
    Ok(())
}

fn open_temporary(path: &Path) -> io::Result<File> {
    let file = private_file()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock()?;
    if FileId::of(&file)? != FileId::of_path(path)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "temporary state file was replaced by another writer",
        ));
    }
    Ok(file)
}
