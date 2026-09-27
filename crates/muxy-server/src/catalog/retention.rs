use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{SessionId, SessionStatus};

use super::Catalog;

pub(crate) const RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Catalog {
    pub(crate) fn expired_sessions(&self, now: u64) -> Vec<SessionId> {
        self.retry_exits();
        self.lock()
            .sessions
            .values()
            .filter(|entry| {
                matches!(
                    entry.status,
                    SessionStatus::Ended | SessionStatus::Unavailable
                ) && entry
                    .ended_at
                    .is_some_and(|ended| now.saturating_sub(ended) >= RETENTION_SECONDS)
            })
            .map(|entry| entry.info.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{OperationId, ServerPath, SessionInfo};

    #[test]
    fn retention_expires_only_ended_sessions_at_the_seven_day_boundary() {
        let catalog = Catalog::memory();
        let ended = 1_000_000;
        let mut expired = Vec::new();
        for (index, status) in [
            SessionStatus::Starting,
            SessionStatus::Live,
            SessionStatus::Ended,
            SessionStatus::Unavailable,
        ]
        .into_iter()
        .enumerate()
        {
            let id = SessionId::new(index as u64 + 1).expect("session");
            catalog
                .reserve(
                    OperationId::new(),
                    &SessionInfo {
                        id,
                        project: catalog.home(),
                        directory: ServerPath(b"/tmp".to_vec()),
                    },
                )
                .expect("reserve");
            let mut state = catalog.lock();
            let entry = state.sessions.get_mut(&id).expect("entry");
            entry.status = status;
            entry.ended_at = Some(ended);
            if matches!(status, SessionStatus::Ended | SessionStatus::Unavailable) {
                expired.push(id);
            }
        }
        assert!(catalog.expired_sessions(ended - 1).is_empty());
        assert!(
            catalog
                .expired_sessions(ended + RETENTION_SECONDS - 1)
                .is_empty()
        );
        assert_eq!(catalog.expired_sessions(ended + RETENTION_SECONDS), expired);
        assert_eq!(catalog.expired_sessions(u64::MAX), expired);
    }
}
