use std::cmp::Reverse;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::Context;
use muxy_app_core::opener::{FileLocation, Target};
use muxy_app_core::{PaneId, ServerId};
use muxy_client::{Client, ClientError};
use muxy_protocol::{ErrorCode, MAX_UPLOAD_BYTES, ProjectId, ServerPath, SessionId};
use muxy_ui::tr;

use super::AppModel;
use crate::views::menu::{Command, Item};

#[derive(Clone, Debug)]
pub(crate) enum Upload {
    Files(Vec<PathBuf>),
    Image(Vec<u8>, &'static str),
}

#[derive(Clone)]
pub(crate) struct RemoteFiles {
    client: Client,
    name: String,
    roots: Vec<(ProjectId, PathBuf)>,
    home: PathBuf,
}

impl RemoteFiles {
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
        read_upload(path, MAX_UPLOAD_BYTES).map_err(|error| match error {
            UploadError::Folder => tr!(
                "%@ is a folder. Only files can be sent to %@.",
                name,
                &self.name
            )
            .to_string(),
            UploadError::TooLarge => too_large(name, &self.name),
            UploadError::Unreadable(error) => {
                tr!("Could not read %@: %@", name, error.to_string()).to_string()
            }
        })
    }

    fn send(&self, session: SessionId, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        match self.client.upload(session, name, bytes) {
            Ok(path) => Ok(path.into()),
            Err(ClientError::Server(error)) if error.code == ErrorCode::Unsupported => Err(tr!(
                "Muxy on %@ can't receive files. Update Muxy there.",
                &self.name
            )
            .to_string()),
            Err(ClientError::Server(error)) => Err(tr!(
                "Could not send %@ to %@: %@",
                name,
                &self.name,
                &error.message
            )
            .to_string()),
            Err(error) => Err(tr!(
                "Could not send %@ to %@: %@",
                name,
                &self.name,
                error.to_string()
            )
            .to_string()),
        }
    }

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

    pub(crate) fn reaches(&self, path: &Path) -> bool {
        self.locate(path).is_some()
    }

    fn locate(&self, path: &Path) -> Option<(ProjectId, PathBuf)> {
        self.roots.iter().find_map(|(project, root)| {
            let relative = path.strip_prefix(root).ok()?;
            Some((*project, relative.to_path_buf()))
        })
    }

    fn exists(&self, path: &Path) -> bool {
        self.locate(path).is_some_and(|(project, relative)| {
            self.client
                .file_info(project, ServerPath::from(&relative))
                .is_ok()
        })
    }

    pub(crate) fn project_file(
        &self,
        project: ProjectId,
        path: &Path,
        root: &Path,
    ) -> Option<String> {
        let relative = path.strip_prefix(root).ok()?;
        let info = self
            .client
            .file_info(project, ServerPath::from(relative))
            .ok()?;
        if info.is_directory {
            return None;
        }
        info.path.as_path().to_str().map(str::to_owned)
    }

    pub(crate) fn folder_available(&self, project: ProjectId, worktree: bool) -> Option<bool> {
        let stat = |path: &[u8]| self.client.file_info(project, ServerPath(path.to_vec()));
        let missing = |error: &ClientError| matches!(error, ClientError::Server(error) if error.code == ErrorCode::BadPath);
        match stat(b"") {
            Ok(info) if info.is_directory => {}
            Ok(_) => return Some(false),
            Err(error) if missing(&error) => return Some(false),
            _ => return None,
        }
        if !worktree {
            return Some(true);
        }
        match stat(b".git") {
            Ok(info) => Some(!info.is_directory),
            Err(error) if missing(&error) => Some(false),
            _ => None,
        }
    }

    pub(crate) fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        let Some((project, relative)) = self.locate(path) else {
            return Err(tr!(
                "Muxy can open copies only of files in %@'s projects and Home.",
                &self.name
            )
            .to_string());
        };
        match self
            .client
            .read_file_bytes(project, ServerPath::from(&relative))
        {
            Ok(file) => Ok(file.bytes),
            Err(ClientError::UnexpectedReply(_)) => {
                Err(tr!("The server sent something other than the file.").to_string())
            }
            Err(ClientError::Server(error)) => Err(error.message),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn too_large(name: &str, server: &str) -> String {
    tr!(
        "%@ is larger than 100 MiB, the most Muxy sends to %@.",
        name,
        server
    )
    .to_string()
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(crate) fn clipboard_upload(item: &gpui::ClipboardItem, cx: &gpui::App) -> Option<Upload> {
    #[cfg(not(test))]
    let copied = muxy_ui::pasteboard::read_content(cx).ok();
    #[cfg(test)]
    let _ = cx;
    #[cfg(test)]
    let copied: Option<muxy_ui::pasteboard::Content> = None;
    upload_for(copied, item)
}

fn upload_for(
    copied: Option<muxy_ui::pasteboard::Content>,
    item: &gpui::ClipboardItem,
) -> Option<Upload> {
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
        gpui::ClipboardEntry::String(_) | gpui::ClipboardEntry::ExternalPaths(_) => None,
    })
}

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

#[derive(Debug)]
enum UploadError {
    Folder,
    TooLarge,
    Unreadable(io::Error),
}

fn read_upload(path: &Path, limit: u64) -> Result<Vec<u8>, UploadError> {
    use std::io::Read;
    if std::fs::metadata(path)
        .map_err(UploadError::Unreadable)?
        .is_dir()
    {
        return Err(UploadError::Folder);
    }
    let file = open_without_waiting(path).map_err(UploadError::Unreadable)?;
    let metadata = file.metadata().map_err(UploadError::Unreadable)?;
    if metadata.is_dir() {
        return Err(UploadError::Folder);
    }
    if !metadata.is_file() {
        return Err(UploadError::Unreadable(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        )));
    }
    if metadata.len() > limit {
        return Err(UploadError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(UploadError::Unreadable)?;
    if bytes.len() as u64 > limit {
        return Err(UploadError::TooLarge);
    }
    Ok(bytes)
}

fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags};
        Ok(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into())
    }
    #[cfg(windows)]
    {
        std::fs::File::open(path)
    }
}

pub(crate) fn save_copy(bytes: &[u8], original: &Path, server: &str) -> io::Result<PathBuf> {
    let name = display_name(original).replace(['/', '\\', '\0'], "-");
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name.as_str(), ""),
    };
    let server = server.replace(['/', '\\', '\0'], "-");
    let mut folder = tempfile::Builder::new();
    folder.prefix("Muxy Copy ");
    #[cfg(unix)]
    folder.permissions(std::fs::Permissions::from_mode(0o700));
    let folder = folder.tempdir()?.keep();
    let copy = folder.join(format!("{stem} (copy from {server}){extension}"));
    let mut file = muxy_core::files::private_file()
        .write(true)
        .create_new(true)
        .open(&copy)?;
    io::Write::write_all(&mut file, bytes)?;
    #[cfg(unix)]
    let read_only = std::fs::Permissions::from_mode(0o444);
    #[cfg(windows)]
    let read_only = {
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(true);
        permissions
    };
    file.set_permissions(read_only)?;
    Ok(copy)
}

pub(crate) type Open = Arc<dyn Fn(&Path) -> io::Result<()> + Send + Sync>;

pub(crate) struct RemoteLinks {
    menu: Option<(PaneId, ServerId, PathBuf)>,
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

const REMOTE_LINK_TRIES: usize = 32;

impl AppModel {
    pub(crate) fn remote_files(
        &mut self,
        server: ServerId,
        cx: &mut Context<Self>,
    ) -> Result<RemoteFiles, String> {
        let name = self.server_label(server);
        if !self.ready(server) || !self.confirmed(server) {
            return Err(tr!("%@ isn't connected.", &name).to_string());
        }
        let Some(client) = self.extensions.client(server) else {
            self.extension_client(server, cx);
            return Err(tr!("Still connecting to %@. Try again in a moment.", &name).to_string());
        };
        let home = self
            .remote_home(server)
            .ok_or_else(|| tr!("%@'s projects haven't loaded yet.", &name).to_string())?
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

    pub(super) fn upload_to_pane(&mut self, pane: PaneId, upload: Upload, cx: &mut Context<Self>) {
        let (Some(attachment), Some((_, session))) =
            (self.attachment(pane, cx), self.state.pane_session(pane))
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
        let mut wanted = candidate.clone();
        wanted.wider.truncate(REMOTE_LINK_TRIES);
        let resolved = cx
            .background_executor()
            .spawn(async move { wanted.resolve(|text| files.resolve(text, &context.directory)) });
        cx.spawn(async move |_, cx| {
            let found = resolved.await;
            let _ = view.update(cx, |pane, cx| pane.link_resolved(&candidate, found, cx));
        })
        .detach();
    }

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
        let mut copy = Item::action(tr!("Open a Copy"), Command::OpenRemoteCopy);
        if !reachable {
            copy = copy.disabled();
        }
        let items = vec![
            copy,
            Item::action(tr!("Copy Path"), Command::CopyRemotePath),
        ];
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
            self.show_notice(tr!("Copied the path").to_string(), cx);
        }
    }

    pub(crate) fn open_remote_copy(&mut self, cx: &mut Context<Self>) {
        let Some((_, server, path)) = self.remote_links.menu.take() else {
            return;
        };
        let name = self.server_label(server);
        let files = match self.remote_files(server, cx) {
            Ok(files) => files,
            Err(error) => {
                self.fail(tr!("Could not open a copy: %@", &error).to_string(), cx);
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
                    model.fail(tr!("Could not open a copy: %@", &error).to_string(), cx);
                });
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use muxy_ui::pasteboard::Content;

    use super::*;

    #[test]
    fn uploads_read_only_regular_files_within_the_limit() {
        let folder = tempfile::tempdir().expect("folder");
        let file = folder.path().join("a.txt");
        std::fs::write(&file, "hello").expect("write");
        assert_eq!(read_upload(&file, 5).expect("read"), b"hello");
        assert!(matches!(read_upload(&file, 4), Err(UploadError::TooLarge)));
        assert!(matches!(
            read_upload(folder.path(), 5),
            Err(UploadError::Folder)
        ));
        #[cfg(unix)]
        {
            assert!(matches!(
                read_upload(Path::new("/dev/zero"), 5),
                Err(UploadError::Unreadable(_))
            ));
            let pipe = folder.path().join("pipe");
            let made = std::process::Command::new("mkfifo")
                .arg(&pipe)
                .status()
                .expect("mkfifo");
            assert!(made.success());
            assert!(matches!(
                read_upload(&pipe, 5),
                Err(UploadError::Unreadable(_))
            ));
        }
    }

    #[test]
    fn copies_land_in_a_new_private_folder_read_only() {
        let copy = save_copy(b"data", Path::new("/srv/notes.txt"), "box/one").expect("copy");
        assert_eq!(
            copy.file_name().and_then(|name| name.to_str()),
            Some("notes (copy from box-one).txt")
        );
        assert_eq!(std::fs::read(&copy).expect("read"), b"data");
        assert!(
            std::fs::metadata(&copy)
                .expect("metadata")
                .permissions()
                .readonly()
        );
        let folder = copy.parent().expect("folder").to_owned();
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&folder)
                .expect("folder")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let second =
            save_copy(b"data", Path::new("/srv/notes.txt"), "box/one").expect("second copy");
        assert_ne!(
            second.parent(),
            Some(folder.as_path()),
            "each copy gets its own folder"
        );
        for copy in [copy, second] {
            let folder = copy.parent().expect("folder").to_owned();
            let mut permissions = std::fs::metadata(&copy).expect("metadata").permissions();
            #[allow(
                clippy::permissions_set_readonly_false,
                reason = "Lets the test clean up"
            )]
            permissions.set_readonly(false);
            std::fs::set_permissions(&copy, permissions).expect("writable");
            std::fs::remove_dir_all(folder).expect("clean up");
        }
    }

    #[test]
    fn project_file_uses_the_source_project_and_the_servers_resolved_path() {
        use muxy_protocol::wire::{Decoder, Encoder};
        use muxy_protocol::{
            CONTROL, ErrorReply, FileInfo, FilesAction, FilesReply, Message, ReplyBody,
            RequestBody, ServerInfo,
        };

        let project = ProjectId::new();
        let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
        server
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("read timeout");
        let worker = std::thread::spawn(move || {
            let mut decoder = Decoder::new(server.try_clone().expect("clone"));
            let mut encoder = Encoder::new(server);
            decoder.next().expect("hello");
            encoder
                .send(
                    CONTROL,
                    &Message::HelloReply {
                        versions: muxy_protocol::SUPPORTED.to_vec(),
                        server: ServerInfo::current(),
                        features: Vec::new(),
                    },
                )
                .expect("hello reply");
            let info = |path: &str, is_directory| {
                ReplyBody::Files(FilesReply::Info(FileInfo {
                    name: Path::new(path).file_name().map_or_else(
                        || ServerPath::from(""),
                        |name| ServerPath::from(Path::new(name)),
                    ),
                    path: path.into(),
                    is_directory,
                    size: 0,
                }))
            };
            for (path, reply) in [
                ("src/../README.md", info("README.md", false)),
                ("src/alias", info("documents/README.md", false)),
                ("src/..", info("", true)),
                (
                    "src/../../secret.md",
                    ReplyBody::Error(ErrorReply {
                        code: ErrorCode::BadPath,
                        message: "Path escapes the workspace root".into(),
                    }),
                ),
            ] {
                let (
                    _,
                    Message::Request {
                        id,
                        body: RequestBody::Files(request),
                    },
                ) = decoder.next().expect("file request")
                else {
                    panic!("expected file request")
                };
                assert_eq!(request.project, project);
                assert_eq!(request.action, FilesAction::Stat(path.into()));
                encoder
                    .send(CONTROL, &Message::Reply { id, body: reply })
                    .expect("file reply");
            }
        });
        let files = RemoteFiles {
            client: Client::from_stream(Box::new(client)).expect("client"),
            name: "Remote".into(),
            roots: vec![
                (ProjectId::new(), PathBuf::from("/repo/src")),
                (project, PathBuf::from("/repo")),
            ],
            home: "/home/me".into(),
        };
        let open = |path| files.project_file(project, Path::new(path), Path::new("/repo"));
        assert_eq!(open("/outside/file.md"), None);
        assert_eq!(open("/repo/src/../README.md").as_deref(), Some("README.md"));
        assert_eq!(
            open("/repo/src/alias").as_deref(),
            Some("documents/README.md")
        );
        assert_eq!(open("/repo/src/.."), None);
        assert_eq!(open("/repo/src/../../secret.md"), None);
        worker.join().expect("server");
    }

    #[test]
    fn copied_files_are_sent_though_their_names_are_also_on_the_clipboard_as_text() {
        let names = gpui::ClipboardItem::new_string("/home/me/a b.txt\n/home/me/c.png".into());
        let files = vec![
            PathBuf::from("/home/me/a b.txt"),
            PathBuf::from("/home/me/c.png"),
        ];
        assert!(matches!(
            upload_for(Some(Content::Files(files.clone())), &names),
            Some(Upload::Files(sent)) if sent == files
        ));
        assert!(
            upload_for(Some(Content::Text), &names).is_none(),
            "paths copied as text paste as text"
        );
        assert!(upload_for(None, &names).is_none());

        let image = gpui::ClipboardItem::new_image(&gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            b"\x89PNG".to_vec(),
        ));
        assert!(matches!(
            upload_for(Some(Content::Image(b"\xff\xd8\xff".to_vec())), &image),
            Some(Upload::Image(_, "jpg"))
        ));
        assert!(matches!(
            upload_for(None, &image),
            Some(Upload::Image(_, "png"))
        ));
    }
}
