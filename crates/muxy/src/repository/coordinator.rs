use muxy_api::repository::{
    ChangedFiles, MutationBoundary, MutationEffect, PullRequestInfo, RepositorySummary,
};
use muxy_api::subprocess::CancellationSignal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const WATCHER_DEBOUNCE: Duration = Duration::from_millis(800);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RepositoryKey {
    pub project_id: String,
    pub worktree_id: String,
    pub normalized_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RepositoryReadKind {
    Summary,
    Branches,
    Changes,
    PullRequest,
    Providers,
}

impl RepositoryReadKind {
    const ALL: [Self; 5] = [
        Self::Summary,
        Self::Branches,
        Self::Changes,
        Self::PullRequest,
        Self::Providers,
    ];

    fn index(self) -> usize {
        match self {
            Self::Summary => 0,
            Self::Branches => 1,
            Self::Changes => 2,
            Self::PullRequest => 3,
            Self::Providers => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RepositoryRevisions([u64; 5]);

impl RepositoryRevisions {
    #[cfg(test)]
    pub(crate) fn all_newer_than(self, previous: Self) -> bool {
        self.0
            .iter()
            .zip(previous.0)
            .all(|(current, previous)| *current > previous)
    }

    fn get(self, kind: RepositoryReadKind) -> u64 {
        self.0[kind.index()]
    }

    fn bump(&mut self, kind: RepositoryReadKind) -> u64 {
        let revision = &mut self.0[kind.index()];
        *revision = revision.wrapping_add(1).max(1);
        *revision
    }

    fn bump_all(&mut self) {
        for kind in RepositoryReadKind::ALL {
            self.bump(kind);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestReadIdentity {
    pub branch: String,
    pub head_oid: String,
}

impl PullRequestReadIdentity {
    pub(crate) fn new(branch: impl Into<String>, head_oid: impl Into<String>) -> Self {
        Self {
            branch: branch.into(),
            head_oid: head_oid.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RepositoryRefreshSet(u8);

impl RepositoryRefreshSet {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn all() -> Self {
        let mut set = Self::empty();
        for kind in RepositoryReadKind::ALL {
            set.insert(kind);
        }
        set
    }

    pub(crate) fn summary_and_branches() -> Self {
        let mut set = Self::empty();
        set.insert(RepositoryReadKind::Summary);
        set.insert(RepositoryReadKind::Branches);
        set
    }

    pub(crate) fn branches() -> Self {
        let mut set = Self::empty();
        set.insert(RepositoryReadKind::Branches);
        set
    }

    #[cfg(test)]
    pub(crate) fn summary_and_changes() -> Self {
        let mut set = Self::empty();
        set.insert(RepositoryReadKind::Summary);
        set.insert(RepositoryReadKind::Changes);
        set
    }

    pub(crate) fn summary_branches_pull_request() -> Self {
        let mut set = Self::empty();
        set.insert(RepositoryReadKind::Summary);
        set.insert(RepositoryReadKind::Branches);
        set.insert(RepositoryReadKind::PullRequest);
        set
    }

    #[cfg(test)]
    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) fn contains(self, kind: RepositoryReadKind) -> bool {
        self.0 & (1 << kind.index()) != 0
    }

    pub(crate) fn insert(&mut self, kind: RepositoryReadKind) {
        self.0 |= 1 << kind.index();
    }

    fn union(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum LoadState<T> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Error(String),
}

impl<T> LoadState<T> {
    pub(crate) fn begin_refresh(&mut self) {
        if !matches!(self, Self::Ready(_)) {
            *self = Self::Loading;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum PullRequestLoadState {
    #[default]
    Idle,
    Loading,
    NoPullRequest,
    Unavailable(String),
    Found(Box<PullRequestInfo>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RepositoryState {
    pub key: Option<RepositoryKey>,
    pub summary: LoadState<RepositorySummary>,
    pub branches: LoadState<Vec<Vec<u8>>>,
    pub changes: LoadState<ChangedFiles>,
    pub pull_request: PullRequestLoadState,
    pub providers: LoadState<()>,
}

#[derive(Clone, Debug)]
pub(crate) struct RepositoryReadToken {
    request_id: u64,
    key: RepositoryKey,
    kind: RepositoryReadKind,
    revision: u64,
    pull_request_identity: Option<PullRequestReadIdentity>,
    cancellation: CancellationSignal,
}

impl RepositoryReadToken {
    pub(crate) fn cancellation(&self) -> CancellationSignal {
        self.cancellation.clone()
    }
}

#[allow(dead_code)]
struct MutationRequest {
    request_id: u64,
    cancellation: CancellationSignal,
    boundary: MutationBoundary,
    current_identity: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutationCompletion {
    pub effect: MutationEffect,
    pub current_identity: bool,
    pub refresh: RepositoryRefreshSet,
}

#[derive(Default)]
pub(crate) struct RepositoryCoordinator {
    key: Option<RepositoryKey>,
    state: RepositoryState,
    revisions: RepositoryRevisions,
    environment_revision: u64,
    next_request_id: u64,
    reads: HashMap<u64, RepositoryReadToken>,
    mutation: Option<MutationRequest>,
    pending_refresh: RepositoryRefreshSet,
    debounce_deadline: Option<Instant>,
    changes_monitoring: bool,
    pull_request_identity: Option<PullRequestReadIdentity>,
    closed: bool,
}

impl RepositoryCoordinator {
    pub(crate) fn activate(&mut self, key: Option<RepositoryKey>) -> bool {
        if self.key == key && !self.closed {
            return false;
        }
        self.closed = false;
        self.cancel_reads();
        let cancel_mutation = if let Some(mutation) = &mut self.mutation {
            if mutation.boundary.cancel_for_identity_change() {
                mutation.cancellation.cancel();
                true
            } else {
                mutation.current_identity = false;
                false
            }
        } else {
            false
        };
        if cancel_mutation {
            self.mutation = None;
        }
        self.key = key.clone();
        self.state = RepositoryState {
            key,
            ..RepositoryState::default()
        };
        self.pull_request_identity = None;
        self.revisions.bump_all();
        self.pending_refresh = RepositoryRefreshSet::empty();
        self.debounce_deadline = None;
        true
    }

    pub(crate) fn key(&self) -> Option<&RepositoryKey> {
        self.key.as_ref()
    }

    pub(crate) fn state(&self) -> &RepositoryState {
        &self.state
    }

    pub(crate) fn state_mut(&mut self) -> &mut RepositoryState {
        &mut self.state
    }

    pub(crate) fn begin_read(
        &mut self,
        kind: RepositoryReadKind,
        pull_request_identity: Option<PullRequestReadIdentity>,
    ) -> Option<RepositoryReadToken> {
        if self.closed || self.mutation.is_some() {
            return None;
        }
        let key = self.key.clone()?;
        if kind == RepositoryReadKind::PullRequest {
            self.pull_request_identity = pull_request_identity.clone();
        }
        self.cancel_reads_of_kind(kind);
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let token = RepositoryReadToken {
            request_id: self.next_request_id,
            key,
            kind,
            revision: self.revisions.bump(kind),
            pull_request_identity,
            cancellation: CancellationSignal::new(),
        };
        self.reads.insert(token.request_id, token.clone());
        Some(token)
    }

    pub(crate) fn finish_read(
        &mut self,
        token: &RepositoryReadToken,
        current_pull_request_identity: Option<&PullRequestReadIdentity>,
    ) -> bool {
        let Some(active) = self.reads.remove(&token.request_id) else {
            return false;
        };
        active.key == token.key
            && self.key.as_ref() == Some(&token.key)
            && self.revisions.get(token.kind) == token.revision
            && !token.cancellation.is_cancelled()
            && (token.kind != RepositoryReadKind::PullRequest
                || (token.pull_request_identity.as_ref() == current_pull_request_identity
                    && self.pull_request_identity.as_ref() == current_pull_request_identity))
    }

    pub(crate) fn invalidate_pull_request(
        &mut self,
        branch: impl Into<String>,
        head_oid: impl Into<String>,
    ) {
        self.pull_request_identity = Some(PullRequestReadIdentity::new(branch, head_oid));
        self.cancel_reads_of_kind(RepositoryReadKind::PullRequest);
        self.revisions.bump(RepositoryReadKind::PullRequest);
        self.state.pull_request = PullRequestLoadState::Idle;
        self.pending_refresh.insert(RepositoryReadKind::PullRequest);
    }

    #[cfg(test)]
    pub(crate) fn revisions(&self) -> RepositoryRevisions {
        self.revisions
    }

    #[cfg(test)]
    pub(crate) fn environment_revision(&self) -> u64 {
        self.environment_revision
    }

    pub(crate) fn environment_upgraded(&mut self, revision: u64) -> bool {
        if revision <= self.environment_revision {
            return false;
        }
        self.environment_revision = revision;
        self.cancel_reads();
        self.revisions.bump_all();
        let key = self.key.clone();
        self.state = RepositoryState {
            key,
            ..RepositoryState::default()
        };
        self.pull_request_identity = None;
        if self.key.is_some() {
            self.pending_refresh.union(RepositoryRefreshSet::all());
        }
        true
    }

    pub(crate) fn watcher_invalidated(&mut self, now: Instant) {
        if self.key.is_none() || self.closed {
            return;
        }
        self.pending_refresh.insert(RepositoryReadKind::Summary);
        if self.changes_monitoring {
            self.pending_refresh.insert(RepositoryReadKind::Changes);
        }
        self.debounce_deadline = Some(now + WATCHER_DEBOUNCE);
    }

    pub(crate) fn take_debounced_refresh(&mut self, now: Instant) -> RepositoryRefreshSet {
        if self.mutation.is_some() || self.debounce_deadline.is_none_or(|deadline| now < deadline) {
            return RepositoryRefreshSet::empty();
        }
        self.debounce_deadline = None;
        std::mem::take(&mut self.pending_refresh)
    }

    pub(crate) fn request_refresh(&mut self, refresh: RepositoryRefreshSet) {
        if self.key.is_some() && !self.closed {
            self.pending_refresh.union(refresh);
        }
    }

    pub(crate) fn take_refresh(&mut self) -> RepositoryRefreshSet {
        if self.mutation.is_some() {
            RepositoryRefreshSet::empty()
        } else {
            std::mem::take(&mut self.pending_refresh)
        }
    }

    pub(crate) fn set_changes_monitoring(&mut self, monitoring: bool) {
        self.changes_monitoring = monitoring;
    }

    pub(crate) fn app_activated(&mut self) {
        if self.key.is_none() || self.closed {
            return;
        }
        self.pending_refresh.insert(RepositoryReadKind::Summary);
        self.pending_refresh.insert(RepositoryReadKind::Branches);
        self.pending_refresh.insert(RepositoryReadKind::PullRequest);
        self.pending_refresh.insert(RepositoryReadKind::Providers);
        if self.changes_monitoring {
            self.pending_refresh.insert(RepositoryReadKind::Changes);
        }
    }

    #[allow(dead_code)]
    pub fn begin_mutation(
        &mut self,
        request_id: u64,
    ) -> Option<(CancellationSignal, MutationBoundary)> {
        if self.closed || self.key.is_none() || self.mutation.is_some() {
            return None;
        }
        self.cancel_reads();
        let cancellation = CancellationSignal::new();
        let boundary = MutationBoundary::default();
        self.mutation = Some(MutationRequest {
            request_id,
            cancellation: cancellation.clone(),
            boundary: boundary.clone(),
            current_identity: true,
        });
        Some((cancellation, boundary))
    }

    #[allow(dead_code)]
    pub fn mark_irreversible(&mut self, request_id: u64) -> Option<()> {
        let mutation = self.mutation.as_mut()?;
        if mutation.request_id != request_id {
            return None;
        }
        mutation.boundary.begin_irreversible().then_some(())
    }

    #[allow(dead_code)]
    pub fn finish_mutation(
        &mut self,
        request_id: u64,
        effect: MutationEffect,
    ) -> Option<MutationCompletion> {
        if self.mutation.as_ref()?.request_id != request_id {
            return None;
        }
        let mutation = self.mutation.take()?;
        Some(MutationCompletion {
            effect,
            current_identity: mutation.current_identity && !self.closed,
            refresh: std::mem::take(&mut self.pending_refresh),
        })
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.cancel_reads();
        if let Some(mutation) = &mut self.mutation {
            mutation.cancellation.cancel();
            mutation.current_identity = false;
        }
        self.pending_refresh = RepositoryRefreshSet::empty();
        self.debounce_deadline = None;
    }

    #[cfg(test)]
    pub(crate) fn active_request_count(&self) -> usize {
        self.reads.len() + usize::from(self.mutation.is_some())
    }

    fn cancel_reads(&mut self) {
        for read in self.reads.values() {
            read.cancellation.cancel();
        }
        self.reads.clear();
    }

    fn cancel_reads_of_kind(&mut self, kind: RepositoryReadKind) {
        self.reads.retain(|_, read| {
            if read.kind == kind {
                read.cancellation.cancel();
                false
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn key(project: &str, worktree: &str) -> RepositoryKey {
        RepositoryKey {
            project_id: project.to_owned(),
            worktree_id: worktree.to_owned(),
            normalized_path: PathBuf::from(format!("/{project}/{worktree}")),
        }
    }

    #[test]
    fn repository_coordinator_revisions_reject_stale_and_pr_identity_results() {
        let mut coordinator = RepositoryCoordinator::default();
        coordinator.activate(Some(key("one", "primary")));
        let stale_summary = coordinator
            .begin_read(RepositoryReadKind::Summary, None)
            .unwrap();
        let branches = coordinator
            .begin_read(RepositoryReadKind::Branches, None)
            .unwrap();
        let current_summary = coordinator
            .begin_read(RepositoryReadKind::Summary, None)
            .unwrap();

        assert!(stale_summary.cancellation().is_cancelled());
        assert!(!coordinator.finish_read(&stale_summary, None));
        assert!(coordinator.finish_read(&branches, None));
        assert!(coordinator.finish_read(&current_summary, None));

        let old_pr = coordinator
            .begin_read(
                RepositoryReadKind::PullRequest,
                Some(PullRequestReadIdentity::new("topic", "aaaa")),
            )
            .unwrap();
        coordinator.invalidate_pull_request("topic", "bbbb");
        assert!(old_pr.cancellation().is_cancelled());
        assert!(!coordinator.finish_read(
            &old_pr,
            Some(&PullRequestReadIdentity::new("topic", "bbbb"))
        ));

        let before = coordinator.revisions();
        coordinator.environment_upgraded(2);
        assert!(coordinator.revisions().all_newer_than(before));
        assert_eq!(coordinator.environment_revision(), 2);
    }

    #[test]
    fn repository_coordinator_debounces_bursts_and_coalesces_mutation_refreshes() {
        let mut coordinator = RepositoryCoordinator::default();
        coordinator.activate(Some(key("one", "primary")));
        let start = Instant::now();
        for offset in 0..10_000 {
            coordinator.watcher_invalidated(start + Duration::from_micros(offset));
        }
        assert!(
            coordinator
                .take_debounced_refresh(start + Duration::from_millis(799))
                .is_empty()
        );
        let refresh = coordinator.take_debounced_refresh(
            start + Duration::from_millis(800) + Duration::from_micros(9_999),
        );
        assert!(refresh.contains(RepositoryReadKind::Summary));
        assert!(!refresh.contains(RepositoryReadKind::Changes));
        assert!(
            coordinator
                .take_debounced_refresh(start + Duration::from_secs(2))
                .is_empty()
        );

        coordinator.set_changes_monitoring(true);
        let (mutation, _) = coordinator.begin_mutation(7).unwrap();
        coordinator.request_refresh(RepositoryRefreshSet::summary_and_changes());
        assert!(
            coordinator
                .begin_read(RepositoryReadKind::Summary, None)
                .is_none()
        );
        let completion = coordinator
            .finish_mutation(7, muxy_api::repository::MutationEffect::NoMutation)
            .unwrap();
        assert!(completion.current_identity);
        assert!(completion.refresh.contains(RepositoryReadKind::Summary));
        assert!(completion.refresh.contains(RepositoryReadKind::Changes));
        assert!(!mutation.is_cancelled());
    }

    #[test]
    fn repository_coordinator_identity_and_close_follow_irreversible_cancellation_rules() {
        let mut coordinator = RepositoryCoordinator::default();
        coordinator.activate(Some(key("one", "primary")));
        let (cancellable, cancellable_boundary) = coordinator.begin_mutation(1).unwrap();
        coordinator.activate(Some(key("two", "primary")));
        assert!(cancellable.is_cancelled());
        assert!(cancellable_boundary.is_cancelled());
        assert!(
            coordinator
                .finish_mutation(1, muxy_api::repository::MutationEffect::NoMutation)
                .is_none()
        );

        let (irreversible, irreversible_boundary) = coordinator.begin_mutation(2).unwrap();
        coordinator.mark_irreversible(2).unwrap();
        coordinator.activate(Some(key("three", "primary")));
        assert!(!irreversible.is_cancelled());
        assert!(irreversible_boundary.stop_after_current());
        if irreversible_boundary.finish_irreversible() {
            irreversible.cancel();
        }
        assert!(irreversible.is_cancelled());
        let completion = coordinator
            .finish_mutation(2, muxy_api::repository::MutationEffect::Uncertain)
            .unwrap();
        assert!(!completion.current_identity);

        let (closing, _) = coordinator.begin_mutation(3).unwrap();
        coordinator.mark_irreversible(3).unwrap();
        coordinator.close();
        assert!(closing.is_cancelled());
        let completion = coordinator
            .finish_mutation(3, muxy_api::repository::MutationEffect::Uncertain)
            .unwrap();
        assert_eq!(
            completion.effect,
            muxy_api::repository::MutationEffect::Uncertain
        );
        assert_eq!(coordinator.active_request_count(), 0);
    }

    #[test]
    fn repository_coordinator_activation_refresh_and_identity_switch_cancel_every_read() {
        let mut coordinator = RepositoryCoordinator::default();
        coordinator.activate(Some(key("one", "primary")));
        coordinator.app_activated();
        let refresh = coordinator.take_refresh();
        assert!(refresh.contains(RepositoryReadKind::Summary));
        assert!(refresh.contains(RepositoryReadKind::Branches));
        assert!(refresh.contains(RepositoryReadKind::PullRequest));
        assert!(refresh.contains(RepositoryReadKind::Providers));
        assert!(!refresh.contains(RepositoryReadKind::Changes));

        coordinator.set_changes_monitoring(true);
        coordinator.app_activated();
        assert!(
            coordinator
                .take_refresh()
                .contains(RepositoryReadKind::Changes)
        );
        let reads: Vec<_> = RepositoryReadKind::ALL
            .into_iter()
            .map(|kind| {
                let identity = (kind == RepositoryReadKind::PullRequest)
                    .then(|| PullRequestReadIdentity::new("topic", "aaaa"));
                coordinator.begin_read(kind, identity).unwrap()
            })
            .collect();
        coordinator.activate(Some(key("two", "secondary")));
        for read in reads {
            assert!(read.cancellation().is_cancelled());
            assert!(!coordinator.finish_read(&read, None));
        }
        assert_eq!(coordinator.active_request_count(), 0);
    }
}
#[test]
fn load_state_refresh_preserves_ready_content_and_marks_other_states_loading() {
    let mut ready = LoadState::Ready("main");
    ready.begin_refresh();
    assert_eq!(ready, LoadState::Ready("main"));

    let mut idle = LoadState::<&str>::Idle;
    idle.begin_refresh();
    assert_eq!(idle, LoadState::Loading);

    let mut failed = LoadState::<&str>::Error("failed".to_owned());
    failed.begin_refresh();
    assert_eq!(failed, LoadState::Loading);
}
