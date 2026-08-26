use std::collections::{HashSet, VecDeque};
use std::os::unix::net::UnixStream;

use crate::framing::InputAccumulator;

pub(super) struct Session {
    pub(super) stream: UnixStream,
    pub(super) input: InputAccumulator,
    pub(super) writes: VecDeque<u8>,
    pub(super) read_eof: bool,
    pub(super) close_after_flush: bool,
    pub(super) pending_requests: usize,
    pub(super) extension_id: Option<String>,
    pub(super) subscriptions: HashSet<String>,
    pub(super) dropped_notifications: usize,
}

impl Session {
    pub(super) fn new(stream: UnixStream, max_input_bytes: usize) -> Self {
        Self {
            stream,
            input: InputAccumulator::new(max_input_bytes),
            writes: VecDeque::new(),
            read_eof: false,
            close_after_flush: false,
            pending_requests: 0,
            extension_id: None,
            subscriptions: HashSet::new(),
            dropped_notifications: 0,
        }
    }

    pub(super) fn enqueue(&mut self, bytes: impl IntoIterator<Item = u8>) {
        self.writes.extend(bytes);
    }

    pub(super) fn can_close(&self) -> bool {
        self.pending_requests == 0
            && self.writes.is_empty()
            && (self.read_eof || self.close_after_flush)
    }
}
