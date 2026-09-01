use super::*;

fn desktop_notification_title(title: String) -> String {
    if title.is_empty() {
        "Command executed!".to_owned()
    } else {
        title
    }
}

fn desktop_notification_is_sound_only(window_active: bool, target_focused: bool) -> bool {
    window_active && target_focused
}

fn terminal_input_overlay_active(
    overlay_open: bool,
    search_open: bool,
    composer_input_focused: bool,
) -> bool {
    overlay_open || search_open || composer_input_focused
}

fn clear_persistent_flags(store: &mut muxy_core::workspace_store::WorkspaceStore) {
    for workspace in store.states_mut() {
        let tab_ids = workspace
            .root
            .as_ref()
            .map(|root| root.tabs())
            .unwrap_or_default()
            .into_iter()
            .filter(|tab| tab.kind == muxy_core::workspace::TabKind::Terminal)
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>();
        for tab_id in tab_ids {
            if let Some(tab) = workspace.tab_mut(&tab_id) {
                tab.rust_persistent_session = false;
            }
        }
    }
}

fn staged_restart_failure(point: &str) -> bool {
    muxy_core::prefs::is_test_process()
        && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
        && std::env::var("MUXY_TEST_P8_RESTART_FAILURE").as_deref() == Ok(point)
}

fn record_staged_restart_failure_complete() {
    if muxy_core::prefs::is_test_process()
        && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
        && std::env::var_os("MUXY_TEST_P8_RESTART_FAILURE").is_some()
    {
        let _ = std::fs::write(
            muxy_core::prefs::app_support_dir().join(".muxy-test-p8-restart-failure-complete"),
            b"complete\n",
        );
    }
}

pub(crate) struct TerminalRuntime {
    pub(crate) surfaces: TerminalSurfaces,
    pub(super) _tasks: Vec<Task<()>>,
    offline_scan_task: Option<Task<()>>,
    offline_scan_in_flight: bool,
    offline_scan_generation: u64,
}

impl TerminalRuntime {
    pub(super) fn new(surfaces: TerminalSurfaces, tasks: Vec<Task<()>>) -> Self {
        Self {
            surfaces,
            _tasks: tasks,
            offline_scan_task: None,
            offline_scan_in_flight: false,
            offline_scan_generation: 0,
        }
    }
}

impl MainWindow {
    pub(crate) fn reload_terminal_offline(&mut self, cx: &mut Context<Self>) {
        let enabled = muxy_core::prefs::settings::bool_value(
            crate::terminal::offline::ENABLED_SETTING,
            false,
        );
        let seconds = muxy_core::prefs::settings::f64_value(
            crate::terminal::offline::IDLE_THRESHOLD_SETTING,
            crate::terminal::offline::DEFAULT_IDLE_THRESHOLD.as_secs_f64(),
        );
        let idle_threshold = if seconds.is_finite() && seconds > 0.0 {
            Duration::from_secs_f64(seconds)
        } else {
            crate::terminal::offline::DEFAULT_IDLE_THRESHOLD
        };
        self.terminal_runtime.offline_scan_generation = self
            .terminal_runtime
            .offline_scan_generation
            .wrapping_add(1);
        self.terminal_runtime.offline_scan_task = None;
        self.terminal_runtime.offline_scan_in_flight = false;
        self.terminal_runtime
            .surfaces
            .configure_offline(enabled, idle_threshold);
        if !enabled {
            if self.terminal_runtime.surfaces.wake_all_offline() {
                cx.notify();
            }
            return;
        }
        let interval = muxy_terminal::offline::scan_interval(idle_threshold);
        self.terminal_runtime.offline_scan_task = Some(cx.spawn(async move |window, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                if window
                    .update(cx, |window, cx| window.scan_terminal_offline(cx))
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    pub(super) fn scan_terminal_offline(&mut self, cx: &mut Context<Self>) {
        if !self.terminal_runtime.surfaces.offline_enabled()
            || self.terminal_runtime.offline_scan_in_flight
        {
            return;
        }
        let probes = self
            .terminal_runtime
            .surfaces
            .offline_probes(&self.state.tab_workspaces, self.elapsed());
        if probes.is_empty() {
            return;
        }
        let generation = self.terminal_runtime.offline_scan_generation;
        self.terminal_runtime.offline_scan_in_flight = true;
        cx.spawn(async move |window, cx| {
            let decisions = cx
                .background_executor()
                .spawn(async move {
                    probes
                        .into_iter()
                        .map(crate::terminal::offline::evaluate_probe)
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = window.update(cx, |window, cx| {
                if window.terminal_runtime.offline_scan_generation != generation {
                    return;
                }
                window.terminal_runtime.offline_scan_in_flight = false;
                let now = window.elapsed();
                let visible = window
                    .state
                    .active_tab_workspace()
                    .map(|workspace| {
                        workspace
                            .visible_area_tabs()
                            .into_iter()
                            .filter(|(_, tab_id)| {
                                workspace.tab(tab_id).is_some_and(|tab| {
                                    tab.kind == muxy_core::workspace::TabKind::Terminal
                                })
                            })
                            .map(|(_, tab_id)| tab_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let focused = if window.view.terminal.overlay_was_open {
                    None
                } else {
                    window.state.active_tab_workspace().and_then(|workspace| {
                        let area_id = workspace.focused_area_id.as_deref()?;
                        workspace.area(area_id)?.active_tab_id.clone()
                    })
                };
                let window_visible = window.terminal_runtime.surfaces.window_is_visible();
                window.terminal_runtime.surfaces.observe_offline(
                    &window.state.tab_workspaces,
                    &visible,
                    focused.as_deref(),
                    window_visible,
                    now,
                );
                let mut changed = false;
                for decision in decisions.into_iter().filter(|decision| decision.is_idle) {
                    if !window.terminal_runtime.surfaces.can_take_offline(
                        &window.state.tab_workspaces,
                        &decision,
                        now,
                    ) {
                        continue;
                    }
                    let previous = window.state.tab_workspaces.clone();
                    let mut directory_changed = false;
                    for workspace in window.state.tab_workspaces.states_mut() {
                        if let Some(tab) = workspace.tab_mut(&decision.tab_id) {
                            let directory = decision.directory.to_string_lossy().into_owned();
                            if tab.terminal_resume_directory.as_deref() != Some(directory.as_str())
                            {
                                tab.terminal_resume_directory = Some(directory);
                                directory_changed = true;
                            }
                            break;
                        }
                    }
                    if directory_changed && let Err(error) = window.state.persist_tab_workspaces() {
                        window.state.tab_workspaces = previous;
                        log::warn!(
                            "failed to preserve terminal directory before idle freeing: {error}"
                        );
                        continue;
                    }
                    changed |= window.terminal_runtime.surfaces.take_offline(
                        &window.state.tab_workspaces,
                        decision,
                        now,
                    );
                }
                if changed {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn wake_terminal(&mut self, tab_id: &str, cx: &mut Context<Self>) -> bool {
        let woke = self.terminal_runtime.surfaces.wake_offline(tab_id);
        if woke {
            cx.notify();
        }
        woke
    }

    pub(crate) fn terminate_removed_sessions(
        &mut self,
        tab_ids: &[String],
        cx: &mut Context<Self>,
    ) {
        let targets = tab_ids
            .iter()
            .filter(|tab_id| self.terminal_runtime.surfaces.is_persistent_tab(tab_id))
            .cloned()
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return;
        }
        let Some(client) = self.terminal_runtime.surfaces.persistent_client() else {
            return;
        };
        for target in &targets {
            self.terminal_runtime.surfaces.forget_persistent_tab(target);
        }
        cx.background_executor()
            .spawn(async move {
                if let crate::terminal::session::client::TerminateOutcome::Unreachable(error) =
                    client.terminate_each(&targets)
                {
                    log::warn!("removed terminal sessions could not be terminated: {error}");
                }
            })
            .detach();
    }

    pub(crate) fn request_persistent_sessions(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if enabled && let Some(issue) = self.state.tab_workspaces.terminal_identity_issues().first()
        {
            let problem = match issue.problem {
                muxy_core::workspace_store::TerminalIdentityProblem::Malformed => {
                    "is not a canonical uppercase UUID"
                }
                muxy_core::workspace_store::TerminalIdentityProblem::Duplicate => {
                    "is duplicated across terminal tabs"
                }
            };
            self.feedback(
                "Unable to enable terminal persistence",
                format!(
                    "Terminal ID {} {problem}. Close or recreate that terminal tab and try again.",
                    issue.tab_id
                ),
                crate::toast::ToastTone::Error,
                cx,
            );
            return;
        }
        let answer = self.ask(
            "Restart Muxy?".to_owned(),
            "Changing persistent terminal sessions requires restarting the whole app.".to_owned(),
            &["Restart Now", "Cancel"],
            cx,
        );
        cx.spawn(async move |window, cx| {
            if answer.await != Some(0) {
                return;
            }
            let _ = window.update(cx, |window, cx| {
                window.begin_persistent_sessions_change(enabled, cx);
            });
        })
        .detach();
    }

    pub(crate) fn begin_persistent_sessions_change(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |window, cx| {
            let prepared = cx
                .background_executor()
                .spawn(async { crate::relaunch::PreparedRelaunch::prepare() })
                .await;
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = window.update(cx, |window, cx| {
                        window.feedback(
                            "Unable to restart Muxy",
                            error,
                            crate::toast::ToastTone::Error,
                            cx,
                        );
                    });
                    return;
                }
            };
            if enabled {
                let _ = window.update(cx, |window, cx| {
                    window.enable_persistent_sessions(prepared, cx);
                });
                return;
            }
            let client = window
                .update(cx, |window, cx| {
                    if let Err(error) = window.capture_terminal_directories() {
                        window.feedback(
                            "Unable to disable terminal persistence",
                            format!("The latest terminal folders could not be saved: {error}"),
                            crate::toast::ToastTone::Error,
                            cx,
                        );
                        return None;
                    }
                    let client = window.terminal_runtime.surfaces.persistent_client();
                    if client.is_none() {
                        if window
                            .terminal_runtime
                            .surfaces
                            .persistent_service_available()
                        {
                            window.feedback(
                                "Unable to disable terminal persistence",
                                "Muxy can't reach the terminal session service. Persistence remains enabled.",
                                crate::toast::ToastTone::Error,
                                cx,
                            );
                            return None;
                        }
                        return Some(None);
                    }
                    window
                        .terminal_runtime
                        .surfaces
                        .begin_persistent_termination();
                    Some(client)
                })
                .ok()
                .flatten();
            let Some(client) = client else {
                return;
            };
            let Some(client) = client else {
                let _ = window.update(cx, |window, cx| {
                    window.disable_persistent_sessions(prepared, cx);
                });
                return;
            };
            let outcome = cx
                .background_executor()
                .spawn(async move { client.terminate_all() })
                .await;
            let _ = window.update(cx, |window, cx| match outcome {
                crate::terminal::session::client::TerminateOutcome::Terminated
                | crate::terminal::session::client::TerminateOutcome::NoSessions => {
                    window.disable_persistent_sessions(prepared, cx)
                }
                crate::terminal::session::client::TerminateOutcome::Unreachable(error) => {
                    window
                        .terminal_runtime
                        .surfaces
                        .fail_persistent_termination();
                    window.feedback(
                        "Unable to disable terminal persistence",
                        format!("{error}. Terminal persistence remains enabled."),
                        crate::toast::ToastTone::Error,
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn enable_persistent_sessions(
        &mut self,
        prepared: crate::relaunch::PreparedRelaunch,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = muxy_core::prefs::settings::try_set(
            crate::terminal::session::PERSISTENT_SESSION_SETTING,
            serde_json::Value::Bool(true),
        ) {
            self.feedback(
                "Unable to enable terminal persistence",
                format!("The setting could not be saved: {error}"),
                crate::toast::ToastTone::Error,
                cx,
            );
            return;
        }
        let commit = if staged_restart_failure("enable-after-setting") {
            Err("injected failure after the persistent setting write".to_owned())
        } else {
            prepared.commit()
        };
        if let Err(error) = commit {
            let rollback = muxy_core::prefs::settings::try_set(
                crate::terminal::session::PERSISTENT_SESSION_SETTING,
                serde_json::Value::Bool(false),
            );
            record_staged_restart_failure_complete();
            self.feedback(
                "Unable to enable terminal persistence",
                match rollback {
                    Ok(()) => error,
                    Err(rollback) => format!("{error}; setting rollback failed: {rollback}"),
                },
                crate::toast::ToastTone::Error,
                cx,
            );
            return;
        }
        self.flush_notification_store();
        self.flush_composer_store();
        cx.quit();
    }

    fn disable_persistent_sessions(
        &mut self,
        prepared: crate::relaunch::PreparedRelaunch,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.state.tab_workspaces.clone();
        clear_persistent_flags(&mut self.state.tab_workspaces);
        if let Err(error) = self.state.persist_tab_workspaces() {
            self.state.tab_workspaces = snapshot;
            self.post_termination_disable_failure(
                format!("The terminal layout could not be saved: {error}"),
                cx,
            );
            return;
        }
        if staged_restart_failure("disable-after-workspace") {
            self.restore_disabled_transaction(
                snapshot,
                "injected failure after the workspace write".to_owned(),
                cx,
            );
            return;
        }
        if let Err(error) = muxy_core::prefs::settings::try_set(
            crate::terminal::session::PERSISTENT_SESSION_SETTING,
            serde_json::Value::Bool(false),
        ) {
            self.restore_disabled_transaction(snapshot, format!("The setting failed: {error}"), cx);
            return;
        }
        if staged_restart_failure("disable-after-setting") {
            self.restore_disabled_transaction(
                snapshot,
                "injected failure after the persistent setting write".to_owned(),
                cx,
            );
            return;
        }
        if let Err(error) = prepared.commit() {
            self.restore_disabled_transaction(snapshot, error, cx);
            return;
        }
        self.flush_notification_store();
        self.flush_composer_store();
        cx.quit();
    }

    fn restore_disabled_transaction(
        &mut self,
        snapshot: muxy_core::workspace_store::WorkspaceStore,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.state.tab_workspaces = snapshot;
        let workspace_rollback = self.state.persist_tab_workspaces();
        let setting_rollback = muxy_core::prefs::settings::try_set(
            crate::terminal::session::PERSISTENT_SESSION_SETTING,
            serde_json::Value::Bool(true),
        );
        let mut message = error;
        if let Err(rollback) = workspace_rollback {
            message.push_str(&format!("; workspace rollback failed: {rollback}"));
        }
        if let Err(rollback) = setting_rollback {
            message.push_str(&format!("; setting rollback failed: {rollback}"));
        }
        self.post_termination_disable_failure(message, cx);
    }

    fn post_termination_disable_failure(&mut self, error: String, cx: &mut Context<Self>) {
        self.terminal_runtime
            .surfaces
            .mark_persistent_sessions_missing();
        record_staged_restart_failure_complete();
        self.feedback(
            "Terminal sessions were already stopped",
            format!("{error}. Persistence remains enabled; use Start Fresh for affected tabs."),
            crate::toast::ToastTone::Error,
            cx,
        );
    }

    fn capture_terminal_directories(&mut self) -> Result<(), std::io::Error> {
        let previous = self.state.tab_workspaces.clone();
        let tab_ids = self
            .state
            .tab_workspaces
            .states()
            .iter()
            .flat_map(|workspace| {
                workspace
                    .root
                    .as_ref()
                    .map(|root| root.tabs())
                    .unwrap_or_default()
            })
            .filter(|tab| tab.kind == muxy_core::workspace::TabKind::Terminal)
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>();
        let mut changed = false;
        for tab_id in tab_ids {
            let directory = self
                .terminal_runtime
                .surfaces
                .handle(&tab_id)
                .and_then(|handle| handle.metadata().working_directory.clone())
                .filter(|directory| std::path::Path::new(directory).is_absolute());
            let Some(directory) = directory else {
                continue;
            };
            for workspace in self.state.tab_workspaces.states_mut() {
                if let Some(tab) = workspace.tab_mut(&tab_id)
                    && tab.terminal_resume_directory.as_deref() != Some(directory.as_str())
                {
                    tab.terminal_resume_directory = Some(directory.clone());
                    changed = true;
                }
            }
        }
        if changed && let Err(error) = self.state.persist_tab_workspaces() {
            self.state.tab_workspaces = previous;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn reconnect_terminal(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        if self.terminal_runtime.surfaces.retry_persistent(tab_id) {
            cx.notify();
            return;
        }
        if !self
            .terminal_runtime
            .surfaces
            .persistent_service_available()
        {
            self.feedback(
                "Terminal session service is unavailable",
                "Muxy could not start its terminal session service. Turn off persistent terminal sessions in Settings to use ordinary terminals.",
                crate::toast::ToastTone::Error,
                cx,
            );
        }
    }

    pub(crate) fn start_fresh_terminal(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        match self
            .terminal_runtime
            .surfaces
            .start_fresh(&mut self.state.tab_workspaces, tab_id)
        {
            Ok(true) => cx.notify(),
            Ok(false) => {}
            Err(error) => self.feedback(
                "Unable to start terminal",
                format!("The terminal state could not be saved: {error}"),
                crate::toast::ToastTone::Error,
                cx,
            ),
        }
    }

    pub(crate) fn submit_quick_terminal_notification(
        &mut self,
        title: String,
        body: String,
        focused_osc: bool,
        cx: &mut Context<Self>,
    ) {
        self.submit_notification(
            crate::notifications::ResolvedNotificationEvent {
                target: None,
                source: muxy_core::notifications::NotificationSource::Osc,
                origin: crate::notifications::NotificationOrigin::TerminalOsc,
                title: desktop_notification_title(title),
                body,
                timestamp: muxy_core::store::reference_now(),
            },
            focused_osc,
            cx,
        );
    }

    pub(super) fn elapsed(&self) -> Duration {
        self.view.terminal.started_at.elapsed()
    }

    pub(super) fn on_runtime_event(&mut self, event: TerminalEvent, cx: &mut Context<Self>) {
        let Some((tab_id, signal)) = self.terminal_runtime.surfaces.route(event, cx) else {
            return;
        };
        match signal {
            SurfaceSignal::DesktopNotification { title, body } => {
                let Some(target) = self.state.notification_target_for_pane(&tab_id) else {
                    return;
                };
                let focused_osc = desktop_notification_is_sound_only(
                    self.view.window_active,
                    self.state.notification_target_is_focused(&target),
                );
                self.submit_notification(
                    crate::notifications::ResolvedNotificationEvent {
                        target: Some(target),
                        source: muxy_core::notifications::NotificationSource::Osc,
                        origin: crate::notifications::NotificationOrigin::TerminalOsc,
                        title: desktop_notification_title(title),
                        body,
                        timestamp: muxy_core::store::reference_now(),
                    },
                    focused_osc,
                    cx,
                );
            }
            SurfaceSignal::Exited => {
                let disposition = self
                    .terminal_runtime
                    .surfaces
                    .persistent_surface_exited(&tab_id);
                match disposition {
                    crate::terminal::surfaces::PersistentExitDisposition::Direct => {
                        if self
                            .terminal_runtime
                            .surfaces
                            .handle_exit(&mut self.state.tab_workspaces, &tab_id)
                        {
                            let _ = self.state.persist_tab_workspaces();
                        }
                    }
                    crate::terminal::surfaces::PersistentExitDisposition::Retain => {}
                    crate::terminal::surfaces::PersistentExitDisposition::Recover => {
                        let Some(client) = self.terminal_runtime.surfaces.persistent_client()
                        else {
                            let _ = self.terminal_runtime.surfaces.finish_persistent_exit(
                                &mut self.state.tab_workspaces,
                                &tab_id,
                                crate::terminal::session::client::QueryOutcome::Unreachable(
                                    "terminal session service is unavailable".to_owned(),
                                ),
                            );
                            cx.notify();
                            return;
                        };
                        cx.spawn(async move |window, cx| {
                            let query_tab_id = tab_id.clone();
                            let outcome = cx
                                .background_executor()
                                .spawn(async move { client.query(&query_tab_id) })
                                .await;
                            let _ = window.update(cx, |window, cx| {
                                if let Err(error) =
                                    window.terminal_runtime.surfaces.finish_persistent_exit(
                                        &mut window.state.tab_workspaces,
                                        &tab_id,
                                        outcome,
                                    )
                                {
                                    log::warn!(
                                        "failed to reconcile persistent session exit {tab_id}: {error}"
                                    );
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    }
                }
                cx.notify();
            }
            SurfaceSignal::Confirm { .. } => self.show_next_confirmation(cx),
            signal => {
                let metadata_before = self
                    .terminal_runtime
                    .surfaces
                    .handle(&tab_id)
                    .map(|handle| handle.metadata().clone());
                if self.terminal_runtime.surfaces.apply(&tab_id, signal) {
                    let metadata_after = self
                        .terminal_runtime
                        .surfaces
                        .handle(&tab_id)
                        .map(|handle| handle.metadata().clone());
                    if !self.terminal_runtime.surfaces.has_native_scrollbar(&tab_id)
                        && metadata_after.as_ref().map(|metadata| metadata.scrollbar)
                            != metadata_before.as_ref().map(|metadata| metadata.scrollbar)
                    {
                        self.reveal_scrollbar(&tab_id, cx);
                    }
                    if let (Some(before), Some(after)) = (metadata_before, metadata_after) {
                        self.record_terminal_indicators(&tab_id, &before, &after, cx);
                    }
                    self.sync_tab_metadata(&tab_id);
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn scrollbar_metrics(
        &self,
        tab_id: &str,
    ) -> Option<muxy_terminal::scrollbar::ScrollbarMetrics> {
        self.terminal_runtime
            .surfaces
            .handle(tab_id)
            .map(|handle| handle.metadata().scrollbar)
    }

    pub(super) fn record_terminal_indicators(
        &mut self,
        tab_id: &str,
        before: &muxy_terminal::backend::SurfaceMetadata,
        after: &muxy_terminal::backend::SurfaceMetadata,
        cx: &mut Context<Self>,
    ) {
        if before.progress.is_active() && !after.progress.is_active() {
            self.view.terminal.attention.insert(tab_id.to_owned());
        }
        if before.bell_generation == after.bell_generation {
            return;
        }
        self.view.terminal.attention.insert(tab_id.to_owned());
        self.view
            .terminal
            .bell_flashes
            .insert(tab_id.to_owned(), self.elapsed() + BELL_FLASH_DURATION);
        self.view.terminal.bell_expiry = Some(cx.spawn(async move |window, cx| {
            cx.background_executor().timer(BELL_FLASH_DURATION).await;
            let _ = window.update(cx, |window, cx| {
                let now = window.elapsed();
                window
                    .view
                    .terminal
                    .bell_flashes
                    .retain(|_, deadline| *deadline > now);
                cx.notify();
            });
        }));
    }

    pub(super) fn show_next_confirmation(&mut self, cx: &mut Context<Self>) {
        if self.view.overlay.is_open() {
            cx.notify();
            return;
        }
        let Some((tab_id, id, kind)) = self.terminal_runtime.surfaces.active_confirmation() else {
            cx.notify();
            return;
        };
        self.view.subscriptions.clear();
        self.view.pending_focus = Some(self.view.menu_focus.clone());
        self.view.overlay = Overlay::TerminalConfirm { tab_id, id, kind };
        cx.notify();
    }

    pub(crate) fn resolve_terminal_confirmation(&mut self, approved: bool, cx: &mut Context<Self>) {
        let Overlay::TerminalConfirm { tab_id, id, kind } = &self.view.overlay else {
            return;
        };
        let (tab_id, id, kind) = (tab_id.clone(), *id, *kind);
        self.terminal_runtime
            .surfaces
            .perform(&tab_id, SurfaceAction::ClipboardDecision { id, approved });
        self.view.overlay = Overlay::None;
        if approved && kind == ConfirmationKind::ActiveProcessClose {
            self.close_tab(&tab_id, cx);
        }
        if self
            .terminal_runtime
            .surfaces
            .active_confirmation()
            .is_none()
        {
            self.view.pending_focus = Some(self.view.workspace_focus.clone());
        }
        self.show_next_confirmation(cx);
    }

    pub(super) fn sync_tab_metadata(&mut self, tab_id: &str) {
        let Some(metadata) = self
            .terminal_runtime
            .surfaces
            .handle(tab_id)
            .map(|handle| handle.metadata().clone())
        else {
            return;
        };
        let mut changed = false;
        for state in self.state.tab_workspaces.states_mut() {
            if let Some(tab) = state.tab_mut(tab_id) {
                if let Some(title) = metadata.title
                    && tab.pane_title.as_deref() != Some(title.as_str())
                {
                    tab.pane_title = Some(title);
                    changed = true;
                }
                if let Some(directory) = metadata
                    .working_directory
                    .filter(|directory| std::path::Path::new(directory).is_absolute())
                    && tab.terminal_resume_directory.as_deref() != Some(directory.as_str())
                {
                    tab.terminal_resume_directory = Some(directory);
                    changed = true;
                }
                break;
            }
        }
        if changed {
            let _ = self.state.persist_tab_workspaces();
        }
    }

    pub(crate) fn reconcile_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible: Vec<String> = self
            .state
            .active_tab_workspace()
            .map(|workspace| {
                workspace
                    .visible_area_tabs()
                    .into_iter()
                    .filter(|(_, tab_id)| {
                        workspace
                            .tab(tab_id)
                            .is_some_and(|tab| tab.kind == muxy_core::workspace::TabKind::Terminal)
                    })
                    .map(|(_, tab_id)| tab_id)
                    .collect()
            })
            .unwrap_or_default();
        let probes = self.terminal_runtime.surfaces.reconcile(
            &self.state.tab_workspaces,
            &visible,
            window,
            cx,
        );
        if let Some(client) = self.terminal_runtime.surfaces.persistent_client() {
            for tab_id in probes {
                let client = client.clone();
                cx.spawn(async move |window, cx| {
                    let query_tab_id = tab_id.clone();
                    let outcome = cx
                        .background_executor()
                        .spawn(async move { client.wait_for_session(&query_tab_id) })
                        .await;
                    let _ = window.update(cx, |window, cx| {
                        if let Err(error) = window.terminal_runtime.surfaces.finish_establishment(
                            &mut window.state.tab_workspaces,
                            &tab_id,
                            outcome,
                        ) {
                            log::warn!("failed to publish persistent session {tab_id}: {error}");
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        self.reconcile_search_bars(&visible, window, cx);
        self.view
            .terminal
            .scrollbar_reveal
            .retain(|tab_id, _| visible.contains(tab_id));
        let terminals = &self.terminal_runtime.surfaces;
        self.view
            .terminal
            .attention
            .retain(|tab_id| terminals.handle(tab_id).is_some());
        let now = self.elapsed();
        self.view
            .terminal
            .bell_flashes
            .retain(|tab_id, deadline| terminals.handle(tab_id).is_some() && *deadline > now);
        if !self.view.overlay.is_open()
            && let Some((tab_id, id, kind)) = self.terminal_runtime.surfaces.active_confirmation()
        {
            self.view.subscriptions.clear();
            self.view.pending_focus = Some(self.view.menu_focus.clone());
            self.view.overlay = Overlay::TerminalConfirm { tab_id, id, kind };
        }

        self.reconcile_composer_target(cx);
        let overlay_open = terminal_input_overlay_active(
            self.view.overlay.is_open(),
            !self.view.terminal.search_inputs.is_empty(),
            self.composer_input_is_focused(window, cx),
        );
        if overlay_open != self.view.terminal.overlay_was_open {
            self.terminal_runtime
                .surfaces
                .set_overlay_active(overlay_open);
            self.view.terminal.overlay_was_open = overlay_open;
        }

        self.view.window_active = window.is_window_active();
        self.terminal_runtime
            .surfaces
            .set_window_active(self.view.window_active);

        let focused = if overlay_open {
            None
        } else {
            self.state
                .active_tab_workspace()
                .and_then(|workspace| {
                    let area_id = workspace.focused_area_id.as_deref()?;
                    workspace.area(area_id)?.active_tab_id.clone()
                })
                .or_else(|| visible.first().cloned())
        };
        if self.terminal_runtime.surfaces.window_is_visible()
            && focused
                .as_deref()
                .is_some_and(|tab_id| self.terminal_runtime.surfaces.wake_offline(tab_id))
        {
            cx.notify();
        }
        self.terminal_runtime
            .surfaces
            .set_focused_tab(focused.as_deref());
        let window_visible = self.terminal_runtime.surfaces.window_is_visible();
        self.terminal_runtime
            .surfaces
            .set_remote_projects(self.state.remote_project_ids());
        self.terminal_runtime.surfaces.observe_offline(
            &self.state.tab_workspaces,
            &visible,
            focused.as_deref(),
            window_visible,
            self.elapsed(),
        );
        if window.is_window_active()
            && let Some(tab_id) = focused
        {
            self.view.terminal.attention.remove(&tab_id);
        }
    }

    pub(super) fn reconcile_search_bars(
        &mut self,
        visible: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let searching: Vec<String> = visible
            .iter()
            .filter(|tab_id| {
                self.terminal_runtime
                    .surfaces
                    .handle(tab_id)
                    .is_some_and(|handle| handle.metadata().search_totals.active)
            })
            .cloned()
            .collect();
        self.view
            .terminal
            .search_inputs
            .retain(|tab_id, _| searching.contains(tab_id));
        self.view
            .terminal
            .search_subscriptions
            .retain(|tab_id, _| searching.contains(tab_id));

        let style = self.input_style();
        for tab_id in searching {
            if self.view.terminal.search_inputs.contains_key(&tab_id) {
                continue;
            }
            let input = cx.new(|cx| {
                TextInput::new(style, cx)
                    .with_key_context(muxy_ui::text_input::SEARCH_CONTEXT)
                    .with_placeholder("Search")
            });
            let owner = tab_id.clone();
            let subscription = cx.subscribe(&input, move |window: &mut Self, input, event, cx| {
                let owner = owner.clone();
                match event {
                    InputEvent::Changed => {
                        let needle = input.read(cx).text().to_owned();
                        window.dispatch_search_query(owner, needle, cx);
                    }
                    InputEvent::Submitted => window.navigate_search(true, cx),
                    InputEvent::Cancelled => window.close_search(&owner, cx),
                }
            });
            self.view
                .terminal
                .search_subscriptions
                .insert(tab_id.clone(), subscription);
            self.view.terminal.search_inputs.insert(tab_id, input);
        }

        if let Some(tab_id) = self.view.terminal.pending_search_focus.clone()
            && let Some(input) = self.view.terminal.search_inputs.get(&tab_id)
        {
            self.view.terminal.pending_search_focus = None;
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle);
        }
    }

    pub(crate) fn forward_pane_pointer(
        &mut self,
        tab_id: &str,
        area_id: &str,
        input: PointerInput,
        cx: &mut Context<Self>,
    ) -> bool {
        if let PointerInput::Moved { x, .. } = input {
            if self.view.terminal.scrollbar_drag.is_some() {
                return true;
            }
            self.reveal_scrollbar_near_track(tab_id, area_id, x, cx);
        }
        self.terminal_runtime
            .surfaces
            .forward_pointer(tab_id, input)
    }

    pub(super) fn release_pointer_outside_panes(&mut self, position: Point<Pixels>) {
        let Some(tab_id) = self.terminal_runtime.surfaces.pointer_tab() else {
            return;
        };
        let inside = self
            .state
            .active_tab_workspace()
            .and_then(|workspace| workspace.area_containing_tab(tab_id))
            .filter(|area| area.active_tab_id.as_deref() == Some(tab_id))
            .and_then(|area| self.view.workspace.area_bounds.get(&area.id))
            .is_some_and(|bounds| bounds.contains(&position));
        if !inside {
            self.terminal_runtime.surfaces.clear_pointer_tab();
        }
    }

    pub(crate) fn focus_pane(&mut self, tab_id: &str, area_id: &str, cx: &mut Context<Self>) {
        self.wake_terminal(tab_id, cx);
        self.close_search_except(Some(tab_id), cx);
        self.focus_area(area_id, cx);
    }

    pub(super) fn focused_tab_id(&self) -> Option<String> {
        let workspace = self.state.active_tab_workspace()?;
        let area_id = workspace.focused_area_id.as_deref()?;
        workspace.area(area_id)?.active_tab_id.clone()
    }

    pub(crate) fn open_search(&mut self, cx: &mut Context<Self>) {
        let Some(tab_id) = self.focused_tab_id() else {
            return;
        };
        self.terminal_runtime
            .surfaces
            .perform(&tab_id, SurfaceAction::SearchStart);
        self.view.terminal.pending_search_focus = Some(tab_id);
        cx.notify();
    }

    pub(crate) fn close_search(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        if self.view.terminal.search_inputs.remove(tab_id).is_none() {
            return;
        }
        self.view.terminal.search_subscriptions.remove(tab_id);
        self.view.terminal.search_debounce = None;
        self.terminal_runtime
            .surfaces
            .perform(tab_id, SurfaceAction::SearchEnd);
        self.view.pending_focus = Some(self.view.workspace_focus.clone());
        cx.notify();
    }

    pub(super) fn close_search_except(&mut self, keep: Option<&str>, cx: &mut Context<Self>) {
        let stale: Vec<String> = self
            .view
            .terminal
            .search_inputs
            .keys()
            .filter(|tab_id| Some(tab_id.as_str()) != keep)
            .cloned()
            .collect();
        for tab_id in stale {
            self.close_search(&tab_id, cx);
        }
    }

    pub(crate) fn navigate_search(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(tab_id) = self.searching_tab_id() else {
            return;
        };
        let action = if forward {
            SurfaceAction::SearchNext
        } else {
            SurfaceAction::SearchPrevious
        };
        self.terminal_runtime.surfaces.perform(&tab_id, action);
        cx.notify();
    }

    pub(super) fn searching_tab_id(&self) -> Option<String> {
        self.focused_tab_id()
            .filter(|tab_id| self.view.terminal.search_inputs.contains_key(tab_id))
            .or_else(|| self.view.terminal.search_inputs.keys().next().cloned())
    }

    pub(super) fn dispatch_search_query(
        &mut self,
        tab_id: String,
        needle: String,
        cx: &mut Context<Self>,
    ) {
        match dispatch_for_query(needle.clone()) {
            crate::terminal::SearchDispatch::Immediate(_) => {
                self.view.terminal.search_debounce = None;
                self.terminal_runtime
                    .surfaces
                    .perform(&tab_id, SurfaceAction::SearchQuery(needle));
                cx.notify();
            }
            crate::terminal::SearchDispatch::Debounced { delay, .. } => {
                self.view.terminal.search_debounce = Some(cx.spawn(async move |window, cx| {
                    cx.background_executor().timer(delay).await;
                    let _ = window.update(cx, |window, cx| {
                        let current = window
                            .view
                            .terminal
                            .search_inputs
                            .get(&tab_id)
                            .map(|input| input.read(cx).text().to_owned());
                        if current.as_deref() != Some(needle.as_str()) {
                            return;
                        }
                        window
                            .terminal_runtime
                            .surfaces
                            .perform(&tab_id, SurfaceAction::SearchQuery(needle));
                        cx.notify();
                    });
                }));
            }
        }
    }

    pub(super) fn reveal_scrollbar(&mut self, tab_id: &str, cx: &mut Context<Self>) {
        let now = self.elapsed();
        self.view
            .terminal
            .scrollbar_reveal
            .entry(tab_id.to_owned())
            .or_default()
            .reveal(now);
        self.view.terminal.scrollbar_expiry = Some(cx.spawn(async move |window, cx| {
            cx.background_executor()
                .timer(muxy_ui::scrollbar::REVEAL_DURATION)
                .await;
            let _ = window.update(cx, |_, cx| cx.notify());
        }));
        cx.notify();
    }

    pub(super) fn reveal_scrollbar_near_track(
        &mut self,
        tab_id: &str,
        area_id: &str,
        x: f64,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_runtime.surfaces.has_native_scrollbar(tab_id) {
            return;
        }
        let Some(bounds) = self.view.workspace.area_bounds.get(area_id).copied() else {
            return;
        };
        let now = self.elapsed();
        let pointer_x = x - f64::from(bounds.origin.x);
        let width = f64::from(bounds.size.width);
        let near = self
            .view
            .terminal
            .scrollbar_reveal
            .entry(tab_id.to_owned())
            .or_default()
            .extend_near_track(now, pointer_x, width, f64::from(SCROLLBAR_WIDTH));
        if near {
            self.reveal_scrollbar(tab_id, cx);
        }
    }

    pub(crate) fn begin_scrollbar_drag(
        &mut self,
        tab_id: &str,
        area_id: &str,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.view.workspace.area_bounds.get(area_id).copied() else {
            return;
        };
        let Some(geometry) = self.thumb_geometry(tab_id, bounds) else {
            return;
        };
        let pointer_y = f64::from(position.y - bounds.origin.y);
        self.view
            .terminal
            .scrollbar_reveal
            .entry(tab_id.to_owned())
            .or_default()
            .begin_drag();
        self.view.terminal.scrollbar_drag = Some(ScrollbarDrag {
            tab_id: tab_id.to_owned(),
            area_id: area_id.to_owned(),
            grab: pointer_y - f64::from(SCROLLBAR_TRACK_INSET) - geometry.origin,
            origin: geometry.origin,
            last_row: None,
        });
        cx.notify();
    }

    pub(super) fn drag_scrollbar(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = &self.view.terminal.scrollbar_drag else {
            return false;
        };
        let (tab_id, area_id, grab) = (drag.tab_id.clone(), drag.area_id.clone(), drag.grab);
        let Some(bounds) = self.view.workspace.area_bounds.get(&area_id).copied() else {
            return true;
        };
        let Some(metrics) = self.scrollbar_metrics(&tab_id) else {
            return true;
        };
        let track = workspace_view::scrollbar_track_length(bounds);
        let Some(geometry) = ThumbGeometry::from_lengths(
            metrics.total as f64,
            metrics.visible as f64,
            metrics.offset as f64,
            track,
            SCROLLBAR_MIN_THUMB,
        ) else {
            return true;
        };
        let pointer_y = f64::from(position.y - bounds.origin.y);
        let travel = (track - geometry.length).max(0.0);
        let origin = (pointer_y - f64::from(SCROLLBAR_TRACK_INSET) - grab).clamp(0.0, travel);
        let row = muxy_terminal::scrollbar::row_offset_for_thumb_origin(
            metrics,
            origin,
            track,
            geometry.length,
        );
        if let Some(drag) = &mut self.view.terminal.scrollbar_drag
            && drag.origin != origin
        {
            drag.origin = origin;
            cx.notify();
        }
        if let Some(drag) = &mut self.view.terminal.scrollbar_drag
            && drag.last_row != Some(row)
        {
            drag.last_row = Some(row);
            self.terminal_runtime
                .surfaces
                .perform(&tab_id, SurfaceAction::ScrollToRow(row));
            cx.notify();
        }
        true
    }

    pub(super) fn end_scrollbar_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.view.terminal.scrollbar_drag.take() else {
            return;
        };
        let now = self.elapsed();
        if let Some(reveal) = self.view.terminal.scrollbar_reveal.get_mut(&drag.tab_id) {
            reveal.end_drag(now);
        }
        self.reveal_scrollbar(&drag.tab_id, cx);
    }

    pub(super) fn thumb_geometry(
        &self,
        tab_id: &str,
        bounds: Bounds<Pixels>,
    ) -> Option<ThumbGeometry> {
        let metrics = self.scrollbar_metrics(tab_id)?;
        ThumbGeometry::from_lengths(
            metrics.total as f64,
            metrics.visible as f64,
            metrics.offset as f64,
            workspace_view::scrollbar_track_length(bounds),
            SCROLLBAR_MIN_THUMB,
        )
    }

    pub(super) fn focused_working_directory(&self) -> Option<String> {
        let workspace = self.state.active_tab_workspace()?;
        let area_id = workspace.focused_area_id.as_deref()?;
        let tab_id = workspace.area(area_id)?.active_tab_id.as_deref()?;
        self.terminal_runtime
            .surfaces
            .handle(tab_id)?
            .metadata()
            .working_directory
            .clone()
    }

    pub(super) fn focused_launch_directory(&self) -> Option<std::path::PathBuf> {
        let workspace = self.state.active_tab_workspace()?;
        let area_id = workspace.focused_area_id.as_deref()?;
        let tab_id = workspace.area(area_id)?.active_tab_id.as_deref()?;
        self.terminal_runtime
            .surfaces
            .handle(tab_id)?
            .metadata()
            .working_directory
            .as_ref()
            .map(std::path::PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_notification_title_default_and_focus_policy_are_exact() {
        assert_eq!(
            desktop_notification_title(String::new()),
            "Command executed!"
        );
        assert_eq!(desktop_notification_title("  ".to_owned()), "  ",);
        assert_eq!(desktop_notification_title("Done".to_owned()), "Done");
        assert!(desktop_notification_is_sound_only(true, true));
        assert!(!desktop_notification_is_sound_only(true, false));
        assert!(!desktop_notification_is_sound_only(false, true));
        assert!(!desktop_notification_is_sound_only(false, false));
    }

    #[test]
    fn composer_only_suppresses_terminal_input_while_its_editor_is_focused() {
        assert!(terminal_input_overlay_active(false, false, true));
        assert!(!terminal_input_overlay_active(false, false, false));
        assert!(terminal_input_overlay_active(true, false, false));
        assert!(terminal_input_overlay_active(false, true, false));
    }
}
