//! This computer's access for phones and other computers, as the Devices tab
//! shows and changes it.

use muxy_protocol::{
    ListenerStatus, PairedDevice, PairingInvite, PairingOffer, RemoteAccessSettings,
    RemoteAccessState,
};

use super::tab::{open_tab, tab_window};
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

/// A window with the Devices tab open, its first read answered with `state`.
fn devices_tab(
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
        "opening the tab reads device access"
    );
    view.update(cx, |model, cx| model.receive_remote_access(Ok(state), cx));
    cx.run_until_parked();
    (view, cx, requests)
}

fn close_devices_tab(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    view.update(cx, |model, cx| {
        let (_, tab) = model.state.servers_tab().expect("devices tab");
        model.close_tab(tab, cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn the_tab_shows_access_and_its_port(cx: &mut TestAppContext) {
    let (view, cx, _) = devices_tab(cx, access(true, None, Vec::new()));
    view.read_with(cx, |model, cx| {
        assert_eq!(
            model
                .servers_tab
                .port_input()
                .map(|input| input.read(cx).text().to_owned()),
            Some("7419".into())
        );
    });
    for selector in [
        "devices-this-computer",
        "devices-access-row",
        "devices-status",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} is on screen"
        );
    }
}

#[gpui::test]
fn access_and_port_changes_go_to_the_server_or_say_why_not(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(false, None, Vec::new()));
    view.update(cx, |model, cx| model.set_device_port("80", cx));
    view.read_with(cx, |model, _| {
        assert!(model.device_access().error.is_some());
    });
    assert!(work(&requests).is_empty(), "an invalid port isn't sent");
    view.update(cx, |model, cx| model.set_device_access(true, cx));
    assert!(work(&requests).iter().any(|work| matches!(
        work,
        Work::WriteRemoteAccess(RemoteAccessSettings {
            enabled: true,
            port: 7419
        })
    )));
    view.read_with(cx, |model, _| {
        assert!(model.device_access().error.is_none());
    });
}

#[gpui::test]
fn device_changes_refresh_only_while_the_tab_is_open(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::remote_access_changed);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    close_devices_tab(&view, cx);
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
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        model.read_remote_access(cx);
        assert!(model.mobile.busy);
        model.cancel_pairing(cx);
        assert!(model.mobile.pairing.is_none());
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
fn closing_the_tab_withdraws_a_code_still_on_screen(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    assert!(cx.debug_bounds("devices-code").is_some());
    close_devices_tab(&view, cx);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
    view.read_with(cx, |model, _| assert!(model.mobile.pairing.is_none()));
}

#[gpui::test]
fn closing_the_tab_withdraws_a_pending_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    view.update(cx, AppModel::show_pairing_code);
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::StartPairing))
    );
    close_devices_tab(&view, cx);
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
        assert!(model.mobile.pairing.is_none());
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
}

#[gpui::test]
fn a_tab_saved_under_its_old_title_takes_the_new_one(cx: &mut TestAppContext) {
    let (view, cx, _, _) = tab_window(Vec::new(), cx);
    view.update(cx, |model, _| {
        let project = model.state.current_project().id;
        model
            .state
            .show_servers_tab(project, "Remote Servers")
            .expect("old tab");
    });
    open_tab(&view, cx);
    view.read_with(cx, |model, _| {
        let (project, tab) = model.state.servers_tab().expect("devices tab");
        let title = model
            .state
            .project(project)
            .and_then(|project| project.tabs.iter().find(|listed| listed.id == tab))
            .and_then(|tab| tab.panes.first())
            .map(|pane| pane.title.clone());
        assert_eq!(title.as_deref(), Some("Devices"));
    });
}

/// Chooses Copy My Pair Link; the Devices tab reads access again as it
/// opens, and the server answers with `state`.
fn copy_link(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
    state: RemoteAccessState,
) {
    cx.update(|window, cx| {
        view.update(cx, |model, cx| model.copy_pairing_link(window, cx));
    });
    assert!(
        work(requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess)),
        "the tab reads device access"
    );
    view.update(cx, |model, cx| model.receive_remote_access(Ok(state), cx));
    cx.run_until_parked();
}

fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

fn asked_for_code(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> bool {
    work(requests)
        .iter()
        .any(|work| matches!(work, Work::StartPairing))
}

#[gpui::test]
fn copying_the_link_uses_the_code_on_screen(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    let code = offer(now() + 300);
    view.update(cx, |model, cx| model.receive_pairing(Ok(code.clone()), cx));
    work(&requests);
    copy_link(
        &view,
        cx,
        &requests,
        access(true, Some(code.expires_at), Vec::new()),
    );
    assert_eq!(clipboard(cx), Some(code.invite.to_link()));
    assert!(!asked_for_code(&requests), "the code on screen is reused");
}

/// Copy My Pair Link asks for a code, shows it, and copies its link once it
/// arrives, whether or not the Devices tab was open.
fn copies_a_new_code(cx: &mut TestAppContext, open: bool) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    if !open {
        close_devices_tab(&view, cx);
        work(&requests);
    }
    copy_link(&view, cx, &requests, access(true, None, Vec::new()));
    view.read_with(cx, |model, _| assert!(model.state.servers_tab().is_some()));
    assert!(asked_for_code(&requests));
    assert_eq!(
        clipboard(cx),
        None,
        "nothing to copy until the code arrives"
    );
    let code = offer(now() + 300);
    view.update(cx, |model, cx| model.receive_pairing(Ok(code.clone()), cx));
    cx.run_until_parked();
    assert_eq!(clipboard(cx), Some(code.invite.to_link()));
    assert!(cx.debug_bounds("devices-code").is_some(), "the code shows");
}

#[gpui::test]
fn copying_the_link_shows_a_code_and_copies_it_once_it_arrives(cx: &mut TestAppContext) {
    copies_a_new_code(cx, true);
}

#[gpui::test]
fn copying_the_link_opens_the_devices_tab_first(cx: &mut TestAppContext) {
    copies_a_new_code(cx, false);
}

#[gpui::test]
fn copying_the_link_with_access_off_says_why_not(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(false, None, Vec::new()));
    copy_link(&view, cx, &requests, access(false, None, Vec::new()));
    assert!(!asked_for_code(&requests));
    assert_eq!(clipboard(cx), None);
    view.read_with(cx, |model, _| {
        assert!(model.device_access().error.is_some());
    });
}

#[gpui::test]
fn a_code_cancelled_before_it_arrives_is_not_copied(cx: &mut TestAppContext) {
    let (view, cx, requests) = devices_tab(cx, access(true, None, Vec::new()));
    copy_link(&view, cx, &requests, access(true, None, Vec::new()));
    assert!(asked_for_code(&requests));
    view.update(cx, AppModel::cancel_pairing);
    view.update(cx, |model, cx| {
        model.receive_pairing(Ok(offer(now() + 300)), cx);
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, None, Vec::new())), cx);
    });
    assert!(!asked_for_code(&requests), "no new code after cancelling");
    assert_eq!(clipboard(cx), None);
}
