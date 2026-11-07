//! The welcome tour: when it opens, what ends it, and the extensions and phone
//! it sets up.

use std::path::Path;
use std::time::Duration;

use gpui::point;

use muxy_protocol::{
    DeviceId, DeviceKind, ListenerStatus, PairedDevice, PairingInvite, PairingOffer,
    RemoteAccessSettings, RemoteAccessState,
};

use super::preferences::{click_preference, settings_tab};
use super::servers::work;
use super::*;
use crate::extensions::marketplace::ExtensionDetails;
use crate::model::onboarding::{OfferState, Step};

fn connect_local(model: &mut AppModel, cx: &mut Context<AppModel>) {
    model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
    acknowledge_catalog(model, cx);
}

fn open(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.show_onboarding = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    (view, cx)
}

fn step(view: &Entity<AppModel>, cx: &VisualTestContext) -> Option<Step> {
    view.read_with(cx, |model, _| {
        model.onboarding.tour.as_ref().map(|tour| tour.step)
    })
}

fn tour_is_showing(view: &Entity<AppModel>, cx: &VisualTestContext) -> bool {
    view.read_with(cx, |model, _| {
        matches!(model.overlay, Some(Overlay::Onboarding))
    })
}

#[gpui::test]
fn launch_opens_the_tour_until_it_is_completed(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(tour_is_showing(&view, cx));
    assert_eq!(step(&view, cx), Some(Step::Welcome));
}

#[gpui::test]
fn launch_stays_quiet_once_the_tour_is_completed(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.onboarding.tour.is_none());
    });
}

#[gpui::test]
fn skipping_closes_the_tour_and_remembers_it(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, AppModel::finish_onboarding);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.onboarding.tour.is_none());
        assert!(model.appearance.onboarding_completed);
        let saved =
            muxy_app_core::settings::Appearance::load(&model.path.with_file_name("settings.toml"))
                .expect("settings");
        assert!(saved.onboarding_completed);
    });
}

#[gpui::test]
fn dismissing_leaves_the_tour_open(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.onboarding_completed);
    });
}

#[gpui::test]
fn the_steps_walk_forward_and_back_and_the_last_finishes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| model.step_onboarding(false, cx));
    assert_eq!(step(&view, cx), Some(Step::Welcome));
    for expected in &Step::ALL[1..] {
        view.update(cx, |model, cx| model.step_onboarding(true, cx));
        assert_eq!(step(&view, cx), Some(*expected));
    }
    view.update(cx, |model, cx| model.step_onboarding(false, cx));
    assert_eq!(step(&view, cx), Some(Step::Mobile));
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    cx.run_until_parked();
    assert_eq!(step(&view, cx), None);
    view.read_with(cx, |model, _| {
        assert!(model.appearance.onboarding_completed);
    });
}

#[gpui::test]
fn every_step_draws_its_words_and_art(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    for target in Step::ALL {
        view.update(cx, |model, cx| model.show_onboarding_step(target, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("onboarding-title").is_some(), "{target:?}");
        assert!(cx.debug_bounds("onboarding-art").is_some(), "{target:?}");
    }
}

#[gpui::test]
fn the_next_button_moves_on_and_skip_ends_the_tour(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let next = cx.debug_bounds("onboarding-next").expect("next");
    cx.simulate_click(next.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(step(&view, cx), Some(Step::Server));
    let skip = cx.debug_bounds("onboarding-skip").expect("skip");
    cx.simulate_click(skip.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(step(&view, cx), None);
}

#[gpui::test]
fn the_tour_comes_back_after_what_opened_over_it_closes(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| {
        model.show_onboarding_step(Step::Remote, cx);
        model.add_server_from_onboarding(cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::ServerForm(_))));
    });
    view.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    assert_eq!(step(&view, cx), Some(Step::Remote));
}

fn press(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn moves_begun(view: &Entity<AppModel>, cx: &VisualTestContext) -> usize {
    view.read_with(cx, |model, _| model.window_moves_begun)
}

/// Clicks Next and then Back, leaving focus on a Back button that the first
/// step doesn't have.
fn lose_focus_to_a_button_that_is_gone(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    let next = cx.debug_bounds("onboarding-next").expect("next");
    cx.simulate_click(next.center(), Modifiers::default());
    cx.run_until_parked();
    let back = cx.debug_bounds("onboarding-back").expect("back");
    cx.simulate_click(back.center(), Modifiers::default());
    cx.run_until_parked();
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(step(view, cx), Some(Step::Welcome));
}

#[gpui::test]
fn every_empty_surface_moves_the_window(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let art = cx.debug_bounds("onboarding-art-drag").expect("art");
    press(cx, art.center());
    assert_eq!(moves_begun(&view, cx), 1);
    let header = cx.debug_bounds("onboarding-header").expect("header");
    press(cx, header.center());
    assert_eq!(moves_begun(&view, cx), 2);
    let backdrop = cx.debug_bounds("onboarding-backdrop").expect("backdrop");
    press(cx, backdrop.origin + point(px(2.0), px(2.0)));
    assert_eq!(moves_begun(&view, cx), 3);
    assert!(tour_is_showing(&view, cx));
}

#[gpui::test]
fn the_skip_button_does_not_move_the_window(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let skip = cx.debug_bounds("onboarding-skip").expect("skip");
    cx.simulate_event(gpui::MouseDownEvent {
        position: skip.center(),
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    assert_eq!(moves_begun(&view, cx), 0);
}

#[gpui::test]
fn the_window_still_moves_after_the_focused_button_is_gone(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    lose_focus_to_a_button_that_is_gone(&view, cx);
    let backdrop = cx.debug_bounds("onboarding-backdrop").expect("backdrop");
    press(cx, backdrop.origin + point(px(2.0), px(2.0)));
    assert_eq!(moves_begun(&view, cx), 1);
    let art = cx.debug_bounds("onboarding-art-drag").expect("art");
    press(cx, art.center());
    assert_eq!(moves_begun(&view, cx), 2);
}

#[gpui::test]
fn the_keyboard_still_works_after_the_focused_button_is_gone(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.show_onboarding = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    lose_focus_to_a_button_that_is_gone(&view, cx);
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), Some(Step::Server));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), None);
}

#[gpui::test]
fn the_command_palette_finds_the_tour_by_the_words_people_use(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let registry = model.command_registry(true, cx);
        assert!(registry.command("show_welcome_tour").is_some());
        for query in ["onboarding", "welcome", "tour", "getting started"] {
            assert!(!registry.items(query).is_empty(), "{query}");
        }
    });
}

#[gpui::test]
fn the_command_opens_the_tour_and_reopens_it_after_finishing(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    cx.dispatch_action(crate::views::workspace::ShowWelcomeTour);
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    view.update(cx, AppModel::finish_onboarding);
    cx.run_until_parked();
    assert_eq!(step(&view, cx), None);
    cx.dispatch_action(crate::views::workspace::ShowWelcomeTour);
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    assert_eq!(step(&view, cx), Some(Step::Welcome));
}

#[gpui::test]
fn the_window_can_be_moved_while_the_tour_is_open_but_not_under_another_modal(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx);
    view.read_with(cx, |model, _| assert!(model.window_can_move()));
    view.update(cx, |model, cx| model.open_server_form(None, cx));
    view.read_with(cx, |model, _| assert!(!model.window_can_move()));
    view.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    view.update(cx, AppModel::finish_onboarding);
    view.read_with(cx, |model, _| assert!(model.window_can_move()));
}

#[gpui::test]
fn settings_replays_the_tour(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = settings_tab(boot, cx);
    view.read_with(cx, |model, _| assert!(model.onboarding.tour.is_none()));
    click_preference(cx, "settings-welcome-tour");
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    assert_eq!(step(&view, cx), Some(Step::Welcome));
}

#[gpui::test]
fn the_tour_confirms_the_project_its_button_added(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().to_path_buf();
    view.update_in(cx, |model, window, cx| {
        model.add_project_from_onboarding(window, cx);
    });
    cx.run_until_parked();
    let name = view.update(cx, |model, cx| {
        let id = model.add_project(path, cx).expect("project");
        model.close_overlay(cx);
        model
            .state
            .projects()
            .iter()
            .find(|project| project.id == id)
            .expect("project")
            .name
            .clone()
    });
    cx.run_until_parked();
    assert!(tour_is_showing(&view, cx));
    view.read_with(cx, |model, _| {
        let tour = model.onboarding.tour.as_ref().expect("tour");
        assert_eq!(tour.added_project.as_deref(), Some(name.as_str()));
    });
}

#[gpui::test]
fn projects_that_appear_on_their_own_are_not_claimed(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().to_path_buf();
    view.update(cx, |model, cx| {
        model.add_project(path, cx).expect("project");
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let tour = model.onboarding.tour.as_ref().expect("tour");
        assert_eq!(tour.added_project, None);
    });
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "Stands in for the marketplace's fetch, which can fail"
)]
fn details(name: &str) -> Result<ExtensionDetails, String> {
    Ok(ExtensionDetails {
        name: name.to_owned(),
        description: String::new(),
        version: Some("2.0.0".into()),
        current_version: Some("2.0.0".into()),
        permissions: Some(["panels:write".to_owned()].into_iter().collect()),
        download_url: None,
        size: None,
        sha256: None,
    })
}

fn stage(details: &ExtensionDetails, directory: &Path) -> Result<tempfile::TempDir, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    write_package(&stage.path().join("package"), &details.name);
    Ok(stage)
}

fn stage_all_but_files(
    details: &ExtensionDetails,
    directory: &Path,
) -> Result<tempfile::TempDir, String> {
    if details.name == "files" {
        return Err("no network".to_owned());
    }
    stage(details, directory)
}

fn write_package(root: &Path, name: &str) {
    std::fs::create_dir_all(root).expect("directory");
    std::fs::write(
        root.join("package.json"),
        serde_json::json!({
            "name": name,
            "version": "2.0.0",
            "muxy": {"permissions": ["panels:write"]},
        })
        .to_string(),
    )
    .expect("package");
}

fn pump_until(cx: &mut VisualTestContext, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    for _ in 0..2000 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the tour's extension work did not finish");
}

/// The tour on its extensions step, with its offers loaded from a stub.
fn at_extensions<'a>(
    cx: &'a mut TestAppContext,
    download: fn(&ExtensionDetails, &Path) -> Result<tempfile::TempDir, String>,
    installed: &[&str],
) -> (Entity<AppModel>, &'a mut VisualTestContext) {
    let (view, cx) = open(cx);
    let directory = view.read_with(cx, |model, _| model.extensions.registry.directory());
    for name in installed {
        write_package(&directory.join(name), name);
    }
    view.update(cx, |model, cx| {
        model.onboarding.fetch = details;
        model.onboarding.download = download;
        model.refresh_installed_extensions(cx);
    });
    let count = installed.len();
    pump_until(cx, |cx| {
        view.read_with(cx, |model, _| {
            model.extensions_loaded() && model.extensions.registry.extensions.len() == count
        })
    });
    view.update(cx, |model, cx| {
        model.show_onboarding_step(Step::Extensions, cx);
    });
    pump_until(cx, |cx| {
        view.read_with(cx, |model, _| {
            model
                .onboarding
                .offers
                .iter()
                .all(|offer| offer.state == OfferState::Ready)
        })
    });
    (view, cx)
}

fn offer_state(view: &Entity<AppModel>, name: &str, cx: &VisualTestContext) -> OfferState {
    view.read_with(cx, |model, _| {
        model
            .onboarding
            .offers
            .iter()
            .find(|offer| offer.name == name)
            .expect("offer")
            .state
            .clone()
    })
}

fn enabled(view: &Entity<AppModel>, name: &str, cx: &VisualTestContext) -> bool {
    view.read_with(cx, |model, _| {
        model.extensions.registry.enabled(name).is_some()
    })
}

fn settled(view: &Entity<AppModel>, cx: &VisualTestContext) -> bool {
    view.read_with(cx, |model, _| !model.onboarding.installing())
}

#[gpui::test]
fn nothing_installs_until_asked(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    assert_eq!(offer_state(&view, "files", cx), OfferState::Ready);
    view.read_with(cx, |model, _| {
        assert!(model.extensions.registry.extensions.is_empty());
        assert_eq!(model.offers_to_install(), 2);
        let permissions = model.onboarding.offers[0]
            .details
            .as_ref()
            .and_then(|details| details.permissions.as_ref())
            .expect("permissions");
        assert!(permissions.contains("panels:write"));
    });
}

#[gpui::test]
fn installing_sets_up_every_ticked_offer_in_turn(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    view.update(cx, AppModel::install_offers);
    pump_until(cx, |cx| settled(&view, cx));
    for name in ["files", "git"] {
        assert_eq!(offer_state(&view, name, cx), OfferState::Done);
        assert!(enabled(&view, name, cx));
    }
    view.read_with(cx, |model, _| {
        assert_eq!(model.extensions.installs.finished, None);
        assert_eq!(model.offers_to_install(), 0);
    });
}

#[gpui::test]
fn an_unticked_offer_stays_uninstalled(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    view.update(cx, |model, cx| {
        model.toggle_offer("git", cx);
        model.install_offers(cx);
    });
    pump_until(cx, |cx| settled(&view, cx));
    assert!(enabled(&view, "files", cx));
    assert!(!enabled(&view, "git", cx));
    assert_eq!(offer_state(&view, "git", cx), OfferState::Ready);
}

#[gpui::test]
fn one_failed_install_does_not_stop_the_next(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage_all_but_files, &[]);
    view.update(cx, AppModel::install_offers);
    pump_until(cx, |cx| settled(&view, cx));
    assert_eq!(
        offer_state(&view, "files", cx),
        OfferState::Failed("no network".into())
    );
    assert!(enabled(&view, "git", cx));
    view.read_with(cx, |model, _| assert_eq!(model.offers_to_install(), 1));
}

#[gpui::test]
fn an_installed_extension_is_only_enabled(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, |_, _| Err("must not download".to_owned()), &["files"]);
    view.update(cx, |model, cx| {
        model.toggle_offer("git", cx);
        model.install_offers(cx);
    });
    pump_until(cx, |cx| settled(&view, cx));
    assert_eq!(offer_state(&view, "files", cx), OfferState::Done);
    assert!(enabled(&view, "files", cx));
}

#[gpui::test]
fn installs_finish_after_the_tour_has_closed(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    view.update(cx, |model, cx| {
        model.install_offers(cx);
        model.finish_onboarding(cx);
    });
    pump_until(cx, |cx| settled(&view, cx));
    assert!(enabled(&view, "files", cx));
    assert!(enabled(&view, "git", cx));
    view.read_with(cx, |model, _| assert!(model.onboarding.tour.is_none()));
}

fn access(enabled: bool, devices: Vec<PairedDevice>) -> RemoteAccessState {
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
        pairing_expires_at: None,
        devices,
    }
}

fn phone(id: DeviceId) -> PairedDevice {
    PairedDevice {
        id,
        name: "Saeed's iPhone".into(),
        paired_at: 1,
        last_seen: None,
        connected: true,
        kind: DeviceKind::Phone,
    }
}

fn offer() -> PairingOffer {
    PairingOffer {
        invite: PairingInvite {
            hosts: vec!["192.168.1.5".into()],
            port: 7419,
            fingerprint: [1; 32],
            secret: [2; 16],
        },
        expires_at: u64::MAX / 2,
    }
}

/// The tour on its mobile step, with device access read as `state`.
fn at_mobile(
    cx: &mut TestAppContext,
    state: RemoteAccessState,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let (mut boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.show_onboarding = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, connect_local);
    work(&requests);
    view.update(cx, |model, cx| model.show_onboarding_step(Step::Mobile, cx));
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess)),
        "the mobile step reads device access"
    );
    view.update(cx, |model, cx| model.receive_remote_access(Ok(state), cx));
    cx.run_until_parked();
    (view, cx, requests)
}

fn wrote_access(requests: &std::sync::mpsc::Receiver<(u64, Work)>, enabled: bool) -> bool {
    work(requests).iter().any(
        |work| matches!(work, Work::WriteRemoteAccess(settings) if settings.enabled == enabled),
    )
}

#[gpui::test]
fn opening_the_mobile_step_changes_nothing_on_the_server(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    assert!(work(&requests).is_empty());
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_none());
    });
}

#[gpui::test]
fn pairing_turns_access_on_then_shows_a_code(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    assert!(wrote_access(&requests, true));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::StartPairing))
    );
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_some());
    });
}

#[gpui::test]
fn leaving_unpaired_withdraws_the_code_and_turns_access_back_off(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    work(&requests);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
        model.receive_pairing(Ok(offer()), cx);
    });
    work(&requests);
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_none());
    });
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(wrote_access(&requests, false));
}

#[gpui::test]
fn access_that_was_already_on_stays_on_after_leaving(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(true, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    work(&requests);
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    work(&requests);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(!wrote_access(&requests, false));
}

#[gpui::test]
fn the_phone_that_pairs_is_named_and_keeps_access_on(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
        model.receive_pairing(Ok(offer()), cx);
    });
    work(&requests);
    let device = DeviceId::from_u128(7);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, vec![phone(device)])), cx);
    });
    view.read_with(cx, |model, _| {
        let tour = model.onboarding.tour.as_ref().expect("tour");
        assert_eq!(tour.pair.paired.as_deref(), Some("Saeed's iPhone"));
        assert!(model.client_access.pairing.is_none());
    });
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    assert!(!wrote_access(&requests, false));
}

#[gpui::test]
fn a_phone_already_paired_is_not_taken_for_a_new_one(cx: &mut TestAppContext) {
    let device = DeviceId::from_u128(7);
    let (view, cx, _requests) = at_mobile(cx, access(true, vec![phone(device)]));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, vec![phone(device)])), cx);
    });
    view.read_with(cx, |model, _| {
        let tour = model.onboarding.tour.as_ref().expect("tour");
        assert_eq!(tour.pair.paired, None);
    });
}

#[gpui::test]
fn a_code_answered_after_leaving_the_step_is_thrown_away(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(true, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    work(&requests);
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    view.read_with(cx, |model, _| {
        assert!(model.client_access.pairing.is_none());
    });
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::CancelPairing))
    );
}

fn bind_default_keys(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&muxy_app_core::settings::Keymap::default(), cx);
    });
}

fn keyboard_in_tour(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> bool {
    view.update_in(cx, |model, window, cx| {
        model.overlay_focus.contains_focused(window, cx)
    })
}

fn default_key(id: muxy_core::shortcuts::ShortcutId) -> &'static str {
    id.definition().keys[0]
}

#[gpui::test]
fn a_pending_window_move_survives_the_art_redrawing(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let art = cx.debug_bounds("onboarding-art-drag").expect("art");
    press(cx, art.center());
    let before = view.read_with(cx, |model, _| model.window_move_cancels);
    for _ in 0..3 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(
        view.read_with(cx, |model, _| model.window_move_cancels),
        before
    );
    view.update(cx, |model, cx| model.open_server_form(None, cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.window_move_cancels) > before);
}

#[gpui::test]
fn shortcuts_that_focus_the_pane_leave_the_keyboard_in_the_tour(cx: &mut TestAppContext) {
    bind_default_keys(cx);
    let (view, cx) = open(cx);
    for id in [
        muxy_core::shortcuts::ShortcutId::ToggleSidebar,
        muxy_core::shortcuts::ShortcutId::OpenSettings,
    ] {
        cx.simulate_keystrokes(default_key(id));
        cx.run_until_parked();
        assert!(tour_is_showing(&view, cx), "{id:?}");
        assert!(keyboard_in_tour(&view, cx), "{id:?}");
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), None);
}

#[gpui::test]
fn the_keyboard_still_works_after_install_disables_its_button(cx: &mut TestAppContext) {
    bind_default_keys(cx);
    let (view, cx) = at_extensions(cx, stage, &[]);
    cx.run_until_parked();
    let button = cx.debug_bounds("onboarding-install").expect("install");
    cx.simulate_click(button.center(), Modifiers::default());
    pump_until(cx, |cx| settled(&view, cx));
    cx.run_until_parked();
    assert!(keyboard_in_tour(&view, cx));
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), Some(Step::Agents));
}

#[gpui::test]
fn the_keyboard_still_works_after_cancelling_a_code(cx: &mut TestAppContext) {
    bind_default_keys(cx);
    let (view, cx, requests) = at_mobile(cx, access(true, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    cx.run_until_parked();
    work(&requests);
    let cancel = cx
        .debug_bounds("onboarding-cancel-pairing")
        .expect("cancel");
    cx.simulate_click(cancel.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(keyboard_in_tour(&view, cx));
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), Some(Step::Shortcuts));
}

#[gpui::test]
fn right_on_the_last_step_does_not_finish_the_tour(cx: &mut TestAppContext) {
    bind_default_keys(cx);
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| {
        model.show_onboarding_step(Step::Shortcuts, cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    assert_eq!(step(&view, cx), Some(Step::Shortcuts));
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.onboarding_completed);
    });
}

#[gpui::test]
fn quitting_on_the_mobile_step_turns_access_back_off(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
        model.receive_pairing(Ok(offer()), cx);
    });
    work(&requests);
    view.update(cx, AppModel::quit);
    assert!(wrote_access(&requests, false));
}

#[gpui::test]
fn quitting_leaves_access_on_when_the_tour_did_not_turn_it_on(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(true, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    work(&requests);
    view.update(cx, AppModel::quit);
    assert!(!wrote_access(&requests, false));
}

#[gpui::test]
fn a_refused_access_change_stops_and_shows_its_error(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    assert!(wrote_access(&requests, true));
    view.update(cx, |model, cx| {
        model.receive_remote_access(
            Err(muxy_client::ClientError::Protocol("could not save".into())),
            cx,
        );
    });
    assert!(!wrote_access(&requests, true));
    view.read_with(cx, |model, _| {
        assert!(model.client_access.error.is_some());
        assert!(!model.client_access.busy);
        let tour = model.onboarding.tour.as_ref().expect("tour");
        assert!(!tour.pair.wanted);
    });
}

#[gpui::test]
fn leaving_withdraws_the_code_even_with_the_servers_page_open(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(true, Vec::new()));
    view.update_in(cx, AppModel::show_servers_settings);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    work(&requests);
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| model.receive_pairing(Ok(offer()), cx));
    work(&requests);
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
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
fn turning_access_back_off_waits_for_a_lost_server_to_return(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
        model.receive_pairing(Ok(offer()), cx);
    });
    work(&requests);
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
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    view.read_with(cx, |model, _| assert!(model.client_access.restore_off));
    work(&requests);
    view.update(cx, |model, cx| {
        let server = ServerId::local();
        model.connect_server(server, cx);
        let generation = model.generation(server);
        model.receive((server, generation, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });
    cx.run_until_parked();
    assert!(
        work(&requests)
            .iter()
            .any(|work| matches!(work, Work::ReadRemoteAccess))
    );
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(wrote_access(&requests, false));
    view.read_with(cx, |model, _| assert!(model.client_access.restore_off));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(false, Vec::new())), cx);
    });
    view.read_with(cx, |model, _| assert!(!model.client_access.restore_off));
}

#[gpui::test]
fn a_refused_turn_off_is_kept_and_retried_on_the_next_good_answer(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
        model.receive_pairing(Ok(offer()), cx);
    });
    work(&requests);
    view.update(cx, |model, cx| model.step_onboarding(true, cx));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(wrote_access(&requests, false));
    view.update(cx, |model, cx| {
        model.receive_remote_access(
            Err(muxy_client::ClientError::Protocol("could not save".into())),
            cx,
        );
    });
    assert!(!wrote_access(&requests, false));
    view.read_with(cx, |model, _| assert!(model.client_access.restore_off));
    view.update(cx, |model, cx| {
        model.receive_remote_access(Ok(access(true, Vec::new())), cx);
    });
    assert!(wrote_access(&requests, false));
}

#[gpui::test]
fn quitting_while_turning_access_on_still_turns_it_back_off(cx: &mut TestAppContext) {
    let (view, cx, requests) = at_mobile(cx, access(false, Vec::new()));
    view.update(cx, AppModel::pair_phone_in_onboarding);
    assert!(wrote_access(&requests, true));
    view.update(cx, AppModel::quit);
    assert!(wrote_access(&requests, false));
}

fn details_of_another_extension(name: &str) -> Result<ExtensionDetails, String> {
    details(&format!("not-{name}"))
}

#[gpui::test]
fn an_answer_about_another_extension_is_not_installed(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| {
        model.onboarding.fetch = details_of_another_extension;
        model.onboarding.download = stage;
        model.show_onboarding_step(Step::Extensions, cx);
    });
    pump_until(cx, |cx| {
        view.read_with(cx, |model, _| {
            model
                .onboarding
                .offers
                .iter()
                .all(|offer| matches!(offer.state, OfferState::Unavailable(_)))
        })
    });
    view.read_with(cx, |model, _| assert_eq!(model.offers_to_install(), 0));
}

#[gpui::test]
fn an_offer_done_earlier_can_be_set_up_again_after_being_disabled(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    view.update(cx, AppModel::install_offers);
    pump_until(cx, |cx| settled(&view, cx));
    view.update(cx, |model, cx| {
        model.set_extension_enabled("files", false, cx).detach();
    });
    pump_until(cx, |cx| !enabled(&view, "files", cx));
    view.read_with(cx, |model, _| assert_eq!(model.offers_to_install(), 1));
    view.update(cx, AppModel::install_offers);
    pump_until(cx, |cx| enabled(&view, "files", cx));
}

#[gpui::test]
fn replaying_the_tour_sees_extensions_as_they_are_now(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &[]);
    view.update(cx, AppModel::install_offers);
    pump_until(cx, |cx| settled(&view, cx));
    view.update(cx, AppModel::finish_onboarding);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.set_extension_enabled("files", false, cx).detach();
    });
    pump_until(cx, |cx| !enabled(&view, "files", cx));
    view.update(cx, AppModel::open_onboarding);
    view.update(cx, |model, cx| {
        model.show_onboarding_step(Step::Extensions, cx);
    });
    cx.run_until_parked();
    assert_eq!(offer_state(&view, "files", cx), OfferState::Ready);
    view.read_with(cx, |model, _| assert_eq!(model.offers_to_install(), 1));
}

#[gpui::test]
fn a_failed_install_after_closing_is_not_called_ready(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage_all_but_files, &[]);
    view.update(cx, |model, cx| {
        model.install_offers(cx);
        model.finish_onboarding(cx);
    });
    pump_until(cx, |cx| settled(&view, cx));
    view.read_with(cx, |model, _| {
        assert!(
            model
                .error
                .as_deref()
                .is_some_and(|error| error.contains("no network"))
        );
        assert!(model.notice.is_none());
    });
}

#[gpui::test]
fn another_jobs_result_is_told_before_the_next_offer_starts(cx: &mut TestAppContext) {
    let (view, cx) = at_extensions(cx, stage, &["reader"]);
    view.update(cx, |model, cx| {
        assert!(model.update_all_extensions_with(
            |_| Err("offline".to_owned()),
            |_, _| Err("unused".to_owned()),
            cx,
        ));
        model.install_offers(cx);
    });
    pump_until(cx, |cx| settled(&view, cx));
    assert!(enabled(&view, "files", cx));
    view.read_with(cx, |model, _| {
        assert_eq!(model.error.as_deref(), Some("offline"));
    });
}

#[gpui::test]
fn the_composer_step_shows_the_shortcuts_of_this_keymap(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |model, cx| {
        model.show_onboarding_step(Step::Composer, cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("onboarding-art").is_some());
    view.read_with(cx, |model, _| {
        for id in [
            muxy_core::shortcuts::ShortcutId::ToggleComposer,
            muxy_core::shortcuts::ShortcutId::ComposerSubmit,
        ] {
            assert!(
                !crate::views::onboarding::art::key_caps(id, model).is_empty(),
                "{id:?}"
            );
        }
    });
}
