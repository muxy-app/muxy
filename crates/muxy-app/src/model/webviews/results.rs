use std::collections::{HashMap, HashSet};

use async_channel::{Receiver, Sender};
use gpui::EntityId;
use serde_json::Value;

const MAX_UNCONSUMED: usize = 128;

struct Pending {
    owner: (EntityId, u64),
    result: Receiver<Value>,
}

#[derive(Default)]
pub(super) struct Results(HashMap<String, Pending>);

impl Results {
    pub(super) fn reserve(
        &mut self,
        id: String,
        owner: EntityId,
        generation: u64,
    ) -> Result<Sender<Value>, String> {
        self.check_capacity()?;
        let (sender, result) = async_channel::bounded(1);
        self.0.insert(
            id,
            Pending {
                owner: (owner, generation),
                result,
            },
        );
        Ok(sender)
    }

    pub(super) fn check_capacity(&self) -> Result<(), String> {
        if self.0.len() >= MAX_UNCONSUMED {
            Err("too many unconsumed modal results".into())
        } else {
            Ok(())
        }
    }

    pub(super) fn take(
        &mut self,
        id: &str,
        owner: EntityId,
        generation: u64,
    ) -> Result<Receiver<Value>, String> {
        if self
            .0
            .get(id)
            .is_none_or(|pending| pending.owner != (owner, generation))
        {
            return Err(
                "modal result does not belong to this document or was already consumed".into(),
            );
        }
        self.0
            .remove(id)
            .map(|pending| pending.result)
            .ok_or_else(|| "modal result already consumed".into())
    }

    pub(super) fn retain(&mut self, live: impl Iterator<Item = (EntityId, u64)>) {
        let live: HashSet<_> = live.collect();
        self.0.retain(|_, pending| live.contains(&pending.owner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;

    #[gpui::test]
    fn replacement_before_await_preserves_cancellation_and_scope(cx: &mut gpui::TestAppContext) {
        let first_owner = cx.new(|_| ()).entity_id();
        let other_owner = cx.new(|_| ()).entity_id();
        let mut results = Results::default();
        let old = results
            .reserve("first".into(), first_owner, 1)
            .expect("first slot");
        let current = results
            .reserve("second".into(), first_owner, 1)
            .expect("second slot");
        old.try_send(Value::Null).expect("replacement cancellation");
        assert!(results.take("first", other_owner, 1).is_err());
        assert!(results.take("first", first_owner, 2).is_err());
        assert_eq!(
            results
                .take("first", first_owner, 1)
                .expect("late await")
                .try_recv()
                .expect("cancelled"),
            Value::Null
        );
        assert!(results.take("first", first_owner, 1).is_err());
        current.try_send(Value::Bool(true)).expect("submission");
        assert_eq!(
            results
                .take("second", first_owner, 1)
                .expect("current await")
                .try_recv()
                .expect("result"),
            Value::Bool(true)
        );
    }
}
