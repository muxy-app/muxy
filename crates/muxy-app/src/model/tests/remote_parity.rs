//! Remote projects work like local ones where this computer's disk was
//! assumed: files sent to terminals, file links, and dropped connections.

use super::servers::{connect_remote, entry, serve_remote, work};
use super::*;
use crate::views::composer::ComposerEvent;
use muxy_app_core::opener::{FileLocation, Target};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// A 1×1 PNG.
const PNG: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Another computer, played by a real server behind a fake ssh. Its files
/// are in `directory`: the project `app`, and the server's `uploads`.
struct Machine {
    directory: tempfile::TempDir,
    socket: PathBuf,
}

impl Machine {
    fn new() -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("muxy-remote-")
            .tempdir_in("/tmp")?;
        let socket = directory.path().join("remote.sock");
        serve_remote(&socket)?;
        std::fs::create_dir(directory.path().join("app"))?;
        Ok(Self { directory, socket })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.directory
            .path()
            .canonicalize()
            .expect("directory")
            .join(name)
    }

    /// An ssh that fails like a real one while a marker file is there:
    /// `offline` (unreachable) or `refuse` (a refused login). It counts its
    /// runs in `attempts`.
    fn ssh(&self) -> Result<muxy_client::SshTarget> {
        let directory = self.directory.path().display();
        let program = self.directory.path().join("ssh");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\n\
                 echo run >> '{directory}/attempts'\n\
                 if [ -e '{directory}/refuse' ]; then echo 'dev@box: Permission denied (publickey).' >&2; exit 255; fi\n\
                 if [ -e '{directory}/offline' ]; then echo 'ssh: connect to host box port 22: Connection refused' >&2; exit 255; fi\n\
                 printf 'MUXY-STDIO/1\\n'\n\
                 exec /usr/bin/nc -U '{}'\n",
                self.socket.display()
            ),
        )?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
        Ok(muxy_client::SshTarget::new("box")?.with_program(program))
    }

    fn mark(&self, marker: &str, present: bool) -> Result {
        let path = self.directory.path().join(marker);
        if present {
            std::fs::write(path, "")?;
        } else {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn attempts(&self) -> usize {
        std::fs::read_to_string(self.directory.path().join("attempts"))
            .map_or(0, |attempts| attempts.lines().count())
    }

    /// Drops every connection to its server, as a network failure would.
    fn drop_connections(&self) -> Result {
        std::process::Command::new("/usr/bin/pkill")
            .arg("-f")
            .arg(format!("nc -U {}", self.socket.display()))
            .status()?;
        Ok(())
    }

    /// The files its server keeps uploaded, by name.
    fn uploads(&self) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        let Ok(sessions) = std::fs::read_dir(self.directory.path().join("uploads")) else {
            return files;
        };
        for session in sessions.flatten() {
            for file in std::fs::read_dir(session.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                files.push((
                    file.file_name().to_string_lossy().into_owned(),
                    std::fs::read(file.path()).unwrap_or_default(),
                ));
            }
        }
        files.sort();
        files
    }
}

/// The app with `machine` as the remote server "box", showing a live
/// terminal in its project `app`.
fn remote_terminal<'a>(
    cx: &'a mut TestAppContext,
    machine: &Machine,
) -> Result<(
    Entity<AppModel>,
    &'a mut VisualTestContext,
    ServerId,
    PaneId,
)> {
    let remote = ServerId::new();
    let (mut boot, _local_work, _) =
        remote_boot(AppState::bootstrap()?, vec![entry(remote, "box")]);
    boot.settings.composer.presentation = muxy_app_core::settings::ComposerPresentation::Panel;
    let (updates, received) = async_channel::unbounded();
    let ssh = machine.ssh()?;
    boot.workers = crate::boot::Workers::with(move |server, _| {
        crate::boot::worker(
            server,
            crate::boot::Target::Ssh(ssh.clone()),
            updates.clone(),
        )
    });
    boot.updates = received;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });
    wait(cx, &view, |model, _| {
        model.state.server_home(remote).is_some() && model.extensions.client(remote).is_some()
    })?;
    let folder = machine.path("app");
    let pane = view.update(cx, |model, cx| {
        let project = model.add_project_on(remote, folder, cx);
        model.select_project(project.expect("remote project"), cx);
        model.new_tab(cx);
        let pane = model.active_pane().expect("pane");
        model.start_attach(
            pane,
            Size {
                cols: 120,
                rows: 24,
            },
            cx,
        );
        pane
    });
    wait(cx, &view, |model, cx| model.attachment(pane, cx).is_some())?;
    wait_text(cx, &view, "$")?;
    Ok((view, cx, remote, pane))
}

fn pane_view(
    view: &Entity<AppModel>,
    pane: PaneId,
    cx: &VisualTestContext,
) -> Entity<TerminalPane> {
    view.read_with(cx, |model, _| {
        model.terminal(&pane).expect("terminal").view.clone()
    })
}

#[gpui::test]
fn files_and_images_given_to_a_remote_terminal_arrive_there_by_path(cx: &mut TestAppContext) {
    uploads(cx).expect("uploads to a remote terminal");
}

fn uploads(cx: &mut TestAppContext) -> Result {
    let machine = Machine::new()?;
    let (view, cx, _, pane) = remote_terminal(cx, &machine)?;
    let here = tempfile::tempdir()?;
    let notes = here.path().join("notes.txt");
    std::fs::write(&notes, "dropped notes")?;
    let terminal = pane_view(&view, pane, cx);
    terminal.update(cx, |pane, cx| {
        pane.drop_paths(std::slice::from_ref(&notes), cx);
    });
    let uploaded = machine.path("uploads");
    wait_text(cx, &view, &format!("{}/", uploaded.display()))?;
    wait_text(cx, &view, "/notes.txt'")?;
    assert_eq!(
        machine.uploads(),
        [("notes.txt".to_owned(), b"dropped notes".to_vec())]
    );
    cx.update(|_, cx| {
        let image = gpui::Image::from_bytes(gpui::ImageFormat::Png, PNG.to_vec());
        cx.write_to_clipboard(gpui::ClipboardItem::new_image(&image));
    });
    terminal.update(cx, TerminalPane::paste_clipboard);
    wait_text(cx, &view, "/pasted-image.png'")?;
    assert!(
        machine
            .uploads()
            .contains(&("pasted-image.png".to_owned(), PNG.to_vec()))
    );
    terminal.update(cx, |pane, cx| {
        pane.drop_paths(&[here.path().to_path_buf()], cx);
    });
    wait(cx, &view, |model, _| {
        model
            .error
            .as_deref()
            .is_some_and(|error| error.contains("is a folder"))
    })?;
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

#[gpui::test]
fn the_composer_sends_images_to_a_remote_terminal_and_keeps_the_draft_when_it_cannot(
    cx: &mut TestAppContext,
) {
    composer(cx).expect("composer to a remote terminal");
}

fn composer(cx: &mut TestAppContext) -> Result {
    let machine = Machine::new()?;
    let (view, cx, _, _) = remote_terminal(cx, &machine)?;
    cx.update(|window, cx| view.update(cx, |model, cx| model.toggle_composer(window, cx)));
    let composer = view.read_with(cx, |model, _| {
        model.composer.view.clone().expect("composer")
    });
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::Image(PNG.to_vec())));
    wait(cx, &view, |model, cx| {
        model
            .composer
            .view
            .as_ref()
            .is_some_and(|view| !view.read(cx).draft.image_attachments.is_empty())
    })?;
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::Submit(false)));
    wait_text(cx, &view, "/pasted-image.png'")?;
    assert_eq!(machine.uploads().len(), 1);
    let here = tempfile::tempdir()?;
    let huge = here.path().join("huge.bin");
    std::fs::File::create(&huge)?.set_len(muxy_protocol::MAX_UPLOAD_BYTES + 1)?;
    composer.update(cx, |_, cx| {
        cx.emit(ComposerEvent::Files(vec![huge.clone()]));
    });
    composer.update(cx, |_, cx| cx.emit(ComposerEvent::Submit(false)));
    wait(cx, &view, |model, cx| {
        model.composer.view.as_ref().is_some_and(|view| {
            let view = view.read(cx);
            view.error
                .as_deref()
                .is_some_and(|error| error.contains("larger than 100 MiB"))
                && view
                    .draft
                    .file_attachments
                    .contains(&huge.to_string_lossy().into_owned())
        })
    })?;
    assert_eq!(machine.uploads().len(), 1, "nothing more was sent");
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

#[gpui::test]
fn remote_file_links_open_a_read_only_copy_or_copy_their_path(cx: &mut TestAppContext) {
    links(cx).expect("remote file links");
}

fn links(cx: &mut TestAppContext) -> Result {
    let machine = Machine::new()?;
    std::fs::write(machine.path("app").join("notes.txt"), "remote notes")?;
    let (view, cx, remote, pane) = remote_terminal(cx, &machine)?;
    let (files, directory) = view.update(cx, |model, cx| {
        let directory = model
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).link_context())
            .expect("link context")
            .directory;
        (
            model.remote_files(remote, cx).expect("remote files"),
            directory,
        )
    });
    let resolve = |text: &str, _: &mut VisualTestContext| -> Result<Option<Target>> {
        let (files, directory, text) = (files.clone(), directory.clone(), text.to_owned());
        thread::spawn(move || files.resolve(&text, &directory))
            .join()
            .map_err(|_| "resolving panicked".into())
    };
    let notes = machine.path("app").join("notes.txt");
    assert_eq!(
        resolve("notes.txt:3", cx)?,
        Some(Target::File(FileLocation {
            path: notes.clone(),
            line: Some(3),
            column: None,
        }))
    );
    assert_eq!(resolve("missing.txt", cx)?, None);
    let opened = std::sync::Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
    let seen = opened.clone();
    view.update(cx, |model, cx| {
        model.remote_links.open = std::sync::Arc::new(move |path: &Path| {
            seen.lock().expect("opened").push(path.to_path_buf());
            Ok(())
        });
        model.open_terminal_link(
            pane,
            Target::File(FileLocation {
                path: notes.clone(),
                line: None,
                column: None,
            }),
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.menu_outline()),
        [vec!["Open a Copy".to_owned(), "Copy Path".to_owned()]]
    );
    let open = cx.debug_bounds("menu-item-0").expect("Open a Copy");
    cx.simulate_click(open.center(), Modifiers::default());
    wait(cx, &view, |_, _| !opened.lock().expect("opened").is_empty())?;
    let copy = opened.lock().expect("opened")[0].clone();
    assert_eq!(
        copy.file_name().and_then(|name| name.to_str()),
        Some("notes (copy from box).txt")
    );
    assert_eq!(std::fs::read_to_string(&copy)?, "remote notes");
    assert_eq!(
        std::fs::metadata(&copy)?.permissions().mode() & 0o777,
        0o444
    );
    std::fs::remove_dir_all(copy.parent().expect("copy folder"))?;
    let outside = Target::File(FileLocation {
        path: "/etc/hosts".into(),
        line: None,
        column: None,
    });
    assert_eq!(resolve("/etc/hosts", cx)?, Some(outside.clone()));
    view.update(cx, |model, cx| model.open_terminal_link(pane, outside, cx));
    cx.run_until_parked();
    let copy_path = cx.debug_bounds("menu-item-1").expect("Copy Path");
    cx.simulate_click(copy_path.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("/etc/hosts".into())
    );
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

#[gpui::test]
fn a_dropped_remote_reconnects_with_backoff_and_stops_on_a_refused_login(cx: &mut TestAppContext) {
    reconnect(cx).expect("reconnecting a remote");
}

fn reconnect(cx: &mut TestAppContext) -> Result {
    let machine = Machine::new()?;
    let (view, cx, remote, pane) = remote_terminal(cx, &machine)?;
    let session = view.read_with(cx, |model, _| model.pane_session(pane));
    let disconnected =
        |model: &AppModel, _: &gpui::App| model.connection(remote) == ConnectionState::Disconnected;
    let failed = |reason| {
        move |model: &AppModel, _: &gpui::App| {
            model.connection(remote) == ConnectionState::Disconnected
                && model
                    .servers
                    .get(remote)
                    .and_then(|runtime| runtime.failure)
                    == Some(reason)
        }
    };
    machine.mark("offline", true)?;
    machine.drop_connections()?;
    wait(cx, &view, disconnected)?;
    cx.executor().advance_clock(Duration::from_secs(2));
    wait(cx, &view, failed(muxy_client::RemoteReason::Unreachable))?;
    let attempts = machine.attempts();
    machine.mark("offline", false)?;
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    thread::sleep(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(machine.attempts(), attempts, "the second wait is 4 s");
    cx.executor().advance_clock(Duration::from_secs(1));
    wait(cx, &view, |model, cx| model.attachment(pane, cx).is_some())?;
    assert_eq!(
        view.read_with(cx, |model, _| model.pane_session(pane)),
        session,
        "the terminal survived"
    );
    let channel = view.read_with(cx, |model, cx| {
        model.attachment(pane, cx).expect("attached").1
    });
    view.update(cx, |model, cx| {
        model.send(
            remote,
            Work::Input(channel, b"echo back-$((40 + 2))\r".to_vec()),
            cx,
        );
    });
    wait_text(cx, &view, "back-42")?;
    machine.mark("refuse", true)?;
    machine.drop_connections()?;
    wait(cx, &view, disconnected)?;
    cx.executor().advance_clock(Duration::from_secs(2));
    wait(
        cx,
        &view,
        failed(muxy_client::RemoteReason::AuthenticationFailed),
    )?;
    let attempts = machine.attempts();
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    thread::sleep(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(machine.attempts(), attempts, "it waits for Connect");
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

#[gpui::test]
fn remotes_stopped_by_the_user_or_needing_a_password_wait_for_connect(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (boot, _, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![entry(remote, "box")],
    );
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    let connects = |worker: &std::sync::mpsc::Receiver<(u64, Work)>| {
        work(worker)
            .iter()
            .filter(|work| matches!(work, Work::Connect))
            .count()
    };
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        connect_remote(model, remote, vec![], &[], cx);
    });
    connects(&worker);
    view.update(cx, |model, cx| {
        let unreachable = Some(muxy_client::RemoteReason::Unreachable);
        let generation = model.generation(remote);
        model.receive(
            (
                remote,
                generation,
                Update::ConnectFailed("offline".into(), unreachable),
            ),
            cx,
        );
    });
    cx.executor().advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(connects(&worker), 1, "an unreachable server is tried again");
    view.update(cx, |model, cx| {
        let generation = model.generation(remote);
        model.receive((remote, generation, Update::Connected(vec![])), cx);
        let stopped = Update::ServerStopped {
            restart: false,
            result: Ok(()),
        };
        model.receive((remote, generation, stopped), cx);
        model.receive(
            (remote, generation, Update::Event(ClientEvent::Disconnected)),
            cx,
        );
    });
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    assert_eq!(
        connects(&worker),
        0,
        "a server the user stopped stays stopped"
    );
    view.update(cx, |model, cx| {
        model.connect_remote_server(remote, cx);
        let generation = model.generation(remote);
        model.receive((remote, generation, Update::Connected(vec![])), cx);
        model
            .servers
            .get_mut(remote)
            .expect("runtime")
            .password_login = true;
        model.receive(
            (remote, generation, Update::Event(ClientEvent::Disconnected)),
            cx,
        );
    });
    connects(&worker);
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    assert_eq!(
        connects(&worker),
        0,
        "a password login waits until the user connects and types it"
    );
}
