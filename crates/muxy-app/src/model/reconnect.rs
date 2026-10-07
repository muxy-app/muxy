//! Another computer's server connects again on its own after a drop, or
//! after an attempt that failed in a way the network or that computer can
//! recover from. This computer's server connects only when needed, as before.

use std::time::Duration;

use gpui::{Context, Task};
use muxy_app_core::ServerId;
use muxy_client::RemoteReason;

use super::{AppModel, ConnectionState, Quitting};

#[derive(Default)]
pub(crate) struct Retry {
    /// Attempts that failed in a row since the last connection.
    attempts: u32,
    /// Waits for the next attempt.
    timer: Option<Task<()>>,
    /// The user stopped the server there; it waits for Connect.
    pub(super) held: bool,
}

impl Retry {
    pub(super) fn waiting(&self) -> bool {
        self.timer.is_some()
    }

    /// Connecting now, by hand or after a wait: a wait still running is
    /// over, and a server the user stopped may connect again.
    pub(super) fn connecting(&mut self) {
        self.timer = None;
        self.held = false;
    }

    /// A connection came: the next drop starts waiting from the shortest.
    pub(super) fn connected(&mut self) {
        self.attempts = 0;
        self.timer = None;
    }
}

/// How long to wait before an attempt after `attempts` failed in a row:
/// 2 s, doubling up to a minute, which then repeats. Backing off spares the
/// other computer's sshd and keeps tools like fail2ban from blocking this one.
pub(super) fn delay(attempts: u32) -> Duration {
    Duration::from_secs((2_u64 << attempts.min(5)).min(60))
}

impl AppModel {
    /// Connects to another computer again after a wait, unless `reason` says
    /// the user must fix something first, such as a refused login or an
    /// untrusted host key, or the user stopped its server.
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
        let wait = delay(runtime.retry.attempts);
        runtime.retry.attempts = runtime.retry.attempts.saturating_add(1);
        let generation = runtime.generation;
        runtime.retry.timer = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(wait).await;
            let _ = model.update(cx, |model, cx| model.reconnect_now(server, generation, cx));
        }));
    }

    /// Connects now if nothing connected the server since the wait began.
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

    /// Coming back to the app, such as after sleep, often means the network
    /// is back, so servers waiting to reconnect try now.
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

    /// It logs in with a password that hasn't been typed this session.
    pub(super) fn server_needs_password(&self, server: ServerId) -> bool {
        self.servers
            .get(server)
            .is_some_and(|runtime| runtime.password_login)
            && !self.servers.passwords.has(server)
    }
}
