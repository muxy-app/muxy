use super::*;
use muxy_protocol::{
    ListenerStatus, PairedDevice, PairingInvite, PairingOffer, RemoteAccessSettings,
    RemoteAccessState,
};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn access(enabled: bool, pairing: Option<u64>, devices: Vec<PairedDevice>) -> RemoteAccessState {
    RemoteAccessState {
        revision: 1,
        settings: RemoteAccessSettings {
            enabled,
            port: 7419,
        },
        status: if enabled {
            ListenerStatus::Listening
        } else {
            ListenerStatus::Disabled
        },
        pairing_expires_at: pairing,
        devices,
    }
}

fn offer(expires_at: u64) -> PairingOffer {
    PairingOffer {
        invite: PairingInvite {
            hosts: vec!["192.168.1.5".into()],
            port: 7419,
            fingerprint: [1; 32],
            secret: [2; 16],
        },
        expires_at,
    }
}

/// A settings window connected to a stub server that reports the given access.
fn connected(
    cx: &mut TestAppContext,
    state: RemoteAccessState,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.read_remote_access(cx);
        model.receive_remote_access(Ok(state), cx);
    });
    cx.run_until_parked();
    (view, cx, requests)
}

#[gpui::test]
fn a_cancel_during_another_request_still_reaches_the_server(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        model.read_remote_access(cx);
        assert!(model.mobile.busy);
        model.cancel_pairing(cx);
        assert!(model.mobile.pairing.is_none());
    });
    assert!(
        requests
            .try_iter()
            .all(|(_, work)| !matches!(work, Work::CancelPairing))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Some(now() + 300), Vec::new())), cx);
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::CancelPairing))
    );
}

#[gpui::test]
fn closing_settings_withdraws_a_code_still_on_screen(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    assert!(cx.simulate_close());
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::CancelPairing))
    );
    view.read_with(cx, |model, _| assert!(model.mobile.pairing.is_none()));
}

#[gpui::test]
fn closing_settings_withdraws_a_pending_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::pair_phone);
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::StartPairing))
    );
    assert!(cx.simulate_close());
    view.update(cx, |model, cx| {
        assert!(model.settings_window.is_none());
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        assert!(model.mobile.pairing.is_none());
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::CancelPairing))
    );
}
