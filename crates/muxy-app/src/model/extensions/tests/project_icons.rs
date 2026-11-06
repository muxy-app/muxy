use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn files_client() -> (muxy_client::Client, Arc<AtomicUsize>) {
    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{
        CONTROL, FilesAction, FilesReply, Message, ReplyBody, RequestBody, ServerInfo,
    };
    let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
    let requests = Arc::new(AtomicUsize::new(0));
    let counted = requests.clone();
    std::thread::spawn(move || {
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
        while let Ok((
            _,
            Message::Request {
                id,
                body: RequestBody::Files(request),
            },
        )) = decoder.next()
        {
            if matches!(request.action, FilesAction::List(_)) {
                counted.fetch_add(1, Ordering::SeqCst);
            }
            if encoder
                .send(
                    CONTROL,
                    &Message::Reply {
                        id,
                        body: ReplyBody::Files(FilesReply::Done),
                    },
                )
                .is_err()
            {
                return;
            }
        }
    });
    (
        muxy_client::Client::from_stream(Box::new(client)).expect("client"),
        requests,
    )
}

#[gpui::test]
fn project_icons_wait_for_remote_creation_and_retry_after_file_reconnection(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("local project");
    std::fs::write(
        root.path().join("favicon.png"),
        include_bytes!("../../../../../muxy-protocol/tests/fixtures/project-logo.png"),
    )
    .expect("local icon");
    let remote = ServerId::new();
    let mut state = AppState::bootstrap().expect("state");
    let local = state
        .add_project(ServerId::local(), root.path().into())
        .expect("local project");
    let mut home = state
        .server_home(ServerId::local())
        .expect("home")
        .descriptor();
    home.id = ProjectId::new();
    home.directory = muxy_protocol::ServerPath(b"/home/dev".to_vec());
    state
        .apply_catalog(
            remote,
            &muxy_protocol::CatalogPage {
                server: muxy_protocol::ServerIdentity::from_u128(2),
                home: home.id,
                revision: 1,
                projects: vec![home],
                next: None,
                legacy_home: None,
            },
        )
        .expect("remote catalog");
    let id = state
        .add_project(remote, "/home/dev/project".into())
        .expect("remote project");
    let server = muxy_app_core::settings::ServerEntry {
        id: remote,
        name: "box".into(),
        ssh: "dev@box".into(),
        ssh_command: None,
        identity_file: None,
        password_login: false,
        group: None,
        paired: None,
    };
    let (boot, _requests, _remotes) = crate::model::tests::remote_boot(state, vec![server]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (client, requests) = files_client();
    view.update(cx, |model, cx| {
        let runtime = model.servers.get_mut(remote).expect("server");
        runtime.connection = crate::model::ConnectionState::Ready;
        runtime.catalog.restore = None;
        model.extensions.clients.insert(remote, client);
        model.extensions.connection_installs.insert(remote, 1);
        model.sync_project_icons(cx);
        assert!(model.state.project_creation_pending(id));
    });
    cx.run_until_parked();
    assert_eq!(requests.load(Ordering::SeqCst), 0, "creation is pending");
    view.update(cx, |model, cx| {
        let operation = model.state.project_intents(remote)[0].operation;
        model
            .state
            .complete_project_intent(remote, operation, true)
            .expect("created");
        model.sync_project_icons(cx);
    });
    cx.run_until_parked();
    let completed = requests.load(Ordering::SeqCst);
    assert!(completed > 0, "creation completion starts discovery");
    view.update(cx, AppModel::sync_project_icons);
    cx.run_until_parked();
    assert_eq!(
        requests.load(Ordering::SeqCst),
        completed,
        "empty result is cached"
    );
    let (client, retried) = files_client();
    view.update(cx, |model, cx| {
        let local_image = model
            .project_icons
            .image(model.state.project(local).expect("local project"))
            .expect("local icon");
        model.extensions.clients.insert(remote, client);
        model.extensions.connection_installs.insert(remote, 2);
        model.extension_connected(remote, cx);
        assert_eq!(
            model
                .project_icons
                .image(model.state.project(local).expect("local project"))
                .expect("cached local icon")
                .id,
            local_image.id
        );
    });
    cx.run_until_parked();
    assert!(
        retried.load(Ordering::SeqCst) > 0,
        "reinstalled connection retries the scan"
    );
}
