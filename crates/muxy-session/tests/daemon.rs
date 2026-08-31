pub mod support;

use muxy_proto::session::SessionStatus;
use muxy_session::{SessionClient, current_build_mode};
use support::TestDaemon;

#[test]
fn daemon_creates_idempotently_lists_and_exits_when_empty() {
    let daemon = TestDaemon::start();
    let mut client = daemon.client();
    client.ping().unwrap();
    let request = daemon.request("idempotent");
    let first = client.create_session(request.clone()).unwrap();
    let mut proposed = request.clone();
    proposed.session_id = muxy_proto::session::SessionId::new();
    let second = client.create_session(proposed).unwrap();
    assert_eq!(first.session_id, second.session_id);
    assert_eq!(first.status, SessionStatus::Running);
    assert_eq!(client.list_sessions().unwrap().len(), 1);
    assert!(client.acknowledge_exited_session(first.session_id).is_err());
    assert_eq!(client.list_sessions().unwrap().len(), 1);

    let mut conflict = request;
    conflict.argv.push("different".into());
    assert_eq!(
        client.create_session(conflict).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );

    client.end_session(first.session_id).unwrap();
    assert!(client.list_sessions().unwrap().is_empty());
    drop(client);
    daemon.finish(true);
}

#[test]
fn concurrent_create_is_idempotent_and_control_operations_complete() {
    let daemon = TestDaemon::start();
    let request = daemon.request("concurrent");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let socket = daemon.socket.clone();
            let mut request = request.clone();
            request.session_id = muxy_proto::session::SessionId::new();
            std::thread::spawn(move || {
                let mut client = SessionClient::connect(socket, current_build_mode()).unwrap();
                client.create_session(request).unwrap()
            })
        })
        .collect();
    let descriptors: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.session_id == descriptors[0].session_id)
    );

    let mut client = daemon.client();
    client
        .set_workspace_placement(descriptors[0].session_id, None)
        .unwrap();
    assert!(
        client
            .get_session(descriptors[0].session_id)
            .unwrap()
            .unwrap()
            .placement
            .is_none()
    );
    client.end_sessions_by_owner(request.owner.clone()).unwrap();
    assert!(client.list_sessions().unwrap().is_empty());

    let first = client
        .create_session(daemon.request("end-all-first"))
        .unwrap();
    let second = client
        .create_session(daemon.request("end-all-second"))
        .unwrap();
    assert_ne!(first.session_id, second.session_id);
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    client.end_all_sessions().unwrap();
    assert!(client.list_sessions().unwrap().is_empty());
    drop(client);
    daemon.finish(false);
}
