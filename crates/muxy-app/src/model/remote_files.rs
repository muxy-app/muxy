//! Files on another computer, reached through its server's second
//! connection: what is dropped or pasted on its terminals is sent there, and
//! files their links name can be copied here.

use std::cmp::Reverse;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::Context;
use muxy_app_core::opener::{FileLocation, Target};
use muxy_app_core::{PaneId, ServerId};
use muxy_client::{Client, ClientError};
use muxy_protocol::{
    ErrorCode, FilesAction, FilesReply, FilesRequest, MAX_UPLOAD_BYTES, ProjectId, ServerPath,
    SessionId,
};

use super::AppModel;
use crate::views::menu::{Command, Item};

/// What a terminal on another computer gets in place of this computer's
/// files or clipboard, which programs there can't reach.
#[derive(Clone, Debug)]
pub(crate) enum Upload {
    Files(Vec<PathBuf>),
    /// Image bytes, and the extension of their format.
    Image(Vec<u8>, &'static str),
}

/// One server's files, through its second connection.
#[derive(Clone)]
pub(crate) struct RemoteFiles {
    client: Client,
    /// The computer's name, for messages.
    name: String,
    /// Its Home and project folders, longest first: file requests reach
    /// files inside them.
    roots: Vec<(ProjectId, PathBuf)>,
    home: PathBuf,
}

impl RemoteFiles {
    /// Sends files or an image for `session`, and returns their paths there.
    pub(crate) fn upload(
        &self,
        session: SessionId,
        upload: Upload,
    ) -> Result<Vec<PathBuf>, String> {
        match upload {
            Upload::Files(paths) => paths
                .iter()
                .map(|path| {
                    let name = display_name(path);
                    let bytes = self.read_local(path, &name)?;
                    self.send(session, &name, &bytes)
                })
                .collect(),
            Upload::Image(bytes, extension) => self
                .send(session, &format!("pasted-image.{extension}"), &bytes)
                .map(|path| vec![path]),
        }
    }

    fn read_local(&self, path: &Path, name: &str) -> Result<Vec<u8>, String> {
        let unreadable = |error: io::Error| format!("Could not read {name}: {error}");
        let metadata = std::fs::metadata(path).map_err(unreadable)?;
        if metadata.is_dir() {
            return Err(format!(
                "{name} is a folder. Only files can be sent to {}.",
                self.name
            ));
        }
        if metadata.len() > MAX_UPLOAD_BYTES {
            return Err(too_large(name, &self.name));
        }
        let bytes = std::fs::read(path).map_err(unreadable)?;
        if bytes.len() as u64 > MAX_UPLOAD_BYTES {
            return Err(too_large(name, &self.name));
        }
        Ok(bytes)
    }

    fn send(&self, session: SessionId, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        match self.client.upload(session, name, bytes) {
            Ok(path) => Ok(local_path(&path)),
            Err(ClientError::Server(error)) if error.code == ErrorCode::Unsupported => {
                Err(format!(
                    "Muxy on {} can't receive files. Update Muxy there.",
                    self.name
                ))
            }
            Err(ClientError::Server(error)) => Err(format!(
                "Could not send {name} to {}: {}",
                self.name, error.message
            )),
            Err(error) => Err(format!("Could not send {name} to {}: {error}", self.name)),
        }
    }

    /// A file a terminal shows, if it is there: `text` is resolved against
    /// the terminal's folder, with `~` meaning that computer's Home. Files
    /// outside its projects and Home can't be checked, so an absolute path
    /// there is taken as it is, for copying its path.
    pub(crate) fn resolve(&self, text: &str, directory: &Path) -> Option<Target> {
        let absolute = text.starts_with('/') || text.starts_with("file://");
        Target::file(text, directory, Some(&self.home), |path| {
            if self.reaches(path) {
                self.exists(path)
            } else {
                absolute
            }
        })
    }

    /// Whether file requests reach `path`: it is in a project or Home.
    pub(crate) fn reaches(&self, path: &Path) -> bool {
        self.locate(path).is_some()
    }

    /// The project or Home holding `path`, and `path` inside it.
    fn locate(&self, path: &Path) -> Option<(ProjectId, PathBuf)> {
        self.roots.iter().find_map(|(project, root)| {
            let relative = path.strip_prefix(root).ok()?;
            Some((*project, relative.to_path_buf()))
        })
    }

    fn files(&self, path: &Path, action: fn(ServerPath) -> FilesAction) -> Option<FilesReply> {
        let (project, relative) = self.locate(path)?;
        self.client
            .files(FilesRequest {
                project,
                action: action(wire_path(&relative)),
            })
            .ok()
    }

    fn exists(&self, path: &Path) -> bool {
        self.files(path, FilesAction::Stat).is_some()
    }

    /// Whether a project's folder is there, as this computer checks its own:
    /// a worktree also needs its `.git` file. `None` when the server
    /// couldn't say.
    fn folder_available(&self, project: ProjectId, worktree: bool) -> Option<bool> {
        let stat = |path: &[u8]| {
            self.client.files(FilesRequest {
                project,
                action: FilesAction::Stat(ServerPath(path.to_vec())),
            })
        };
        let missing = |error: &ClientError| matches!(error, ClientError::Server(error) if error.code == ErrorCode::BadPath);
        match stat(b"") {
            Ok(_) => {}
            Err(error) if missing(&error) => return Some(false),
            Err(_) => return None,
        }
        if !worktree {
            return Some(true);
        }
        match stat(b".git") {
            Ok(FilesReply::Info(info)) => Some(!info.is_directory),
            Err(error) if missing(&error) => Some(false),
            _ => None,
        }
    }

    /// The file's bytes, up to the files limit.
    pub(crate) fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        let Some((project, relative)) = self.locate(path) else {
            return Err(format!(
                "Muxy can open copies only of files in {}'s projects and Home.",
                self.name
            ));
        };
        let request = FilesRequest {
            project,
            action: FilesAction::ReadBytes(wire_path(&relative)),
        };
        match self.client.files(request) {
            Ok(FilesReply::Bytes(file)) => Ok(file.bytes),
            Ok(_) => Err("The server sent something other than the file.".into()),
            Err(ClientError::Server(error)) => Err(error.message),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn too_large(name: &str, server: &str) -> String {
    format!("{name} is larger than 100 MiB, the most Muxy sends to {server}.")
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn wire_path(path: &Path) -> ServerPath {
    use std::os::unix::ffi::OsStrExt;
    ServerPath(path.as_os_str().as_bytes().to_vec())
}

fn local_path(path: &ServerPath) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(&path.0))
}

/// Copied files, or an image copied without text, which a terminal on
/// another computer gets sent instead of their names or a paste key it can't
/// act on. Text pastes as text, as it does here.
pub(crate) fn clipboard_upload(item: &gpui::ClipboardItem) -> Option<Upload> {
    #[cfg(not(test))]
    let copied = muxy_ui::pasteboard::read_content().ok();
    #[cfg(test)]
    let copied: Option<muxy_ui::pasteboard::Content> = None;
    if let Some(muxy_ui::pasteboard::Content::Files(paths)) = copied {
        return Some(Upload::Files(paths));
    }
    if item.text().is_some() {
        return None;
    }
    if let Some(muxy_ui::pasteboard::Content::Image(bytes)) = copied {
        let extension = image_extension(&bytes);
        return Some(Upload::Image(bytes, extension));
    }
    item.entries().iter().find_map(|entry| match entry {
        gpui::ClipboardEntry::Image(image) => Some(Upload::Image(
            image.bytes.clone(),
            image_extension(&image.bytes),
        )),
        gpui::ClipboardEntry::String(_) => None,
    })
}

/// The extension of an image's format, from its first bytes.
pub(crate) fn image_extension(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xff, 0xd8, 0xff, ..] => "jpg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        [b'I', b'I', 0x2a, 0, ..] | [b'M', b'M', 0, 0x2a, ..] => "tiff",
        _ => "png",
    }
}

/// Saves a read-only copy of another computer's file in this computer's
/// temporary folder, named as a copy, and returns its path.
pub(crate) fn save_copy(bytes: &[u8], original: &Path, server: &str) -> io::Result<PathBuf> {
    let name = display_name(original);
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name.as_str(), ""),
    };
    let server = server.replace(['/', '\0'], "-");
    let folder = std::env::temp_dir()
        .join("Muxy Copies")
        .join(muxy_protocol::OperationId::new().to_string());
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&folder)?;
    let copy = folder.join(format!("{stem} (copy from {server}){extension}"));
    std::fs::write(&copy, bytes)?;
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o444))?;
    Ok(copy)
}

/// Opens a file with an app on this computer.
pub(crate) type Open = Arc<dyn Fn(&Path) -> io::Result<()> + Send + Sync>;

/// A remote file link's menu, and how copies open.
pub(crate) struct RemoteLinks {
    /// The file whose menu is open, and the terminal that showed it.
    menu: Option<(PaneId, ServerId, PathBuf)>,
    /// Opens a saved copy with this computer's default app.
    pub(crate) open: Open,
}

impl Default for RemoteLinks {
    fn default() -> Self {
        Self {
            menu: None,
            open: Arc::new(crate::opener::open_with_default_app),
        }
    }
}

impl AppModel {
    /// Another computer's files, once its second connection is open. Asks
    /// for that connection when it isn't.
    pub(crate) fn remote_files(
        &mut self,
        server: ServerId,
        cx: &mut Context<Self>,
    ) -> Result<RemoteFiles, String> {
        let name = self.server_label(server);
        if !self.ready(server) || !self.confirmed(server) {
            return Err(format!("{name} isn't connected."));
        }
        let Some(client) = self.extensions.client(server) else {
            self.extension_client(server, cx);
            return Err(format!(
                "Still connecting to {name}. Try again in a moment."
            ));
        };
        let home = self
            .remote_home(server)
            .ok_or_else(|| format!("{name}'s projects haven't loaded yet."))?
            .to_path_buf();
        let mut roots: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.server_id == server)
            .map(|project| (project.id, project.directory.clone()))
            .collect();
        roots.sort_by_key(|(_, root)| Reverse(root.as_os_str().len()));
        Ok(RemoteFiles {
            client,
            name,
            roots,
            home,
        })
    }

    /// Asks another computer's server whether its projects' folders are
    /// there, as this computer's are checked on disk. A project whose folder
    /// is gone shows as failed.
    pub(crate) fn check_remote_projects(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() || !self.ready(server) || self.extensions.client(server).is_none() {
            return;
        }
        let Ok(files) = self.remote_files(server, cx) else {
            return;
        };
        let projects: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.server_id == server && !project.home)
            .map(|project| {
                (
                    project.id,
                    project.kind == Some(muxy_app_core::ProjectKind::Worktree),
                )
            })
            .collect();
        if projects.is_empty() {
            return;
        }
        let generation = self.generation(server);
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.status_checks += 1;
        let check = runtime.status_checks;
        let checked = cx.background_executor().spawn(async move {
            projects
                .into_iter()
                .filter_map(|(project, worktree)| {
                    Some((project, files.folder_available(project, worktree)?))
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |model, cx| {
            let checked = checked.await;
            let _ = model.update(cx, |model, cx| {
                let latest = model
                    .servers
                    .get(server)
                    .is_some_and(|runtime| runtime.status_checks == check);
                if model.generation(server) != generation || !model.ready(server) || !latest {
                    return;
                }
                let mut changed = false;
                for (project, available) in checked {
                    let status = if available {
                        muxy_app_core::ProjectStatus::Available
                    } else {
                        muxy_app_core::ProjectStatus::Missing
                    };
                    if model
                        .state
                        .project(project)
                        .is_some_and(|project| project.status() != status)
                    {
                        changed |= model
                            .state
                            .set_remote_project_status(project, status)
                            .is_ok();
                    }
                }
                if changed {
                    model.sync_visible(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Sends what was dropped or pasted on a terminal of another computer,
    /// then pastes where it landed there, if the terminal is still the same.
    pub(super) fn upload_to_pane(&mut self, pane: PaneId, upload: Upload, cx: &mut Context<Self>) {
        let (Some(attachment), Some(session)) =
            (self.attachment(pane, cx), self.pane_session(pane))
        else {
            return;
        };
        let (server, _) = attachment;
        let files = match self.remote_files(server, cx) {
            Ok(files) => files,
            Err(error) => {
                self.fail(error, cx);
                return;
            }
        };
        let generation = self.generation(server);
        let sent = cx
            .background_executor()
            .spawn(async move { files.upload(session, upload) });
        cx.spawn(async move |model, cx| {
            let result = sent.await;
            let _ = model.update(cx, |model, cx| match result {
                Ok(paths) => {
                    let current = model.generation(server) == generation
                        && model.attachment(pane, cx) == Some(attachment);
                    if let Some(view) = model.terminal(&pane).map(|pane| pane.view.clone())
                        && current
                    {
                        view.update(cx, |pane, cx| pane.paste_paths(&paths, cx));
                    }
                }
                Err(error) => model.fail(error, cx),
            });
        })
        .detach();
    }

    /// Looks up a file a terminal of another computer shows, through its
    /// server, and tells the terminal whether it is a link.
    pub(super) fn resolve_remote_link(
        &mut self,
        pane: PaneId,
        candidate: crate::views::terminal::links::Candidate,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.terminal(&pane).map(|pane| pane.view.clone()) else {
            return;
        };
        let Some(context) = view.read(cx).link_context() else {
            return;
        };
        let Ok(files) = self.remote_files(context.server, cx) else {
            return;
        };
        let text = candidate.text.clone();
        let resolved = cx
            .background_executor()
            .spawn(async move { files.resolve(&text, &context.directory) });
        cx.spawn(async move |_, cx| {
            let target = resolved.await;
            let _ = view.update(cx, |pane, cx| pane.link_resolved(&candidate, target, cx));
        })
        .detach();
    }

    /// A remote file link offers a copy here, or its path.
    pub(super) fn remote_link_menu(
        &mut self,
        pane: PaneId,
        server: ServerId,
        file: &FileLocation,
        cx: &mut Context<Self>,
    ) {
        let reachable = self
            .remote_files(server, cx)
            .is_ok_and(|files| files.reaches(&file.path));
        self.remote_links.menu = Some((pane, server, file.path.clone()));
        let mut copy = Item::action("Open a Copy", Command::OpenRemoteCopy);
        if !reachable {
            copy = copy.disabled();
        }
        let items = vec![copy, Item::action("Copy Path", Command::CopyRemotePath)];
        let model = cx.entity().downgrade();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let position = window.mouse_position();
                let _ = model.update(cx, |model, cx| {
                    if model.grids.contains_key(&pane) {
                        model.open_menu(items, position, window, cx);
                    }
                });
            });
        });
    }

    pub(crate) fn copy_remote_path(&mut self, cx: &mut Context<Self>) {
        if let Some((_, _, path)) = self.remote_links.menu.take() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                path.to_string_lossy().into_owned(),
            ));
            self.show_notice("Copied the path".into(), cx);
        }
    }

    /// Reads the file through its server, saves a read-only copy here, and
    /// opens it with the default app.
    pub(crate) fn open_remote_copy(&mut self, cx: &mut Context<Self>) {
        let Some((_, server, path)) = self.remote_links.menu.take() else {
            return;
        };
        let name = self.server_label(server);
        let files = match self.remote_files(server, cx) {
            Ok(files) => files,
            Err(error) => {
                self.fail(format!("Could not open a copy: {error}"), cx);
                return;
            }
        };
        let open = self.remote_links.open.clone();
        let opened = cx.background_executor().spawn(async move {
            let bytes = files.read(&path)?;
            let copy = save_copy(&bytes, &path, &name).map_err(|error| error.to_string())?;
            open(&copy).map_err(|error| error.to_string())
        });
        cx.spawn(async move |model, cx| {
            if let Err(error) = opened.await {
                let _ = model.update(cx, |model, cx| {
                    model.fail(format!("Could not open a copy: {error}"), cx);
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "Test fixtures and assertions fail immediately"
)]
mod tests {
    use super::*;

    #[test]
    fn image_formats_name_their_extension() {
        for (bytes, extension) in [
            (&b"\x89PNG\r\n"[..], "png"),
            (b"\xff\xd8\xff\xe0", "jpg"),
            (b"GIF89a", "gif"),
            (b"RIFF\0\0\0\0WEBPVP8", "webp"),
            (b"II*\0", "tiff"),
            (b"MM\0*", "tiff"),
            (b"", "png"),
        ] {
            assert_eq!(image_extension(bytes), extension);
        }
    }

    #[test]
    fn copies_are_read_only_and_named_as_copies() {
        let copy = save_copy(b"fn main() {}", Path::new("/srv/app/main.rs"), "box/1").unwrap();
        assert_eq!(
            copy.file_name().unwrap().to_str(),
            Some("main (copy from box-1).rs")
        );
        assert_eq!(std::fs::read(&copy).unwrap(), b"fn main() {}");
        let mode = std::fs::metadata(&copy).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o444);
        let dotfile = save_copy(b"x", Path::new("/home/dev/.bashrc"), "box").unwrap();
        assert_eq!(
            dotfile.file_name().unwrap().to_str(),
            Some(".bashrc (copy from box)")
        );
        for copy in [copy, dotfile] {
            std::fs::remove_dir_all(copy.parent().unwrap()).unwrap();
        }
    }
}
