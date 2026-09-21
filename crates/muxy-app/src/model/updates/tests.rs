use super::*;
use crate::model::tests::stub_boot;
use gpui::TestAppContext;
use muxy_app_core::AppState;

#[gpui::test]
fn newer_release_replaces_downloaded_update_and_preserves_schedule(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.updates.server = Some(muxy_protocol::ServerInfo::current());
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        model.updates.scheduled = true;
        model.updates.retry_preparation = Some(model.generation);
        model.updates.mode = Some(UpdateMode::EndSessions);
        model.updates.download = AppUpdatePhase::Checking;
        let release = model
            .receive_update_release(Ok(Some(Release::fixture("2.0.0-beta-1236"))), cx)
            .expect("newer release");
        assert_eq!(release.version, "2.0.0-beta-1236");
        assert!(model.updates.download == AppUpdatePhase::Downloading);
        model.begin_update(cx);
        assert!(model.quitting == Quitting::Idle);
        model.receive_server_update(
            Ok(ServerUpdate {
                server: muxy_protocol::ServerInfo::current(),
                sessions: 0,
                replaced: false,
            }),
            cx,
        );
        assert!(model.quitting == Quitting::Idle);
        let mut latest = PreparedUpdate::fixture().expect("latest update");
        latest.version = release.version;
        model.finish_update_check(Ok(Some(latest)), cx);
        assert!(!model.updates.checking());
        assert!(model.updates.scheduled);
        assert!(model.updates.retry_preparation.is_none());
        assert!(model.updates.mode.is_none());
        assert_eq!(
            model.updates.ready.as_ref().expect("update").version,
            "2.0.0-beta-1236"
        );
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(model.path.with_file_name("pending-update.json")).expect("record"),
        )
        .expect("json");
        assert_eq!(record["version"], "2.0.0-beta-1236");
        assert_eq!(record["scheduled"], true);
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush | Work::PrepareUpdate { .. }))
    );
}

#[gpui::test]
fn app_download_does_not_block_the_server_restart_action(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.connection = ConnectionState::Ready;
        model.updates.download = AppUpdatePhase::Downloading;
        assert!(!model.update_action_enabled(UpdateAction::ReviewApp));
        assert!(model.update_action_enabled(UpdateAction::RestartServer));
        assert_eq!(
            model.server_update_action(),
            (UpdateAction::RestartServer, "Restart server…")
        );
        model.connection = ConnectionState::Disconnected;
        assert_eq!(
            model.server_update_action(),
            (UpdateAction::Connect, "Connect to server")
        );
    });
}

#[gpui::test]
fn same_or_older_release_reuses_the_prepared_download(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        for release in [
            Some(Release::fixture("2.0.0-beta-1234")),
            Some(Release::fixture("2.0.0-beta-1233")),
            None,
        ] {
            model.updates.download = AppUpdatePhase::Checking;
            assert!(model.receive_update_release(Ok(release), cx).is_none());
            assert!(!model.updates.checking());
            assert!(model.updates.download == AppUpdatePhase::Idle);
            assert_eq!(
                model.updates.ready.as_ref().expect("update").version,
                "2.0.0-beta-1234"
            );
        }
    });
}

#[gpui::test]
fn failed_refresh_or_persistence_keeps_old_update_without_installing_it(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        model.updates.scheduled = true;
        model.updates.download = AppUpdatePhase::Checking;
        model.receive_update_release(Err("offline".into()), cx);
        assert!(model.update_details().expect("status").failed);
        assert_eq!(
            model.update_details().expect("status").action,
            Some((UpdateAction::RetryApp, "Retry app update"))
        );
        model.receive_server_update(
            Ok(ServerUpdate {
                server: muxy_protocol::ServerInfo::current(),
                sessions: 0,
                replaced: false,
            }),
            cx,
        );
        assert!(model.quitting == Quitting::Idle);
        model.path = model.path.join("missing/state.json");
        let mut latest = PreparedUpdate::fixture().expect("latest");
        latest.version = "2.0.0-beta-1236".into();
        model.finish_update_check(Ok(Some(latest)), cx);
        assert!(model.updates.app_error.is_some());
        assert!(model.updates.scheduled);
        assert_eq!(
            model.updates.ready.as_ref().expect("update").version,
            "2.0.0-beta-1234"
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush | Work::PrepareUpdate { .. }))
    );
}

#[gpui::test]
fn cached_update_does_not_bypass_a_manual_release_check(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.updates.ready = Some(PreparedUpdate::fixture().expect("cached update"));
        model.updates.installation = None;
        model.check_for_updates(true, cx);
        assert!(model.close_prompt.is_none());
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Ok");
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush))
    );
}

#[gpui::test]
fn schedule_can_be_cancelled_during_failed_or_pending_refresh(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.updates.ready = Some(PreparedUpdate::fixture().expect("update"));
        model.updates.scheduled = true;
        model
            .updates
            .ready
            .as_ref()
            .expect("update")
            .persist(&model.path.with_file_name("pending-update.json"), true)
            .expect("schedule");
        model.receive_update_release(Err("offline".into()), cx);
        assert!(model.update_scheduled());
        assert!(model.update_action_enabled(UpdateAction::CancelSchedule));
        model.perform_update_action(UpdateAction::CancelSchedule, cx);
        assert!(!model.update_scheduled());
        let read_record = || -> serde_json::Value {
            serde_json::from_slice(
                &std::fs::read(model.path.with_file_name("pending-update.json")).expect("record"),
            )
            .expect("json")
        };
        assert_eq!(read_record()["scheduled"], false);
        model.updates.scheduled = true;
        model.updates.download = AppUpdatePhase::Downloading;
        model.updates.manual = true;
        assert!(model.update_action_enabled(UpdateAction::CancelSchedule));
        model.perform_update_action(UpdateAction::CancelSchedule, cx);
        assert!(!model.update_scheduled());
        assert!(!model.updates.manual);
        let mut latest = PreparedUpdate::fixture().expect("latest");
        latest.version = "2.0.0-beta-1236".into();
        model.finish_update_check(Ok(Some(latest)), cx);
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(model.path.with_file_name("pending-update.json")).expect("record"),
        )
        .expect("json");
        assert_eq!(record["scheduled"], false);
        assert_eq!(record["version"], "2.0.0-beta-1236");
        assert!(model.quitting == Quitting::Idle);
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Flush | Work::PrepareUpdate { .. }))
    );
}
