mod status;
#[cfg(test)]
mod tests;

use std::time::Duration;

use gpui::{Context, Task};

use super::{AppModel, ConnectionState, Quitting};
use crate::boot::Work;
use crate::server::{ServerUpdate, UpdateMode};
use crate::updater::{Installation, PreparedUpdate, Release};

pub(crate) use status::UpdateAction;
use std::io::Write;

#[derive(Default)]
pub(super) struct Updater {
    installation: Option<Installation>,
    unavailable: Option<String>,
    download: AppUpdatePhase,
    app_error: Option<String>,
    server_error: Option<String>,
    pub(crate) anchor: muxy_ui::popover::PopoverAnchor,
    manual: bool,
    pub(super) ready: Option<PreparedUpdate>,
    task: Option<Task<()>>,
    poll: Option<Task<()>>,
    pub(super) server: Option<muxy_protocol::ServerInfo>,
    pub(super) sessions: usize,
    pub(super) scheduled: bool,
    pub(super) queued_attaches:
        std::collections::BTreeMap<muxy_app_core::PaneId, muxy_protocol::Size>,
    mode: Option<UpdateMode>,
    phase: ServerUpdatePhase,
    reconcile: Option<Task<()>>,
    retry_preparation: Option<u64>,
}

#[derive(Default, Eq, PartialEq)]
enum AppUpdatePhase {
    #[default]
    Idle,
    Checking,
    Downloading,
}

#[derive(Default, Eq, PartialEq)]
enum ServerUpdatePhase {
    #[default]
    Idle,
    Checking,
    Replacing,
    Notified,
    Reconnecting,
    Failed,
}

impl Updater {
    fn checking(&self) -> bool {
        self.download != AppUpdatePhase::Idle
    }

    pub(super) fn replacing(&self) -> bool {
        matches!(
            self.phase,
            ServerUpdatePhase::Replacing
                | ServerUpdatePhase::Notified
                | ServerUpdatePhase::Reconnecting
        )
    }
}

impl AppModel {
    pub(super) fn start_update_checks(&mut self, cx: &mut Context<Self>) {
        if crate::updater::build_number(env!("CARGO_PKG_VERSION")).is_none() {
            self.updates.unavailable =
                Some("Automatic updates are available in installed releases of Muxy Beta".into());
            return;
        }
        let record = self.path.with_file_name("pending-update.json");
        let detect = cx.background_executor().spawn(async move {
            let installation = Installation::detect()?;
            let pending = installation.restore(&record);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((installation, pending))
        });
        self.updates.task = Some(cx.spawn(async move |model, cx| {
            let result = detect.await;
            let _ = model.update(cx, |model, cx| match result {
                Ok((installation, pending)) => {
                    match pending {
                        Ok(Some((update, scheduled))) => { model.updates.ready = Some(update); model.updates.scheduled = scheduled; }
                        Ok(None) => {},
                        Err(error) => model.fail(format!("Could not restore the pending update: {error}. Check for updates to retry."), cx),
                    }
                    model.updates.installation = Some(installation);
                    if let Some(server) = model.updates.server.clone() { model.receive_server_info(server, cx); }
                    model.check_for_updates(false, cx);
                    model.start_server_update_poll(cx);
                    model.updates.poll = Some(cx.spawn(async move |model, cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_secs(3600))
                                .await;
                            if model
                                .update(cx, |model, cx| model.check_for_updates(false, cx))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }));
                }
                Err(error) => model.updates.unavailable = Some(error.to_string()),
            });
        }));
    }

    pub(super) fn expect_server_restart(&mut self) {
        if self.quitting == Quitting::Idle && !self.server_preferences.control_busy {
            self.updates.phase = ServerUpdatePhase::Notified;
        }
    }

    pub(super) fn receive_disconnect(&mut self, cx: &mut Context<Self>) {
        let notified = self.updates.phase == ServerUpdatePhase::Notified;
        let reconnect =
            notified && !self.server_preferences.control_busy && self.quitting == Quitting::Idle;
        if notified {
            self.updates.phase = ServerUpdatePhase::Idle;
        }
        self.disconnect(cx);
        if reconnect {
            self.updates.phase = ServerUpdatePhase::Reconnecting;
            self.connect_to_server(true, cx);
        }
    }

    pub(super) fn update_connect_failed(&mut self) {
        if self.updates.replacing() {
            self.updates.phase = ServerUpdatePhase::Failed;
            self.updates.server_error = Some("Could not reconnect to the updated server.".into());
        }
    }

    pub(super) fn server_update_control_started(&mut self, restart: bool) {
        if restart {
            self.updates.phase = ServerUpdatePhase::Replacing;
            self.updates.server_error = None;
        }
    }

    pub(super) fn server_update_control_finished(
        &mut self,
        result: &Result<(), muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if !self.updates.replacing() {
            return;
        }
        match result {
            Ok(()) => self.updates.phase = ServerUpdatePhase::Reconnecting,
            Err(error) => {
                self.updates.phase = ServerUpdatePhase::Failed;
                let message = format!("Could not restart the server: {error}");
                self.updates.server_error = Some(message.clone());
                self.fail(message, cx);
            }
        }
    }

    pub(super) fn resume_update_attaches(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready || self.updates.replacing() {
            return;
        }
        for (pane, size) in std::mem::take(&mut self.updates.queued_attaches) {
            if !self.pending.contains(&pane) && self.pane_session(pane).is_none() {
                self.start_attach(pane, size, cx);
            }
        }
    }

    pub(crate) fn server_update_pending(&self) -> bool {
        self.updates
            .server
            .as_ref()
            .is_some_and(|server| crate::server::newer_build(&server.build.version))
    }

    pub(super) fn server_update_description(&self) -> Option<String> {
        if self.connection != ConnectionState::Ready {
            return None;
        }
        self.updates.server.as_ref().map(|server| {
            if self.server_update_pending() {
                format!("Server {} · Update {} pending. It will restart when all terminal sessions end. Restart Server applies it now and ends running sessions.", server.build.version, env!("CARGO_PKG_VERSION"))
            } else { format!("Server {}", server.build.version) }
        })
    }

    fn start_server_update_poll(&mut self, cx: &mut Context<Self>) {
        self.updates.reconcile = Some(cx.spawn(async move |model, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if model.update(cx, AppModel::reconcile_server_update).is_err() {
                    break;
                }
            }
        }));
        self.reconcile_server_update(cx);
    }

    pub(super) fn reconcile_server_update(&mut self, cx: &mut Context<Self>) {
        if (self.updates.installation.is_none() && !self.updates.scheduled)
            || self.connection != ConnectionState::Ready
            || self.quitting != Quitting::Idle
            || self.updates.phase != ServerUpdatePhase::Idle
            || self.close_prompt.is_some()
            || self.updates.checking()
            || (self.updates.scheduled && self.updates.app_error.is_some())
            || !self.pending.is_empty()
            || !self.discarding.is_empty()
            || self.server_preferences.control_busy
            || self.server_preferences.busy
        {
            return;
        }
        if self.updates.retry_preparation.take() == Some(self.generation) {
            self.begin_update(cx);
            return;
        }
        let replace = self.server_update_pending() && !self.updates.scheduled;
        self.updates.phase = if replace {
            ServerUpdatePhase::Replacing
        } else {
            ServerUpdatePhase::Checking
        };
        if !self.send(
            Work::CheckServerUpdate {
                socket: self.path.with_file_name("server.sock"),
                replace,
            },
            cx,
        ) {
            self.updates.phase = ServerUpdatePhase::Idle;
        }
    }

    pub(super) fn receive_server_info(
        &mut self,
        server: muxy_protocol::ServerInfo,
        cx: &mut Context<Self>,
    ) {
        self.updates.server = Some(server);
        self.updates.phase = ServerUpdatePhase::Idle;
        self.updates.server_error = None;
        if let Some(installation) = self.updates.installation.clone() {
            let socket = self.path.with_file_name("server.sock");
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = installation.cleanup(&socket) {
                        let _ = writeln!(
                            std::io::stderr(),
                            "Could not clean up retired update: {error}"
                        );
                    }
                })
                .detach();
        }
        self.sync_preferences(cx);
    }

    pub(super) fn receive_server_update(
        &mut self,
        result: Result<ServerUpdate, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if self.updates.phase != ServerUpdatePhase::Failed {
            self.updates.phase = ServerUpdatePhase::Idle;
        }
        match result {
            Ok(status) => {
                self.updates.sessions = status.sessions;
                self.updates.server = Some(status.server);
                if status.replaced {
                    self.updates.phase = ServerUpdatePhase::Reconnecting;
                    self.disconnect(cx);
                    self.connect_to_server(true, cx);
                } else if self.updates.scheduled
                    && status.sessions == 0
                    && self.close_prompt.is_none()
                    && !self.updates.checking()
                    && self.updates.app_error.is_none()
                {
                    self.updates.mode = Some(UpdateMode::WhenIdle);
                    self.begin_update(cx);
                } else {
                    self.ensure_visible(cx);
                }
            }
            Err(error) => {
                self.updates.phase = ServerUpdatePhase::Failed;
                let message = format!("Could not update the server: {error}");
                self.updates.server_error = Some(message.clone());
                self.fail(message, cx);
                self.ensure_visible(cx);
            }
        }
        self.resume_update_attaches(cx);
        self.sync_preferences(cx);
        cx.notify();
    }

    pub(crate) fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || self.close_prompt.is_some() {
            return;
        }
        self.updates.manual |= manual;
        if self.updates.checking() {
            return;
        }
        let Some(installation) = self.updates.installation.clone() else {
            if manual {
                self.update_message(
                    self.updates
                        .unavailable
                        .as_deref()
                        .unwrap_or("The updater is starting. Try again shortly."),
                    cx,
                );
            }
            self.updates.manual = false;
            return;
        };
        self.updates.download = AppUpdatePhase::Checking;
        self.updates.app_error = None;
        let check = cx
            .background_executor()
            .spawn(async move { Installation::latest() });
        self.updates.task = Some(cx.spawn(async move |model, cx| {
            let result = check.await;
            let release = model.update(cx, |model, cx| model.receive_update_release(result, cx));
            let Ok(Some(release)) = release else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { installation.prepare(release) })
                .await;
            let _ = model.update(cx, |model, cx| {
                model.finish_update_check(result.map(Some), cx);
            });
        }));
        cx.notify();
    }

    fn receive_update_release(
        &mut self,
        result: crate::updater::Result<Option<Release>>,
        cx: &mut Context<Self>,
    ) -> Option<Release> {
        match result {
            Ok(Some(release))
                if self
                    .updates
                    .ready
                    .as_ref()
                    .is_none_or(|ready| release.is_newer_than(&ready.version)) =>
            {
                self.updates.download = AppUpdatePhase::Downloading;
                cx.notify();
                Some(release)
            }
            Ok(_) => {
                self.finish_update_check(Ok(None), cx);
                None
            }
            Err(error) => {
                self.finish_update_check(Err(error), cx);
                None
            }
        }
    }

    fn finish_update_check(
        &mut self,
        result: crate::updater::Result<Option<PreparedUpdate>>,
        cx: &mut Context<Self>,
    ) {
        self.updates.download = AppUpdatePhase::Idle;
        let manual = std::mem::take(&mut self.updates.manual);
        let result = result.and_then(|update| {
            let Some(update) = update else {
                return Ok(());
            };
            if let Err(error) = update.persist(
                &self.path.with_file_name("pending-update.json"),
                self.updates.scheduled,
            ) {
                self.discard_update(update, cx);
                return Err(error);
            }
            if let Some(previous) = self.updates.ready.replace(update) {
                self.discard_update(previous, cx);
            }
            self.updates.retry_preparation = None;
            self.updates.mode = None;
            Ok(())
        });
        match result {
            Ok(()) => {
                self.updates.app_error = None;
                if manual {
                    if self.updates.ready.is_some() {
                        self.confirm_update(cx);
                    } else {
                        self.update_message("You’re running the latest Muxy 2.x beta.", cx);
                    }
                }
            }
            Err(error) => {
                let message =
                    format!("Could not check for or download the latest app update: {error}");
                self.updates.app_error = Some(message.clone());
                if manual {
                    self.update_message(&message, cx);
                }
            }
        }
        cx.notify();
    }

    fn discard_update(&self, update: PreparedUpdate, cx: &Context<Self>) {
        let socket = self.path.with_file_name("server.sock");
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = update.discard(&socket) {
                    let _ = writeln!(
                        std::io::stderr(),
                        "Could not remove superseded update: {error}"
                    );
                }
            })
            .detach();
    }

    fn update_message(&self, message: &str, cx: &mut Context<Self>) {
        let _ = self.window.update(cx, |_, window, cx| {
            let response = window.prompt(
                gpui::PromptLevel::Info,
                "Muxy Beta Updates",
                Some(message),
                &["OK"],
                cx,
            );
            cx.spawn(async move |_| {
                let _ = response.await;
            })
            .detach();
        });
    }

    pub(super) fn confirm_update(&mut self, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() || self.quitting != Quitting::Idle || self.updates.checking()
        {
            return;
        }
        let Some(update) = &self.updates.ready else {
            return;
        };
        self.updates.retry_preparation = None;
        let version = update.version.clone();
        let compatible = self
            .updates
            .server
            .as_ref()
            .is_some_and(|server| update.compatible_with(server));
        let scheduled = self.updates.scheduled;
        let sessions = self.updates.sessions;
        let window = self.window;
        let generation = self.generation;
        self.dismiss_overlay(cx);
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = crate::views::confirm::prompt_update(
                window, &version, compatible, scheduled, sessions, cx,
            )
            .await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                if model.generation != generation
                    || model
                        .updates
                        .ready
                        .as_ref()
                        .is_none_or(|update| update.version != version)
                {
                    return;
                }
                match response {
                    Ok(crate::views::confirm::UpdateChoice::Install) => {
                        model.updates.mode = Some(UpdateMode::Preserve);
                        model.begin_update(cx);
                    }
                    Ok(crate::views::confirm::UpdateChoice::Schedule) => {
                        model.schedule_update(true, cx);
                    }
                    Ok(crate::views::confirm::UpdateChoice::CancelSchedule) => {
                        model.schedule_update(false, cx);
                    }
                    Ok(crate::views::confirm::UpdateChoice::EndSessions) => {
                        model.updates.mode = Some(UpdateMode::EndSessions);
                        model.begin_update(cx);
                    }
                    Ok(crate::views::confirm::UpdateChoice::Later) => {}
                    Err(error) => model.fail(error, cx),
                }
            });
        }));
    }

    fn schedule_update(&mut self, scheduled: bool, cx: &mut Context<Self>) {
        let Some(update) = &self.updates.ready else {
            return;
        };
        match update.persist(&self.path.with_file_name("pending-update.json"), scheduled) {
            Ok(()) => {
                self.updates.scheduled = scheduled;
                if scheduled {
                    self.updates.phase = ServerUpdatePhase::Idle;
                } else {
                    self.updates.manual = false;
                    self.updates.retry_preparation = None;
                    self.updates.mode = None;
                }
                self.reconcile_server_update(cx);
                cx.notify();
            }
            Err(error) => self.fail(format!("Could not save the update schedule: {error}"), cx),
        }
    }

    pub(super) fn begin_update(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle
            || self.updates.ready.is_none()
            || self.updates.checking()
            || self.updates.replacing()
            || self.server_preferences.control_busy
            || !self.preferences_before_quit(cx)
        {
            return;
        }
        if self.connection != ConnectionState::Ready {
            self.fail("Connect to the server before installing so Muxy can check whether sessions can be preserved".into(), cx);
            return;
        }
        if self.updates.server.is_none() {
            return;
        }
        self.updates.app_error = None;
        self.quitting = Quitting::Update;
        if !self.send(Work::Flush, cx) {
            self.quitting = Quitting::Idle;
        }
        cx.notify();
    }

    pub(super) fn flush_before_update(&mut self, cx: &mut Context<Self>) {
        if !self.pending.is_empty() || !self.discarding.is_empty() {
            if !self.send(Work::Flush, cx) {
                self.quitting = Quitting::Idle;
            }
            return;
        }
        let (Some(update), Some(server)) =
            (self.updates.ready.clone(), self.updates.server.clone())
        else {
            self.quitting = Quitting::Idle;
            return;
        };
        if !self.save(cx)
            || !self.send(
                Work::PrepareUpdate {
                    socket: self.path.with_file_name("server.sock"),
                    update,
                    mode: self.updates.mode.unwrap_or(UpdateMode::Preserve),
                    server,
                },
                cx,
            )
        {
            self.quitting = Quitting::Idle;
        }
    }

    pub(super) fn receive_update_prepared(
        &mut self,
        result: Result<Option<std::fs::File>, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if self.quitting != Quitting::Update {
            return;
        }
        let lock = match result {
            Ok(Some(lock)) => lock,
            Ok(None) => {
                self.quitting = Quitting::Idle;
                if self.updates.mode != Some(UpdateMode::WhenIdle) {
                    self.updates.retry_preparation = Some(self.generation);
                }
                self.ensure_visible(cx);
                cx.notify();
                return;
            }
            Err(error) => {
                self.quitting = Quitting::Idle;
                self.updates.app_error = Some(format!("Could not prepare the app update: {error}"));
                if matches!(&error, muxy_client::ClientError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData)
                {
                    self.updates.ready = None;
                    self.updates.scheduled = false;
                    let _ = std::fs::remove_file(self.path.with_file_name("pending-update.json"));
                }
                self.fail(
                    format!("Could not prepare the update: {error}. Check for updates to retry."),
                    cx,
                );
                return;
            }
        };
        self.disconnect(cx);
        let (Some(update), Some(server)) =
            (self.updates.ready.clone(), self.updates.server.clone())
        else {
            self.quitting = Quitting::Idle;
            return;
        };
        let record = self.path.with_file_name("pending-update.json");
        let install = cx.background_executor().spawn(async move {
            update.install(lock, &server)?;
            let _ = std::fs::remove_file(record);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });
        self.updates.task = Some(cx.spawn(async move |model, cx| {
            let result = install.await;
            let _ = model.update(cx, |model, cx| match result {
                Ok(()) => {
                    cx.quit();
                }
                Err(error) => {
                    model.quitting = Quitting::Idle;
                    model.updates.app_error =
                        Some(format!("Could not install the app update: {error}"));
                    model.connect(cx);
                    model.fail(format!("Could not install the beta: {error}"), cx);
                }
            });
        }));
    }
}
