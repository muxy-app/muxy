use crate::async_request::Response;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use muxy_protocol::{ReplyBody, RequestId};

use crate::ClientError;

#[derive(Default)]
struct State {
    next: u32,
    waiting: HashMap<RequestId, Sender<ReplyBody>>,
    asynchronous: HashMap<RequestId, (Arc<Response>, Instant)>,
    closed: bool,
}

#[derive(Default)]
pub(crate) struct Pending {
    state: Mutex<State>,
    deadlines: Condvar,
}

impl Pending {
    pub(crate) fn register(&self) -> Result<(RequestId, Receiver<ReplyBody>), ClientError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ClientError::Disconnected);
        }
        let id = loop {
            let id = RequestId(state.next);
            state.next = state.next.wrapping_add(1);
            if !state.waiting.contains_key(&id) && !state.asynchronous.contains_key(&id) {
                break id;
            }
        };
        let (sender, receiver) = mpsc::channel();
        state.waiting.insert(id, sender);
        Ok((id, receiver))
    }

    pub(crate) fn resolve(&self, id: RequestId, body: ReplyBody) {
        let mut state = self.lock();
        let asynchronous = state.asynchronous.remove(&id);
        let sender = state.waiting.remove(&id);
        drop(state);
        if let Some((response, _)) = asynchronous {
            response.resolve(match body {
                ReplyBody::Error(error) => Err(ClientError::Server(error)),
                body => Ok(body),
            });
        } else if let Some(sender) = sender {
            let _ = sender.send(body);
        }
    }

    pub(crate) fn forget(&self, id: RequestId) {
        let mut state = self.lock();
        state.waiting.remove(&id);
        state.asynchronous.remove(&id);
    }

    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.deadlines.notify_one();
        state.waiting.clear();
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
        let id = loop {
            let id = RequestId(state.next);
            state.next = state.next.wrapping_add(1);
            if !state.waiting.contains_key(&id) && !state.asynchronous.contains_key(&id) {
                break id;
            }
        };
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc::TryRecvError;

    use super::*;

    #[test]
    fn replies_reach_their_request_and_closing_fails_the_rest() -> Result<(), ClientError> {
        let pending = Pending::default();
        let (first, first_reply) = pending.register()?;
        let (second, second_reply) = pending.register()?;
        assert_ne!(first, second);
        pending.resolve(second, ReplyBody::Pong);
        pending.resolve(RequestId(99), ReplyBody::Detached);
        assert_eq!(second_reply.try_recv(), Ok(ReplyBody::Pong));
        assert_eq!(first_reply.try_recv(), Err(TryRecvError::Empty));
        pending.close();
        assert_eq!(first_reply.try_recv(), Err(TryRecvError::Disconnected));
        assert!(matches!(pending.register(), Err(ClientError::Disconnected)));
        Ok(())
    }

    #[test]
    fn forgotten_requests_ignore_late_replies() -> Result<(), ClientError> {
        let pending = Pending::default();
        let (id, reply) = pending.register()?;
        pending.forget(id);
        pending.resolve(id, ReplyBody::Pong);
        assert_eq!(reply.try_recv(), Err(TryRecvError::Disconnected));
        Ok(())
    }
}
