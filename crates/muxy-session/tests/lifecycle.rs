pub mod support;

use muxy_proto::session::{SessionId, SessionStatus};
use muxy_session::{SessionClient, current_build_mode, identity_is_alive};
use std::time::{Duration, Instant};
use support::TestDaemon;

#[test]
fn lifecycle_background_reattach_quit_exact_owner_cleanup_and_shell_exit_are_distinct() {
    let daemon = TestDaemon::start();
    let marker = daemon.root.path().join("startup-once");
    let mut first_request = daemon.request("lifecycle-first");
    first_request.startup_command =
        Some(format!("printf 'once\\n' >> {}", marker.to_string_lossy()));
    first_request.keep_shell_open = true;
    let first_owner = first_request.owner.clone();
    let first_placement = first_request.placement.clone().unwrap();
    let mut control = daemon.client();
    let first = control.create_session(first_request.clone()).unwrap();
    wait_until(|| marker.exists());
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "once\n");

    control
        .set_workspace_placement(first.session_id, None)
        .unwrap();
    assert!(
        control
            .get_session(first.session_id)
            .unwrap()
            .unwrap()
            .placement
            .is_none()
    );
    control
        .set_workspace_placement(first.session_id, Some(first_placement.clone()))
        .unwrap();
    let mut proposed = first_request;
    proposed.session_id = SessionId::new();
    let recovered = control.create_session(proposed).unwrap();
    assert_eq!(recovered.session_id, first.session_id);
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "once\n");

    let second = control
        .create_session(daemon.request("lifecycle-second"))
        .unwrap();
    drop(control);
    assert!(identity_is_alive(first.shell));
    assert!(identity_is_alive(second.shell));

    let mut reopened = SessionClient::connect(&daemon.socket, current_build_mode()).unwrap();
    reopened
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    assert_eq!(
        reopened
            .get_session(first.session_id)
            .unwrap()
            .unwrap()
            .placement,
        Some(first_placement)
    );
    reopened.end_sessions_by_owner(first_owner).unwrap();
    assert!(reopened.get_session(first.session_id).unwrap().is_none());
    assert!(!identity_is_alive(first.shell));
    assert!(identity_is_alive(second.shell));

    let mut exited_request = daemon.request("lifecycle-exited");
    exited_request.startup_command = Some("/bin/sh -c 'exit 7'".into());
    let exited = reopened.create_session(exited_request).unwrap();
    wait_until(|| {
        reopened
            .get_session(exited.session_id)
            .ok()
            .flatten()
            .is_some_and(|descriptor| {
                descriptor.status == SessionStatus::Exited { status: Some(7) }
            })
    });
    assert!(reopened.get_session(exited.session_id).unwrap().is_some());
    reopened.end_session(exited.session_id).unwrap();
    reopened.end_all_sessions().unwrap();
    assert!(reopened.list_sessions().unwrap().is_empty());
    drop(reopened);
    daemon.finish(false);
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "lifecycle condition timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}
