use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::OperationId;
use serde::{Deserialize, Serialize};

use super::State;

const RECORD_MILLIS: u64 = 24 * 60 * 60 * 1000;
const MAX_RECORDS: usize = 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Finished {
    #[serde(default)]
    pub(super) creations: BTreeMap<OperationId, u64>,
    #[serde(default)]
    pub(super) cancelled: BTreeMap<OperationId, u64>,
    #[serde(default)]
    pub(super) receipts: BTreeMap<OperationId, u64>,
    #[serde(default)]
    pub(super) closes: BTreeMap<OperationId, u64>,
    #[serde(default)]
    pub(super) git: BTreeMap<OperationId, u64>,
}

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

impl State {
    pub(super) fn expire_records(&mut self, now: u64) {
        let creations: Vec<_> = self
            .creations
            .iter()
            .map(|(operation, creation)| {
                (*operation, self.sessions.contains_key(&creation.info.id))
            })
            .collect();
        let cancelled: Vec<_> = self
            .cancelled
            .iter()
            .map(|operation| {
                let running = self
                    .creations
                    .get(operation)
                    .is_some_and(|creation| self.sessions.contains_key(&creation.info.id));
                (*operation, running)
            })
            .collect();
        let receipts: Vec<_> = self
            .receipts
            .iter()
            .map(|(operation, receipt)| (*operation, !receipt.complete))
            .collect();
        let closes: Vec<_> = self
            .closes
            .iter()
            .map(|(operation, (session, discard))| {
                (*operation, *discard && self.discarding.contains(session))
            })
            .collect();
        let git: Vec<_> = self
            .git
            .iter()
            .map(|(operation, receipt)| {
                (
                    *operation,
                    receipt.reply.is_none() && receipt.failed.is_none(),
                )
            })
            .collect();

        for operation in expire(&mut self.finished.creations, creations, now) {
            self.creations.remove(&operation);
        }
        for operation in expire(&mut self.finished.cancelled, cancelled, now) {
            self.cancelled.remove(&operation);
        }
        for operation in expire(&mut self.finished.receipts, receipts, now) {
            self.receipts.remove(&operation);
        }
        for operation in expire(&mut self.finished.closes, closes, now) {
            self.closes.remove(&operation);
        }
        for operation in expire(&mut self.finished.git, git, now) {
            self.git.remove(&operation);
        }
    }
}

fn expire(
    finished: &mut BTreeMap<OperationId, u64>,
    records: Vec<(OperationId, bool)>,
    now: u64,
) -> Vec<OperationId> {
    let excess = records.len().saturating_sub(MAX_RECORDS);
    let stamp = finished
        .values()
        .max()
        .map_or(now, |latest| now.max(latest.saturating_add(1)));
    let mut done: BTreeMap<_, _> = records
        .into_iter()
        .filter(|(_, running)| !running)
        .map(|(operation, _)| {
            (
                operation,
                finished.get(&operation).copied().unwrap_or(stamp),
            )
        })
        .collect();
    let mut oldest: Vec<_> = done
        .iter()
        .map(|(operation, at)| (*at, *operation))
        .collect();
    oldest.sort_unstable();
    let expired: Vec<_> = oldest
        .into_iter()
        .enumerate()
        .take_while(|&(index, (at, _))| index < excess || now.saturating_sub(at) >= RECORD_MILLIS)
        .map(|(_, (_, operation))| operation)
        .collect();
    for operation in &expired {
        done.remove(operation);
    }
    *finished = done;
    expired
}

#[cfg(test)]
mod tests {
    use super::{MAX_RECORDS, RECORD_MILLIS};
    use crate::catalog::storage::StoredState;
    use crate::catalog::{Catalog, Creation, Membership, Receipt, State};
    use muxy_protocol::{
        OperationId, ProjectIntent, ProjectMutation, ServerPath, SessionId, SessionInfo,
        SessionStatus,
    };
    use std::path::Path;

    const NOW: u64 = 1_000_000;

    fn state() -> State {
        State::fresh(Path::new("/tmp"))
    }

    fn session(state: &mut State, id: u64) -> SessionInfo {
        let info = SessionInfo {
            id: SessionId::new(id).expect("non-zero session id"),
            project: state.home,
            directory: ServerPath(b"/tmp".to_vec()),
        };
        state.sessions.insert(
            info.id,
            Membership {
                info: info.clone(),
                status: SessionStatus::Live,
                ended_at: None,
            },
        );
        info
    }

    fn create(state: &mut State, id: u64) -> OperationId {
        let info = session(state, id);
        let operation = OperationId::new();
        state
            .creations
            .insert(operation, Creation { info, error: None });
        operation
    }

    fn create_discarded(state: &mut State, id: u64) -> OperationId {
        let operation = create(state, id);
        state
            .sessions
            .remove(&SessionId::new(id).expect("non-zero session id"));
        operation
    }

    #[test]
    fn catalog_without_finish_times_loads() -> Result<(), serde_json::Error> {
        let mut json =
            serde_json::to_value(StoredState::try_from(&state()).map_err(serde_json::Error::io)?)?;
        json.as_object_mut()
            .expect("catalog is an object")
            .remove("finished");
        let loaded: State = serde_json::from_value::<StoredState>(json)?.into();
        assert!(loaded.finished.creations.is_empty());
        Ok(())
    }

    #[test]
    fn records_without_finish_times_expire_a_day_after_load() {
        let mut state = state();
        let operation = create_discarded(&mut state, 1);
        state.expire_records(NOW);
        state.expire_records(NOW + RECORD_MILLIS - 1);
        assert!(state.creations.contains_key(&operation));
        state.expire_records(NOW + RECORD_MILLIS);
        assert!(!state.creations.contains_key(&operation));
        assert!(state.finished.creations.is_empty());
    }

    #[test]
    fn oldest_record_goes_once_the_kind_is_full() {
        let mut state = state();
        let oldest = create_discarded(&mut state, 1);
        state.expire_records(NOW);
        let mut recent = oldest;
        for id in 2..=u64::try_from(MAX_RECORDS).expect("small") + 1 {
            recent = create_discarded(&mut state, id);
            state.expire_records(NOW + id);
        }
        assert_eq!(state.creations.len(), MAX_RECORDS);
        assert!(!state.creations.contains_key(&oldest));
        assert!(state.creations.contains_key(&recent));
    }

    #[test]
    fn records_of_operations_in_progress_or_sessions_still_kept_never_expire() {
        let mut state = state();
        let created = create(&mut state, 1);
        let cancelled = create(&mut state, 2);
        state.cancelled.insert(cancelled);
        let closed = session(&mut state, 3).id;
        let close = OperationId::new();
        state.closes.insert(close, (closed, true));
        state.discarding.insert(closed);
        let delete = ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Delete(state.home),
        };
        state.receipts.insert(
            delete.operation,
            Receipt {
                intent: delete.clone(),
                error: None,
                complete: false,
            },
        );
        for day in 0..3 {
            state.expire_records(NOW + day * RECORD_MILLIS);
        }
        assert!(state.creations.contains_key(&created));
        assert!(state.creations.contains_key(&cancelled));
        assert!(state.cancelled.contains(&cancelled));
        assert!(state.closes.contains_key(&close));
        assert!(state.receipts.contains_key(&delete.operation));

        state.sessions.clear();
        state.discarding.clear();
        state
            .receipts
            .get_mut(&delete.operation)
            .expect("delete receipt")
            .complete = true;
        state.expire_records(NOW + 3 * RECORD_MILLIS);
        state.expire_records(NOW + 4 * RECORD_MILLIS);
        assert!(state.creations.is_empty());
        assert!(state.cancelled.is_empty());
        assert!(state.closes.is_empty());
        assert!(state.receipts.is_empty());
    }

    #[test]
    fn full_catalog_forgets_the_oldest_discarded_creation_and_answers_recent_retries()
    -> Result<(), crate::ServerError> {
        let catalog = Catalog::memory(std::sync::Arc::default());
        let project = catalog.home();
        let directory = catalog.project(project)?.directory;
        let mut created = Vec::new();
        for id in 1..=u64::try_from(MAX_RECORDS).expect("small") + 1 {
            let info = SessionInfo {
                id: SessionId::new(id).expect("non-zero session id"),
                project,
                directory: directory.clone(),
            };
            let operation = OperationId::new();
            catalog.reserve(operation, &info)?;
            catalog.set_status(info.id, SessionStatus::Live)?;
            catalog.finish_discard(info.id)?;
            created.push((operation, info));
        }
        let (oldest, _) = &created[0];
        let (recent, info) = &created[MAX_RECORDS];
        assert_eq!(catalog.creation(*oldest, project, &directory)?, None);
        assert_eq!(
            catalog.creation(*recent, project, &directory)?.as_ref(),
            Some(info)
        );
        Ok(())
    }
}
