//! Folders on another computer, which its server lists anywhere there.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use muxy_protocol::{FilesAction, FilesReply, FilesRequest, ProjectId, ServerPath};

use super::path_service::{DirectoryItem, standardize};

type List = dyn Fn(&str) -> Result<Vec<DirectoryItem>, String> + Send + Sync;

#[derive(Clone)]
pub(crate) struct RemoteFolders {
    /// The computer's name, for messages.
    pub(crate) name: String,
    /// Its Home folder, which `~` means.
    pub(crate) home: String,
    list: Arc<List>,
}

impl RemoteFolders {
    pub(crate) fn new(
        name: String,
        home: &str,
        list: impl Fn(&str) -> Result<Vec<DirectoryItem>, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            home: standardize(home),
            list: Arc::new(list),
        }
    }

    /// Lists any folder through the server. Servers from before folder
    /// listing can only list inside Home, through `Files` on its Home
    /// project; any absolute path can still be typed there.
    pub(crate) fn through(
        client: muxy_client::Client,
        name: String,
        project: ProjectId,
        home: &str,
    ) -> Self {
        let root = standardize(home);
        let older = AtomicBool::new(false);
        let name_for_errors = name.clone();
        Self::new(name, home, move |path| {
            if !older.load(Ordering::Relaxed) {
                match client.list_folders(ServerPath(path.as_bytes().to_vec())) {
                    Ok(names) => {
                        return Ok(names
                            .into_iter()
                            .map(|name| {
                                DirectoryItem::Directory(
                                    String::from_utf8_lossy(&name.0).into_owned(),
                                )
                            })
                            .collect());
                    }
                    Err(muxy_client::ClientError::Server(error))
                        if error.code == muxy_protocol::ErrorCode::Unsupported =>
                    {
                        older.store(true, Ordering::Relaxed);
                    }
                    Err(muxy_client::ClientError::Server(error)) => return Err(error.message),
                    Err(error) => return Err(error.to_string()),
                }
            }
            home_folders(&client, project, &root, &name_for_errors, path)
        })
    }

    /// Folders in `path`, an absolute path on that computer.
    pub(crate) fn list(&self, path: &str) -> Result<Vec<DirectoryItem>, String> {
        (self.list)(path)
    }
}

/// Folders in `path` through `Files` on the Home project, so only inside Home.
fn home_folders(
    client: &muxy_client::Client,
    project: ProjectId,
    root: &str,
    name: &str,
    path: &str,
) -> Result<Vec<DirectoryItem>, String> {
    let relative = inside(root, path).ok_or_else(|| {
        format!(
            "{name} runs an older Muxy that lists only folders inside {root}. Update it there to browse others, or type a full path."
        )
    })?;
    let request = FilesRequest {
        project,
        action: FilesAction::List(ServerPath(relative.as_bytes().to_vec())),
    };
    match client.files(request) {
        Ok(FilesReply::Entries(entries)) => Ok(entries
            .into_iter()
            .filter(|entry| entry.is_directory)
            .map(|entry| {
                DirectoryItem::Directory(String::from_utf8_lossy(&entry.name.0).into_owned())
            })
            .collect()),
        Ok(_) => Err("The server sent an unexpected reply".into()),
        Err(error) => Err(error.to_string()),
    }
}

/// `path` relative to `root`, or `None` outside it.
fn inside<'a>(root: &str, path: &'a str) -> Option<&'a str> {
    if path == root {
        return Some("");
    }
    if root == "/" {
        return path.strip_prefix('/');
    }
    path.strip_prefix(root)?.strip_prefix('/')
}

#[cfg(test)]
mod tests {
    use super::inside;

    #[test]
    fn paths_are_relative_to_home_and_nothing_outside_is_listed() {
        assert_eq!(inside("/home/dev", "/home/dev"), Some(""));
        assert_eq!(inside("/home/dev", "/home/dev/code/app"), Some("code/app"));
        assert_eq!(inside("/home/dev", "/home/devops"), None);
        assert_eq!(inside("/home/dev", "/srv"), None);
        assert_eq!(inside("/", "/srv/app"), Some("srv/app"));
    }
}
