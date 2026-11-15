use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use muxy_protocol::ChannelId;

type Pending = Mutex<Option<Contents>>;

#[derive(Debug)]
struct Contents {
    sequence: u64,
    text: String,
}

#[derive(Clone, Debug)]
pub struct ClipboardWrite {
    pending: Arc<Pending>,
    consumed: Arc<AtomicU64>,
}

impl ClipboardWrite {
    pub fn take(self) -> Option<String> {
        let contents = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        (self
            .consumed
            .fetch_max(contents.sequence, Ordering::Relaxed)
            < contents.sequence)
            .then_some(contents.text)
    }
}

impl PartialEq for ClipboardWrite {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pending, &other.pending)
    }
}

impl Eq for ClipboardWrite {}

const SWEEP_FLOOR: usize = 16;

#[derive(Default)]
pub(super) struct ClipboardWrites {
    pending: HashMap<ChannelId, Weak<Pending>>,
    attached: HashSet<ChannelId>,
    sweep_at: usize,
    sequence: u64,
    consumed: Arc<AtomicU64>,
}

impl ClipboardWrites {
    pub(super) fn attached(&mut self, channel: ChannelId) {
        self.attached.insert(channel);
    }

    pub(super) fn push(&mut self, channel: ChannelId, text: String) -> Option<ClipboardWrite> {
        if !self.attached.contains(&channel) {
            return None;
        }
        self.sequence += 1;
        let contents = Contents {
            sequence: self.sequence,
            text,
        };
        if let Some(pending) = self.pending.get(&channel).and_then(Weak::upgrade) {
            let mut pending = pending.lock().unwrap_or_else(PoisonError::into_inner);
            if pending.is_some() {
                *pending = Some(contents);
                return None;
            }
        }
        if self.pending.len() >= self.sweep_at {
            self.pending.retain(|_, pending| pending.strong_count() > 0);
            self.sweep_at = (self.pending.len() * 2).max(SWEEP_FLOOR);
        }
        let write = ClipboardWrite {
            pending: Arc::new(Mutex::new(Some(contents))),
            consumed: self.consumed.clone(),
        };
        self.pending.insert(channel, Arc::downgrade(&write.pending));
        Some(write)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;

    #[test]
    fn consuming_during_a_write_never_loses_the_latest_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut pending = ClipboardWrites::default();
        pending.attached(ChannelId(1));
        for _ in 0..128 {
            let write = pending
                .push(ChannelId(1), "before".into())
                .ok_or("missing initial notification")?;
            let stale = write.clone();
            let barrier = Barrier::new(2);
            let (taken, next) = std::thread::scope(|scope| {
                let consumer = scope.spawn(|| {
                    barrier.wait();
                    write.take()
                });
                barrier.wait();
                let next = pending.push(ChannelId(1), "after".into());
                (consumer.join(), next)
            });
            let taken = taken.map_err(|_| "clipboard consumer panicked")?;
            assert_eq!(stale.take(), None);
            match next {
                Some(next) => {
                    assert_eq!(taken.as_deref(), Some("before"));
                    assert_eq!(next.take().as_deref(), Some("after"));
                }
                None => assert_eq!(taken.as_deref(), Some("after")),
            }
        }
        Ok(())
    }

    #[test]
    fn discarded_channels_do_not_retain_clipboard_payloads_or_notifications()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut pending = ClipboardWrites::default();
        for channel in 0..1024 {
            pending.attached(ChannelId(channel));
            let write = pending
                .push(ChannelId(channel), "discarded".into())
                .ok_or("missing notification")?;
            let contents = Arc::downgrade(&write.pending);
            drop(write);
            assert!(contents.upgrade().is_none());
            assert!(
                pending.pending.len() <= SWEEP_FLOOR,
                "{}",
                pending.pending.len()
            );
        }
        let write = pending
            .push(ChannelId(1023), "next".into())
            .ok_or("dropped notification suppressed a later write")?;
        assert_eq!(write.take().as_deref(), Some("next"));
        Ok(())
    }

    #[test]
    fn delayed_channels_cannot_overwrite_a_newer_consumed_clipboard()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut pending = ClipboardWrites::default();
        pending.attached(ChannelId(1));
        pending.attached(ChannelId(2));
        let first = pending
            .push(ChannelId(1), "first".into())
            .ok_or("missing first notification")?;
        let second = pending
            .push(ChannelId(2), "second".into())
            .ok_or("missing second notification")?;
        assert!(pending.push(ChannelId(1), "latest".into()).is_none());
        assert_eq!(first.take().as_deref(), Some("latest"));
        assert_eq!(second.take(), None);
        let next = pending
            .push(ChannelId(2), "next".into())
            .ok_or("missing next notification")?;
        assert_eq!(next.take().as_deref(), Some("next"));
        Ok(())
    }

    #[test]
    fn only_attached_channels_write_the_clipboard() -> Result<(), Box<dyn std::error::Error>> {
        let mut pending = ClipboardWrites::default();
        for channel in 1..10_000 {
            assert!(pending.push(ChannelId(channel), "forged".into()).is_none());
        }
        assert!(pending.pending.is_empty());
        pending.attached(ChannelId(7));
        let write = pending
            .push(ChannelId(7), "copied".into())
            .ok_or("attached channel lost its write")?;
        assert_eq!(write.take().as_deref(), Some("copied"));
        Ok(())
    }

    #[test]
    fn held_notifications_are_not_rescanned_on_every_new_channel() {
        let mut pending = ClipboardWrites::default();
        let mut held = Vec::new();
        for channel in 0..4096 {
            pending.attached(ChannelId(channel));
            held.extend(pending.push(ChannelId(channel), "held".into()));
            assert!(pending.sweep_at >= pending.pending.len());
            assert!(pending.sweep_at <= (2 * pending.pending.len()).max(SWEEP_FLOOR));
        }
        assert_eq!(held.len(), 4096);
        assert_eq!(pending.pending.len(), 4096);
    }
}
