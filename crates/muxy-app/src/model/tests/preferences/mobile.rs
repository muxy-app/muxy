use super::*;
use muxy_protocol::{
    DeviceId, ListenerStatus, PairedDevice, PairingInvite, PairingOffer, RemoteAccessSettings,
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

fn phone(connected: bool) -> PairedDevice {
    PairedDevice {
        id: DeviceId::from_u128(7),
        name: "Test phone".into(),
        paired_at: 1,
        last_seen: Some(now()),
        connected,
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
        model.connection = ConnectionState::Ready;
        model.read_remote_access(cx);
        model.receive_remote_access(Ok(state), cx);
    });
    cx.run_until_parked();
    (view, cx, requests)
}

#[gpui::test]
fn mobile_access_toggles_and_rejects_invalid_ports(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(false, None, Vec::new()));
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::ReadRemoteAccess))
    );
    click_preference(cx, "settings-category-Mobile");
    assert!(cx.debug_bounds("settings-mobile-status").is_some());
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(false, None, Vec::new())), cx);
        model.change_preference(Change::MobileAccess(true), cx);
        assert!(model.mobile.busy);
    });
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::WriteRemoteAccess(settings) if settings.enabled && settings.port == 7419
    )));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
        for invalid in ["80", "70000", "port"] {
            model.change_preference(Change::Field("mobile-port", invalid.into()), cx);
            assert!(!model.mobile.busy, "{invalid}");
            assert!(
                settings_view(model)
                    .read(cx)
                    .errors
                    .contains_key("mobile-port")
            );
        }
        model.change_preference(Change::Field("mobile-port", "7420".into()), cx);
    });
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::WriteRemoteAccess(settings) if settings.enabled && settings.port == 7420
    )));
}

#[gpui::test]
fn a_pairing_code_shows_until_it_is_used_cancelled_or_expires(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    click_preference(cx, "settings-category-Mobile");
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    let settings = view.read_with(cx, |model, _| settings_view(model));
    settings.update(cx, |_, cx| cx.emit(SettingsEvent::PairPhone));
    cx.run_until_parked();
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::StartPairing))
    );
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-mobile-code").is_some());

    view.update(cx, |model, cx| {
        model.receive_event(ClientEvent::RemoteAccessChanged { revision: 2 }, cx);
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::ReadRemoteAccess))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, vec![phone(true)])), cx);
        assert!(model.mobile.pairing.is_none());
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(Box::leak(
            format!("settings-mobile-device-{}", DeviceId::from_u128(7)).into_boxed_str()
        ))
        .is_some()
    );

    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    settings.update(cx, |_, cx| cx.emit(SettingsEvent::CancelPairing));
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.mobile.pairing.is_none()));
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::CancelPairing))
    );

    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
        model.receive_pairing(Ok(offer(now())), cx);
    });
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.mobile.pairing.is_none()));
}

#[gpui::test]
fn revoking_a_device_asks_first(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, vec![phone(false)]));
    let settings = view.read_with(cx, |model, _| settings_view(model));
    let device = DeviceId::from_u128(7);
    for (answer, revoked) in [("Cancel", false), ("Revoke", true)] {
        settings.update(cx, |_, cx| cx.emit(SettingsEvent::RevokeDevice(device)));
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(answer);
        cx.run_until_parked();
        assert_eq!(
            requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::RevokeDevice(id) if id == device)),
            revoked,
            "{answer}"
        );
    }
}

#[gpui::test]
fn mobile_settings_are_searchable_and_wait_for_a_connection(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    click_preference(cx, "settings-category-Mobile");
    assert!(cx.debug_bounds("settings-section-Mobile").is_some());
    assert!(cx.debug_bounds("settings-mobile-status").is_none());
    assert!(
        requests
            .try_iter()
            .all(|(_, work)| !matches!(work, Work::ReadRemoteAccess))
    );
    cx.simulate_keystrokes("cmd-f");
    cx.simulate_input("pairing code");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert_eq!(
            settings_view(model).read(cx).matching_setting_ids(),
            vec!["mobile-pair"]
        );
    });
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
fn closing_settings_with_the_shortcut_withdraws_the_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    cx.simulate_keystrokes("cmd-w");
    cx.run_until_parked();
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::CancelPairing))
    );
    view.read_with(cx, |model, _| {
        assert!(model.settings_window.is_none());
        assert!(model.mobile.pairing.is_none());
    });
}

fn close_while_pairing(cx: &mut TestAppContext, shortcut: bool) {
    let (view, cx, requests) = connected(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::pair_phone);
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::StartPairing))
    );
    if shortcut {
        cx.simulate_keystrokes("cmd-w");
    } else {
        assert!(cx.simulate_close());
    }
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

#[gpui::test]
fn closing_settings_withdraws_a_pending_code(cx: &mut TestAppContext) {
    close_while_pairing(cx, false);
}

#[gpui::test]
fn closing_settings_with_the_shortcut_withdraws_a_pending_code(cx: &mut TestAppContext) {
    close_while_pairing(cx, true);
}

#[gpui::test]
fn mobile_access_reloads_after_a_reconnect(cx: &mut TestAppContext) {
    let (view, cx, requests) = connected(cx, access(true, None, vec![phone(false)]));
    view.update(cx, |model, cx| {
        model.disconnect(cx);
        assert!(model.mobile.state.is_none());
        let generation = model.generation;
        model.receive((generation, Update::Connected(Vec::new())), cx);
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::ReadRemoteAccess))
    );
}
