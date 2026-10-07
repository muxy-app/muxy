//! Folders on another computer, which its server lists anywhere there.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use muxy_protocol::{FilesAction, FilesReply, FilesRequest, ProjectId, ServerPath};
use muxy_ui::tr;

use super::path_service::{DirectoryItem, TypedPathState, standardize};

type List = dyn Fn(&str) -> Result<Vec<DirectoryItem>, String> + Send + Sync;
type Check = dyn Fn(&str, u64) -> Result<TypedPathState, String> + Send + Sync;
type Create = dyn Fn(&str, u64) -> Result<(), String> + Send + Sync;

#[derive(Clone)]
pub(crate) struct RemoteFolders {
    /// The computer's name, for messages.
    pub(crate) name: String,
    /// Its Home folder, which `~` means.
    pub(crate) home: String,
    list: Arc<List>,
    check: Arc<Check>,
    create: Arc<Create>,
}

impl RemoteFolders {
    pub(crate) fn new(
        name: String,
        home: &str,
        list: impl Fn(&str) -> Result<Vec<DirectoryItem>, String> + Send + Sync + 'static,
        check: impl Fn(&str, u64) -> Result<TypedPathState, String> + Send + Sync + 'static,
        create: impl Fn(&str, u64) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            home: standardize(home),
            list: Arc::new(list),
            check: Arc::new(check),
            create: Arc::new(create),
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
        let checking = client.clone();
        let creating = client.clone();
        Self::new(
            name,
            home,
            move |path| {
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
            },
            move |path, job| {
                let result = run(&checking, project, job, vec![
                "/bin/sh".into(), "-c".into(),
                "if [ -d \"$1\" ]; then printf directory; elif [ -e \"$1\" ] || [ -L \"$1\" ]; then printf file; else printf missing; fi".into(),
                "muxy-check-folder".into(), path.into(),
            ])?;
                match result.as_str() {
                    "directory" => Ok(TypedPathState::Directory),
                    "file" => Ok(TypedPathState::NotDirectory),
                    "missing" => Ok(TypedPathState::Missing),
                    _ => Err(tr!("The server sent an unexpected folder status").to_string()),
                }
            },
            move |path, job| {
                run(
                    &creating,
                    project,
                    job,
                    vec!["/bin/mkdir".into(), "-p".into(), path.into()],
                )
                .map(drop)
            },
        )
    }

    /// Folders in `path`, an absolute path on that computer.
    pub(crate) fn list(&self, path: &str) -> Result<Vec<DirectoryItem>, String> {
        (self.list)(path)
    }

    pub(crate) fn check(&self, path: &str, job: u64) -> Result<TypedPathState, String> {
        validate(path)?;
        (self.check)(path, job)
    }

    pub(crate) fn create(&self, path: &str, job: u64) -> Result<(), String> {
        validate(path)?;
        (self.create)(path, job)
    }
}

fn validate(path: &str) -> Result<(), String> {
    muxy_protocol::validate_folder_path(&ServerPath(path.as_bytes().into())).map_err(|_| {
        tr!("Use an absolute folder path of at most 4096 bytes, without NUL characters.")
            .to_string()
    })
}

fn run(
    client: &muxy_client::Client,
    project: ProjectId,
    job: u64,
    argv: Vec<String>,
) -> Result<String, String> {
    let result = client
        .exec(muxy_protocol::ExecRequest {
            job,
            project,
            argv,
            shell: None,
            cwd: None,
            env: std::collections::BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 10_000,
        })
        .map_err(|error| error.to_string())?;
    if result.timed_out {
        return Err(tr!("The server took too long to check or create the folder.").to_string());
    }
    if result.cancelled {
        return Err(tr!("The folder operation was cancelled.").to_string());
    }
    if result.exit_code != 0 || result.truncated {
        let message = result.stderr.trim();
        return Err(if message.is_empty() {
            tr!("The server could not check or create the folder.").to_string()
        } else {
            message.into()
        });
    }
    Ok(result.stdout)
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
        tr!(
            "%@ runs an older Muxy that lists only folders inside %@. Update it there to browse others, or type a full path.",
            name,
            root
        )
        .to_string()
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
        Ok(_) => Err(tr!("The server sent an unexpected reply").to_string()),
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
