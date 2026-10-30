//! Computers paired with a link are listed, connect with their token, and
//! never list twice.

use muxy_app_core::settings::PairingTokens;
use muxy_protocol::{DeviceCredential, DeviceId, Paired, PairingInvite, ServerIdentity};

use super::*;
use crate::model::PairingForm;
use crate::model::pairing::parse_link;
use crate::views::overlays::Overlay;

fn invite(fingerprint: u8) -> PairingInvite {
    PairingInvite {
        hosts: vec!["192.168.1.5".into(), "studio.local".into()],
        port: 7419,
        fingerprint: [fingerprint; 32],
        secret: [2; 16],
    }
}

fn paired(token: u8) -> Paired {
    Paired {
        credential: DeviceCredential {
            device: DeviceId::from_u128(u128::from(token)),
            token: [token; 32],
        },
        server: ServerIdentity::from_u128(5),
        name: "Studio".into(),
    }
}

fn credential(model: &AppModel, server: ServerId) -> Option<DeviceCredential> {
    match &model.servers.get(server)?.target {
        Some(crate::boot::Target::Paired { credential, .. }) => Some(*credential),
        _ => None,
    }
}

#[gpui::test]
fn a_paired_computer_is_listed_and_pairing_again_updates_it(cx: &mut TestAppContext) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let path = boot.state_path.with_file_name("settings.toml");
    let tokens = boot.state_path.with_file_name("paired-servers.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let form = PairingForm {
            group: "Office".into(),
            ..PairingForm::default()
        };
        let studio = model
            .save_paired_server(None, &form, &invite(1), &paired(3), cx)
            .expect("paired");
        let saved = listed(&path);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            (saved[0].name.as_str(), saved[0].group.as_deref()),
            ("Studio", Some("Office"))
        );
        assert_eq!(
            saved[0].paired.as_ref().map(|paired| paired.hosts.clone()),
            Some(invite(1).hosts)
        );
        assert_eq!(
            PairingTokens::load(&tokens).expect("tokens").token(studio),
            Some([3; 32])
        );
        assert_eq!(
            credential(model, studio).map(|credential| credential.token),
            Some([3; 32])
        );
        assert_eq!(model.connection(studio), ConnectionState::Connecting);

        let again = model
            .save_paired_server(None, &PairingForm::default(), &invite(1), &paired(4), cx)
            .expect("paired again");
        assert_eq!(again, studio, "the same certificate is the same computer");
        let saved = listed(&path);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].group.as_deref(), Some("Office"));
        assert_eq!(
            credential(model, studio).map(|credential| credential.token),
            Some([4; 32]),
            "the new pairing reconnects"
        );

        let moved = model
            .save_paired_server(
                Some(studio),
                &PairingForm::default(),
                &invite(9),
                &paired(5),
                cx,
            )
            .expect("paired in place");
        assert_eq!(moved, studio, "pairing again from its form keeps it");
        assert_eq!(listed(&path).len(), 1);
        assert_eq!(
            listed(&path)[0].group,
            None,
            "a cleared group is kept clear"
        );

        let settings = std::fs::read_to_string(&path).expect("settings");
        std::fs::write(&path, "damaged = [").expect("damaged");
        assert!(
            model
                .save_paired_server(
                    Some(studio),
                    &PairingForm::default(),
                    &invite(9),
                    &paired(6),
                    cx
                )
                .is_err()
        );
        assert_eq!(
            PairingTokens::load(&tokens).expect("tokens").token(studio),
            Some([5; 32]),
            "a pairing that can't be listed keeps the one that works"
        );
        std::fs::write(&path, settings).expect("restored");

        model
            .forget_remote_servers(&[studio], cx)
            .expect("forgotten");
        assert!(listed(&path).is_empty());
        assert_eq!(
            PairingTokens::load(&tokens).expect("tokens").token(studio),
            None
        );
    });
}

#[gpui::test]
fn a_paired_server_is_renamed_from_its_own_form(cx: &mut TestAppContext) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let studio = model
            .save_paired_server(None, &PairingForm::default(), &invite(1), &paired(3), cx)
            .expect("paired");
        let before = credential(model, studio);
        let generation = model.generation(studio);
        model.open_server_form(Some(studio), cx);
        assert!(matches!(model.overlay, Some(Overlay::PairForm(_))));
        let renamed = PairingForm {
            name: "Studio Mac".into(),
            ..PairingForm::default()
        };
        model
            .edit_paired_server(studio, &renamed, cx)
            .expect("renamed");
        assert_eq!(listed(&path)[0].name, "Studio Mac");
        assert_eq!(listed(&path)[0].group, None);
        assert_eq!(credential(model, studio), before);
        assert_eq!(
            model.generation(studio),
            generation,
            "a new name keeps the connection"
        );
    });
}

#[gpui::test]
fn a_revoked_pairing_waits_for_the_user(cx: &mut TestAppContext) {
    let (boot, _, remotes) = boot_with(Vec::new());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let studio = view.update(cx, |model, cx| {
        connect_local(model, cx);
        model
            .save_paired_server(None, &PairingForm::default(), &invite(1), &paired(3), cx)
            .expect("paired")
    });
    let worker = remotes.borrow_mut().remove(&studio).expect("worker");
    work(&worker);
    view.update(cx, |model, cx| {
        let generation = model.generation(studio);
        let unpaired = Some(muxy_client::RemoteReason::Unpaired);
        model.receive(
            (
                studio,
                generation,
                Update::ConnectFailed("revoked".into(), unpaired),
            ),
            cx,
        );
    });
    view.update(cx, |model, _| {
        let error = model
            .servers
            .get(studio)
            .and_then(|runtime| runtime.error.clone());
        assert!(
            error
                .as_deref()
                .is_some_and(|error| error.starts_with("Studio no longer accepts")),
            "{error:?}"
        );
    });
    cx.executor().advance_clock(Duration::from_secs(600));
    cx.run_until_parked();
    assert!(
        !work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Connect)),
        "a revoked computer is not tried again"
    );
}

#[test]
fn pairing_links_are_checked_before_pairing() {
    assert!(parse_link("https://example.com").is_err());
    assert_eq!(
        parse_link(&format!("  {}\n", invite(1).to_link())),
        Ok(invite(1))
    );
}

fn link(fingerprint: u8) -> PairingForm {
    PairingForm {
        link: invite(fingerprint).to_link(),
        ..PairingForm::default()
    }
}

#[gpui::test]
fn a_name_or_group_that_cannot_be_saved_is_refused_before_the_link_is_spent(
    cx: &mut TestAppContext,
) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        for form in [
            PairingForm {
                name: "n".repeat(257),
                ..link(1)
            },
            PairingForm {
                group: "g".repeat(257),
                ..link(1)
            },
        ] {
            assert!(model.start_pairing(None, form, cx).is_err());
            assert!(!model.pairing_running(), "the link was never used");
        }
    });
}

#[gpui::test]
fn a_link_for_another_listed_computer_changes_nothing(cx: &mut TestAppContext) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let named = |name: &str| PairingForm {
            name: name.into(),
            ..PairingForm::default()
        };
        let studio = model
            .save_paired_server(None, &named("Studio"), &invite(1), &paired(3), cx)
            .expect("studio");
        let laptop = model
            .save_paired_server(None, &named("Laptop"), &invite(2), &paired(4), cx)
            .expect("laptop");
        let before = listed(&path);
        let error = model
            .start_pairing(Some(laptop), link(1), cx)
            .expect_err("refused");
        assert!(error.contains("Studio"), "{error}");
        assert!(!model.pairing_running());
        assert!(
            model
                .save_paired_server(
                    Some(laptop),
                    &PairingForm::default(),
                    &invite(1),
                    &paired(5),
                    cx
                )
                .is_err()
        );
        assert_eq!(listed(&path), before);
        assert_eq!(
            credential(model, laptop).map(|credential| credential.token),
            Some([4; 32])
        );
        assert_eq!(
            credential(model, studio).map(|credential| credential.token),
            Some([3; 32])
        );
    });
}

#[gpui::test]
fn a_fingerprint_spelled_in_uppercase_is_the_same_computer(cx: &mut TestAppContext) {
    let studio = ServerEntry::paired(
        "Studio".into(),
        muxy_app_core::settings::PairedServer {
            hosts: invite(0xab).hosts,
            port: 7419,
            fingerprint: "AB".repeat(32),
            device: DeviceId::from_u128(1),
        },
    );
    let (boot, _, _remotes) = boot_with(vec![studio.clone()]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let again = model
            .save_paired_server(None, &PairingForm::default(), &invite(0xab), &paired(3), cx)
            .expect("paired again");
        assert_eq!(again, studio.id);
        assert_eq!(listed(&path).len(), 1);
    });
}

#[gpui::test]
fn an_accepted_pairing_that_could_not_be_saved_is_saved_again_without_the_link(
    cx: &mut TestAppContext,
) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let path = boot.state_path.with_file_name("settings.toml");
    let tokens = boot.state_path.with_file_name("paired-servers.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let settings = std::fs::read_to_string(&path).expect("settings");
        std::fs::write(&path, "damaged = [").expect("damaged");
        assert!(
            model
                .save_pairing(None, &PairingForm::default(), &invite(1), &paired(3), cx)
                .is_err()
        );
        std::fs::write(&path, settings).expect("restored");
        model.start_pairing(None, link(1), cx).expect("saved again");
        assert!(!model.pairing_running(), "the spent link isn't used again");
        let saved = listed(&path);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            PairingTokens::load(&tokens)
                .expect("tokens")
                .token(saved[0].id),
            Some([3; 32])
        );
    });
}

#[gpui::test]
fn forgetting_keeps_a_server_and_its_token_together(cx: &mut TestAppContext) {
    let (boot, _, _remotes) = boot_with(Vec::new());
    let path = boot.state_path.with_file_name("settings.toml");
    let tokens = boot.state_path.with_file_name("paired-servers.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let studio = model
            .save_paired_server(None, &PairingForm::default(), &invite(1), &paired(3), cx)
            .expect("paired");

        let saved_tokens = std::fs::read_to_string(&tokens).expect("tokens");
        std::fs::write(&tokens, "damaged = [").expect("damaged");
        assert!(model.forget_remote_servers(&[studio], cx).is_err());
        assert_eq!(listed(&path).len(), 1, "it stays listed to forget again");
        std::fs::write(&tokens, saved_tokens).expect("restored");

        let settings = std::fs::read_to_string(&path).expect("settings");
        std::fs::write(&path, "damaged = [").expect("damaged");
        assert!(model.forget_remote_servers(&[studio], cx).is_err());
        assert_eq!(
            PairingTokens::load(&tokens).expect("tokens").token(studio),
            Some([3; 32]),
            "a server still listed keeps its token"
        );
        std::fs::write(&path, settings).expect("restored");
        assert!(model.settings.server(studio).is_some());
    });
}

#[gpui::test]
fn closing_the_form_leaves_pairing_running_to_the_end(cx: &mut TestAppContext) {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("port")
        .port();
    let refused = PairingInvite {
        hosts: vec!["127.0.0.1".into()],
        port,
        ..invite(1)
    };
    let (boot, _, _remotes) = boot_with(Vec::new());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        model.open_pair_form(None, cx);
        let form = PairingForm {
            link: refused.to_link(),
            ..PairingForm::default()
        };
        model
            .start_pairing(None, form.clone(), cx)
            .expect("started");
        assert!(model.pairing_running());
        assert!(
            model.start_pairing(None, form, cx).is_err(),
            "one at a time"
        );
        model.dismiss_overlay(cx);
        assert!(model.pairing_running(), "the form owned none of it");
    });
    cx.run_until_parked();
    view.update(cx, |model, _| {
        assert!(!model.pairing_running());
        assert!(model.overlay.is_none());
    });
}
