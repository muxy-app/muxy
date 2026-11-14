//! This computer's access for phones and other computers, as the Servers page
//! shows and changes it.

use muxy_protocol::{
    ListenerStatus, PairedDevice, PairingInvite, PairingOffer, RemoteAccessSettings,
    RemoteAccessState,
};

use super::groups::press;
use super::tab::open_tab;
use super::*;

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
        push_notifications: true,
        push_kept_until: None,
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

/// A window with the Servers page open, its first read answered with `state`.
fn servers_tab(
    cx: &mut TestAppContext,
    state: RemoteAccessState,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let (boot, requests, _) = remote_boot(AppState::bootstrap().expect("state"), Vec::new());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, connect_local);
    work(&requests);
    open_tab(&view, cx);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess)),
        "opening the page reads device access"
    );
    view.update(cx, |model, cx| model.receive_remote_access(Ok(state), cx));
    cx.run_until_parked();
    (view, cx, requests)
}

fn close_servers_tab(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    view.update(cx, |model, cx| {
        let (_, tab) = model.state.settings_tab().expect("settings tab");
        model.close_tab(tab, cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn the_page_shows_access_and_its_port(cx: &mut TestAppContext) {
    let (view, cx, _) = servers_tab(cx, access(true, None, Vec::new()));
    view.read_with(cx, |model, cx| {
        assert_eq!(
            model
                .servers_page
                .port_input()
                .map(|input| input.read(cx).text().to_owned()),
            Some("7419".into())
        );
    });
    for selector in [
        "clients-this-computer",
        "clients-access-row",
        "clients-status",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} is on screen"
        );
    }
}

#[gpui::test]
fn the_page_holds_this_computers_server_and_pairs_only_once_clients_are_allowed(
    cx: &mut TestAppContext,
) {
    let (view, cx, _) = servers_tab(cx, access(false, None, Vec::new()));
    for selector in ["settings-row-server", "clients-access-row", "servers-add"] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} is on screen"
        );
    }
    for selector in [
        "clients-status-row",
        "clients-pair-row",
        "clients-paired-row",
    ] {
        assert!(
            cx.debug_bounds(selector).is_none(),
            "{selector} waits until clients are allowed"
        );
    }
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    cx.run_until_parked();
    for selector in [
        "clients-status-row",
        "clients-pair-row",
        "clients-paired-row",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} is on screen"
        );
    }
}

#[gpui::test]
fn access_and_port_changes_go_to_the_server_or_say_why_not(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(false, None, Vec::new()));
    view.update(cx, |model, cx| model.set_client_port("80", cx));
    view.read_with(cx, |model, _| {
        assert!(model.client_access().error.is_some());
    });
    assert!(work(&requests).is_empty(), "an invalid port isn't sent");
    view.update(cx, |model, cx| model.set_client_access(true, cx));
    assert!(work(&requests).iter().any(|work| matches!(
        work,
        Work::WriteRemoteAccess(RemoteAccessSettings {
            enabled: true,
            port: 7419
        })
    )));
    view.read_with(cx, |model, _| {
        assert!(model.client_access().error.is_none());
    });
}

fn phone_with_push(push: muxy_protocol::PushStatus) -> PairedDevice {
    PairedDevice {
        id: muxy_protocol::DeviceId::from_u128(5),
        name: "Phone".into(),
        paired_at: 1,
        last_seen: None,
        connected: false,
        kind: muxy_protocol::DeviceKind::Phone,
        push: Some(push),
    }
}

fn with_push(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    view.update(cx, |model, cx| {
        model.servers.local.features = vec![muxy_protocol::Feature::PUSH_NOTIFICATIONS];
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn the_push_switch_shows_for_servers_that_have_it_and_goes_to_the_server(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    assert!(cx.debug_bounds("clients-push-row").is_none());
    with_push(&view, cx);
    assert!(cx.debug_bounds("clients-push-row").is_some());
    view.update_in(cx, |model, window, cx| {
        model.set_push_notifications(true, window.window_handle(), cx);
    });
    assert!(work(&requests).is_empty(), "push is already on");
    view.update_in(cx, |model, window, cx| {
        model.set_push_notifications(false, window.window_handle(), cx);
    });
    assert!(
        !cx.has_pending_prompt(),
        "no phone is affected, so nothing to confirm"
    );
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::SetPushNotifications(false)))
    );
    view.update(cx, |model, cx| {
        assert!(model.client_access.busy);
        model.switch_push_notifications(true, cx);
        assert!(model.client_access().error.is_some());
        let mut off = access(true, None, Vec::new());
        off.push_notifications = false;
        model.receive_remote_access(Ok(off), cx);
        assert!(!model.client_access.busy);
    });
}

#[gpui::test]
fn pausing_phones_that_get_notifications_asks_first_and_says_what_is_kept(cx: &mut TestAppContext) {
    let phone = phone_with_push(muxy_protocol::PushStatus::Active);
    let (view, cx, requests) = servers_tab(cx, access(true, None, vec![phone]));
    with_push(&view, cx);
    view.update_in(cx, |model, window, cx| {
        model.set_push_notifications(false, window.window_handle(), cx);
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(work(&requests).is_empty(), "cancelling keeps push on");
    view.update_in(cx, |model, window, cx| {
        model.set_push_notifications(false, window.window_handle(), cx);
    });
    cx.run_until_parked();
    cx.simulate_prompt_answer("Pause");
    cx.run_until_parked();
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::SetPushNotifications(false)))
    );
    let mut paused = access(
        true,
        None,
        vec![phone_with_push(muxy_protocol::PushStatus::Disabled)],
    );
    paused.push_notifications = false;
    paused.push_kept_until = Some(1_000 + 90 * 86_400);
    assert_eq!(
        crate::views::clients::push_description(&paused, 1_000).to_string(),
        "Off. Nothing is sent to muxy.app. Paused phones resume when you turn this on, for 90 more days."
    );
    view.update(cx, |model, cx| model.receive_remote_access(Ok(paused), cx));
    view.update_in(cx, |model, window, cx| {
        model.set_push_notifications(true, window.window_handle(), cx);
    });
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt(), "turning push on never asks");
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::SetPushNotifications(true)))
    );
}

#[gpui::test]
fn resetting_a_phones_notifications_asks_and_goes_to_the_server(cx: &mut TestAppContext) {
    let phone = phone_with_push(muxy_protocol::PushStatus::Active);
    let id = phone.id;
    let (view, cx, requests) = servers_tab(cx, access(true, None, vec![phone]));
    with_push(&view, cx);
    view.update_in(cx, |model, window, cx| {
        model.confirm_reset_push(id, window.window_handle(), cx);
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Reset");
    cx.run_until_parked();
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ResetPush(device) if *device == id))
    );
}

#[gpui::test]
fn device_changes_refresh_only_while_the_page_is_open(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::remote_access_changed);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    close_servers_tab(&view, cx);
    work(&requests);
    view.update(cx, AppModel::remote_access_changed);
    assert!(
        !work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess))
    );
}

#[gpui::test]
fn a_cancel_during_another_request_still_reaches_the_server(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        model.read_remote_access(cx);
        assert!(model.client_access.busy);
        model.cancel_pairing(cx);
        assert!(model.client_access.pairing.is_none());
    });
    assert!(
        work(&requests)
            .iter()
            .all(|work| !matches!(work, Work::CancelPairing))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Some(now() + 300), Vec::new())), cx);
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
}

#[gpui::test]
fn closing_settings_withdraws_a_code_still_on_screen(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    assert!(cx.debug_bounds("clients-code").is_some());
    close_servers_tab(&view, cx);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_none());
    });
}

#[gpui::test]
fn closing_settings_withdraws_a_pending_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::show_pairing_code);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::StartPairing))
    );
    close_servers_tab(&view, cx);
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        assert!(model.client_access.pairing.is_none());
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
}

#[gpui::test]
fn leaving_the_page_withdraws_a_shown_code_and_coming_back_reads_access(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    work(&requests);
    press(cx, "settings-category-General".into());
    assert!(cancels_code(&sent(&requests)), "the code was only shown");
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    work(&requests);
    press(cx, "settings-category-Servers".into());
    assert!(reads_access(&sent(&requests)));
}

#[gpui::test]
fn searching_hides_the_page_and_withdraws_a_shown_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    work(&requests);
    press(cx, "settings-search".into());
    cx.simulate_input("theme");
    cx.run_until_parked();
    assert!(cancels_code(&sent(&requests)), "the code is off screen");
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_none());
    });
}

#[gpui::test]
fn the_page_reads_access_again_when_its_server_returns(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Event(ClientEvent::Disconnected),
            ),
            cx,
        );
    });
    cx.run_until_parked();
    work(&requests);
    view.update(cx, connect_local);
    assert!(
        reads_access(&sent(&requests)),
        "the page is still on screen"
    );
}

#[gpui::test]
fn switching_tabs_withdraws_a_shown_code_and_coming_back_reads_access(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    let (settings, terminal) = view.update(cx, |model, cx| {
        let (_, settings) = model.state.settings_tab().expect("settings tab");
        let home = model.state.home().id;
        let terminal = model.state.open_terminal_tab(home).expect("terminal");
        model.select_tab(settings, cx);
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        (settings, terminal)
    });
    cx.run_until_parked();
    work(&requests);
    view.update(cx, |model, cx| model.select_tab(terminal, cx));
    cx.run_until_parked();
    assert!(cancels_code(&sent(&requests)), "the code is off screen");
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    work(&requests);
    view.update(cx, |model, cx| model.select_tab(settings, cx));
    cx.run_until_parked();
    assert!(reads_access(&sent(&requests)), "the list may have changed");
}

#[gpui::test]
fn focusing_a_split_beside_settings_keeps_a_shown_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        let pane = model.active_pane().expect("settings pane");
        model
            .state
            .split_pane(pane, Direction::Right)
            .expect("split");
        assert!(
            !model
                .active_pane()
                .is_some_and(|pane| model.state.is_settings_pane(pane))
        );
        model.sync_servers_page(false, cx);
    });
    cx.run_until_parked();
    assert!(
        !cancels_code(&sent(&requests)),
        "the code is still on screen"
    );
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_some());
    });
}

/// A window connected to this computer's server, with no Devices page.
fn window_without_tab(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let (boot, requests, _) = remote_boot(AppState::bootstrap().expect("state"), Vec::new());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, connect_local);
    work(&requests);
    (view, cx, requests)
}

fn copy_link(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    view.update(cx, AppModel::copy_pairing_link);
    cx.run_until_parked();
}

fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

fn sent(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> Vec<Work> {
    work(requests)
}

fn asks_for_code(work: &[Work]) -> bool {
    work.iter().any(|work| matches!(work, Work::StartPairing))
}

fn reads_access(work: &[Work]) -> bool {
    work.iter()
        .any(|work| matches!(work, Work::ReadRemoteAccess))
}

fn cancels_code(work: &[Work]) -> bool {
    work.iter().any(|work| matches!(work, Work::CancelPairing))
}

#[gpui::test]
fn copying_the_link_uses_the_code_on_screen(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    let code = offer(now() + 300);
    view.update(cx, |model, cx| model.receive_pairing(Ok(code.clone()), cx));
    work(&requests);
    copy_link(&view, cx);
    assert_eq!(clipboard(cx), Some(code.invite.to_link()));
    let work = sent(&requests);
    assert!(!asks_for_code(&work) && !reads_access(&work), "{work:?}");
    view.read_with(cx, |model, _| {
        assert_eq!(model.notice.as_deref(), Some("Pairing link copied"));
    });
}

#[gpui::test]
fn copying_the_link_away_from_the_page_copies_in_place(cx: &mut TestAppContext) {
    let (view, cx, requests) = window_without_tab(cx);
    copy_link(&view, cx);
    assert!(
        reads_access(&sent(&requests)),
        "a code may have changed unseen"
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    assert!(asks_for_code(&sent(&requests)));
    let code = offer(now() + 300);
    view.update(cx, |model, cx| model.receive_pairing(Ok(code.clone()), cx));
    cx.run_until_parked();
    assert_eq!(clipboard(cx), Some(code.invite.to_link()));
    view.update(cx, |model, cx| model.sync_servers_page(false, cx));
    view.read_with(cx, |model, _| {
        assert!(model.state.settings_tab().is_none(), "no tab opens");
        assert_eq!(model.notice.as_deref(), Some("Pairing link copied"));
    });
    assert!(!cancels_code(&sent(&requests)), "a copied code stays valid");
}

#[gpui::test]
fn a_copied_code_outlives_the_page_but_a_shown_one_does_not(cx: &mut TestAppContext) {
    for copied in [true, false] {
        let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
        view.update(cx, |model, cx| {
            model.receive_pairing(Ok(offer(now() + 300)), cx);
        });
        if copied {
            copy_link(&view, cx);
        }
        work(&requests);
        close_servers_tab(&view, cx);
        assert_eq!(cancels_code(&sent(&requests)), !copied, "copied: {copied}");
    }
}

#[gpui::test]
fn copying_again_replaces_a_code_used_meanwhile(cx: &mut TestAppContext) {
    let (view, cx, requests) = window_without_tab(cx);
    for expires_at in [now() + 300, now() + 301] {
        copy_link(&view, cx);
        assert!(reads_access(&sent(&requests)));
        view.update(cx, |model, cx| {
            model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
        });
        assert!(
            asks_for_code(&sent(&requests)),
            "the used code isn't copied"
        );
        let code = offer(expires_at);
        view.update(cx, |model, cx| model.receive_pairing(Ok(code.clone()), cx));
        assert_eq!(clipboard(cx), Some(code.invite.to_link()));
    }
}

#[gpui::test]
fn copying_the_link_with_access_off_says_why_not(cx: &mut TestAppContext) {
    let (view, cx, requests) = window_without_tab(cx);
    copy_link(&view, cx);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(false, None, Vec::new())), cx);
    });
    assert!(!asks_for_code(&sent(&requests)));
    assert_eq!(clipboard(cx), None);
    view.read_with(cx, |model, _| {
        assert!(model.error.is_some(), "the reason shows as a toast");
        assert!(model.state.settings_tab().is_none());
    });
}

#[gpui::test]
fn a_code_cancelled_before_it_arrives_is_not_copied(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    copy_link(&view, cx);
    assert!(asks_for_code(&sent(&requests)));
    view.update(cx, AppModel::cancel_pairing);
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    assert!(cancels_code(&sent(&requests)));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    assert!(
        !asks_for_code(&sent(&requests)),
        "no new code after cancelling"
    );
    assert_eq!(clipboard(cx), None);
}

#[gpui::test]
fn a_refresh_keeps_a_port_edit_until_it_is_saved(cx: &mut TestAppContext) {
    let (view, cx, requests) = servers_tab(cx, access(true, None, Vec::new()));
    let input = view.read_with(cx, |model, _| {
        model
            .servers_page
            .port_input()
            .cloned()
            .expect("port field")
    });
    let port = |cx: &mut VisualTestContext| input.read_with(cx, |input, _| input.text().to_owned());
    let reported = |port: u16| {
        let mut state = access(true, None, Vec::new());
        state.settings.port = port;
        state
    };
    input.update(cx, |input, cx| {
        input.select_all_text(cx);
        input.replace_selection("7420", cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(reported(7419)), cx);
    });
    assert_eq!(port(cx), "7420", "a device reconnecting keeps the edit");
    view.update(cx, |model, cx| model.set_client_port("7420", cx));
    assert!(work(&requests).iter().any(|work| matches!(
        work,
        Work::WriteRemoteAccess(RemoteAccessSettings { port: 7420, .. })
    )));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(reported(7420)), cx);
    });
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(reported(7421)), cx);
    });
    assert_eq!(
        port(cx),
        "7421",
        "once saved, the field follows the server again"
    );
}
