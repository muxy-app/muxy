use gpui::{Context, Task};
use muxy_app_core::ServerId;
use muxy_client::{Backoff, RemoteReason};

use super::{AppModel, ConnectionState, Quitting};

#[derive(Default)]
pub(crate) struct Retry {
    attempts: u32,
    timer: Option<Task<()>>,
    pub(super) held: bool,
}

impl Retry {
    pub(super) fn waiting(&self) -> bool {
        self.timer.is_some()
    }

    pub(super) fn connecting(&mut self) {
        self.timer = None;
        self.held = false;
    }

    pub(super) fn hold(&mut self) {
        self.timer = None;
        self.held = true;
    }

    pub(super) fn connected(&mut self) {
        self.attempts = 0;
        self.timer = None;
    }
}

impl AppModel {
    pub(super) fn schedule_reconnect(
        &mut self,
        server: ServerId,
        reason: Option<RemoteReason>,
        cx: &mut Context<Self>,
    ) {
        if server.is_local()
            || self.quitting != Quitting::Idle
            || reason.is_some_and(|reason| !reason.recoverable())
            || self.server_needs_password(server)
        {
            return;
        }
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        if runtime.retry.held || runtime.stopping || runtime.work.is_none() {
            return;
        }
        let wait = Backoff::Background.delay(runtime.retry.attempts);
        runtime.retry.attempts = runtime.retry.attempts.saturating_add(1);
        let generation = runtime.generation;
        runtime.retry.timer = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(wait).await;
            let _ = model.update(cx, |model, cx| model.reconnect_now(server, generation, cx));
        }));
    }

    fn reconnect_now(&mut self, server: ServerId, generation: u64, cx: &mut Context<Self>) {
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.retry.timer = None;
        if runtime.generation == generation
            && runtime.connection == ConnectionState::Disconnected
            && !runtime.retry.held
        {
            self.connect_server(server, cx);
        }
    }

    pub(super) fn reconnect_waiting(&mut self, cx: &mut Context<Self>) {
        for server in self.servers.ids() {
            if let Some(runtime) = self.servers.get(server)
                && runtime.retry.waiting()
            {
                let generation = runtime.generation;
                self.reconnect_now(server, generation, cx);
            }
        }
    }

    pub(super) fn server_needs_password(&self, server: ServerId) -> bool {
        self.servers
            .get(server)
            .is_some_and(|runtime| runtime.password_login)
            && !self.servers.passwords.has(server)
    }
}
