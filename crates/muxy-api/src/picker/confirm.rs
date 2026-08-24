use super::path_service::{DirectoryState, directory_state, standardize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmResult {
    Success,
    MissingDirectory,
    NotDirectory,
    CreateFailed,
}

impl ConfirmResult {
    pub fn title(self) -> &'static str {
        match self {
            Self::NotDirectory => "Path Is Not a Folder",
            Self::CreateFailed => "Could Not Create Project Folder",
            _ => "Could Not Add Project",
        }
    }

    pub fn message(self, path: &str) -> String {
        match self {
            Self::NotDirectory => {
                "Muxy can only add folders as projects. Choose a folder or type a new folder path."
                    .to_owned()
            }
            Self::MissingDirectory => {
                format!("Muxy couldn't find \"{path}\". Check the path and try again.")
            }
            Self::CreateFailed => format!(
                "Muxy couldn't create and add \"{path}\". Check that you have permission to use this location."
            ),
            Self::Success => String::new(),
        }
    }
}

pub fn ensure_directory(path: &str, create_if_missing: bool) -> ConfirmResult {
    let standardized = standardize(path);
    match directory_state(&standardized) {
        DirectoryState::Directory => ConfirmResult::Success,
        DirectoryState::NotDirectory => ConfirmResult::NotDirectory,
        DirectoryState::Missing => {
            if !create_if_missing {
                return ConfirmResult::MissingDirectory;
            }
            match std::fs::create_dir_all(&standardized) {
                Ok(()) => ConfirmResult::Success,
                Err(_) => ConfirmResult::CreateFailed,
            }
        }
    }
}
