//! Remote servers reconnect on their own only when the user expects it.

use super::servers::{connect_remote, entry, work};
use super::*;

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
