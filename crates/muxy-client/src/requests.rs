use crate::async_request::Response;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use muxy_protocol::{ChannelId, ReplyBody, RequestId, SessionId};

use crate::ClientError;

#[allow(
    clippy::large_enum_variant,
    reason = "A reply moves once, unboxed, as replies always have"
)]
pub(crate) enum Reply {
    Body(ReplyBody),
    /// The snapshot went to the event stream as `ClientEvent::Attached`.
    Attached(ChannelId),
}

/// Who asked for an attach, so its reply can go to the event stream.
#[derive(Clone, Copy)]
pub(crate) struct Attaching {
    pub(crate) tag: u64,
    pub(crate) session: SessionId,
}

#[derive(Default)]
struct State {
    next: u32,
    waiting: HashMap<RequestId, Sender<Reply>>,
    /// Kept after the caller stops waiting, so a late reply still reaches the event stream.
    attaching: HashMap<RequestId, Attaching>,
    asynchronous: HashMap<RequestId, (Arc<Response>, Instant)>,
    closed: bool,
}

impl State {
    fn next_id(&mut self) -> RequestId {
        loop {
            let id = RequestId(self.next);
            self.next = self.next.wrapping_add(1);
            if !self.waiting.contains_key(&id)
                && !self.attaching.contains_key(&id)
                && !self.asynchronous.contains_key(&id)
            {
                return id;
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Pending {
    state: Mutex<State>,
    deadlines: Condvar,
}

impl Pending {
    pub(crate) fn register(&self) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        self.register_waiting(None)
    }

    pub(crate) fn register_attach(
        &self,
        attaching: Attaching,
    ) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        self.register_waiting(Some(attaching))
    }

    fn register_waiting(
        &self,
        attaching: Option<Attaching>,
    ) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ClientError::Disconnected);
        }
        let id = state.next_id();
        let (sender, receiver) = mpsc::channel();
        state.waiting.insert(id, sender);
        if let Some(attaching) = attaching {
            state.attaching.insert(id, attaching);
        }
        Ok((id, receiver))
    }

    /// Who asked for this attach; the reply is then theirs to announce.
    pub(crate) fn take_attaching(&self, id: RequestId) -> Option<Attaching> {
        self.lock().attaching.remove(&id)
    }

    pub(crate) fn resolve(&self, id: RequestId, body: ReplyBody) {
        let mut state = self.lock();
        let asynchronous = state.asynchronous.remove(&id);
        let sender = state.waiting.remove(&id);
        state.attaching.remove(&id);
        drop(state);
        if let Some((response, _)) = asynchronous {
            response.resolve(match body {
                ReplyBody::Error(error) => Err(ClientError::Server(error)),
                body => Ok(body),
            });
        } else if let Some(sender) = sender {
            let _ = sender.send(Reply::Body(body));
        }
    }

    pub(crate) fn resolve_attached(&self, id: RequestId, channel: ChannelId) {
        if let Some(sender) = self.lock().waiting.remove(&id) {
            let _ = sender.send(Reply::Attached(channel));
        }
    }

    /// Stops waiting for a reply. An attach reply still reaches the event stream.
    pub(crate) fn forget(&self, id: RequestId) {
        let mut state = self.lock();
        state.waiting.remove(&id);
        state.asynchronous.remove(&id);
    }

    /// Forgets a request the server never received.
    pub(crate) fn abandon(&self, id: RequestId) {
        let mut state = self.lock();
        state.waiting.remove(&id);
        state.asynchronous.remove(&id);
        state.attaching.remove(&id);
    }

    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.deadlines.notify_one();
        state.waiting.clear();
        state.attaching.clear();
        let responses = std::mem::take(&mut state.asynchronous);
        drop(state);
        for (response, _) in responses.into_values() {
            response.resolve(Err(ClientError::Disconnected));
        }
    }

    pub(crate) fn register_async(
        &self,
        timeout: Duration,
    ) -> Result<(RequestId, Arc<Response>), ClientError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ClientError::Disconnected);
        }
        if state.asynchronous.len() >= 128 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "too many pending asynchronous requests",
            )
            .into());
        }
        let id = state.next_id();
        let response = Arc::new(Response::default());
        state
            .asynchronous
            .insert(id, (response.clone(), Instant::now() + timeout));
        self.deadlines.notify_one();
        Ok((id, response))
    }

    pub(crate) fn contains_async(&self, id: RequestId) -> bool {
        self.lock().asynchronous.contains_key(&id)
    }

    pub(crate) fn fail_async(&self, id: RequestId, error: ClientError) {
        let response = self.lock().asynchronous.remove(&id);
        if let Some((response, _)) = response {
            response.resolve(Err(error));
        }
    }

    pub(crate) fn watch_deadlines(&self) {
        loop {
            let mut state = self.lock();
            while !state.closed {
                let next = state
                    .asynchronous
                    .values()
                    .map(|(_, deadline)| *deadline)
                    .min();
                if next.is_some_and(|deadline| deadline <= Instant::now()) {
                    break;
                }
                state = if let Some(deadline) = next {
                    self.deadlines
                        .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                } else {
                    self.deadlines
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner)
                };
            }
            if state.closed {
                return;
            }
            drop(state);
            self.expire();
        }
    }

    pub(crate) fn expire(&self) {
        let now = Instant::now();
        let mut state = self.lock();
        let expired: Vec<_> = state
            .asynchronous
            .iter()
            .filter(|(_, (_, deadline))| *deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        let responses: Vec<_> = expired
            .into_iter()
            .filter_map(|id| state.asynchronous.remove(&id))
            .collect();
        drop(state);
        for (response, _) in responses {
            response.resolve(Err(ClientError::Timeout));
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
