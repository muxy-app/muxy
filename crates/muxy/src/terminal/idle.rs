use muxy_terminal::backend::TerminalLifecycleFacts;
use muxy_terminal::offline::policy::OfflineCandidate;
use muxy_terminal::offline::state::{
    MAX_WAKE_INPUT_BYTES, MAX_WAKE_OPERATIONS, OfflineStateMachine, OfflineTimer, SurfaceLifecycle,
    WakeOperation, WakeQueue, WakeQueueError,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleActivity {
    Input,
    Output,
    Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SleepRequest {
    pub tab_id: String,
    pub persistent: bool,
    pub working_directory: Option<PathBuf>,
    pub timer: OfflineTimer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputDisposition {
    SendNow,
    QueuedForWake,
}

pub struct IdleMaterialization {
    pub backing_identity: Option<String>,
    pub persistent: bool,
    pub surface_identity: u64,
    pub working_directory: Option<PathBuf>,
    pub host_activity_generation: u64,
    pub now_milliseconds: u64,
}

struct IdleTab {
    state: OfflineStateMachine,
    backing_identity: Option<String>,
    persistent: bool,
    working_directory: Option<PathBuf>,
    visible: bool,
    focused: bool,
    input_transaction_active: bool,
    resize_active: bool,
    materialization_active: bool,
    last_activity_milliseconds: u64,
    host_activity_generation: Option<u64>,
    last_grid_size: Option<(u16, u16)>,
    wake_queue: WakeQueue,
}

impl IdleTab {
    fn new(tab_id: &str, backing_identity: Option<String>, persistent: bool, now: u64) -> Self {
        Self {
            state: OfflineStateMachine::new(tab_id, backing_identity.clone()),
            backing_identity,
            persistent,
            working_directory: None,
            visible: false,
            focused: false,
            input_transaction_active: false,
            resize_active: false,
            materialization_active: false,
            last_activity_milliseconds: now,
            host_activity_generation: None,
            last_grid_size: None,
            wake_queue: WakeQueue::new(MAX_WAKE_INPUT_BYTES, MAX_WAKE_OPERATIONS),
        }
    }

    fn record_activity(&mut self, now: u64) {
        self.state.record_activity();
        self.last_activity_milliseconds = now;
    }
}

pub struct TerminalIdleCoordinator {
    enabled: bool,
    timeout_milliseconds: u64,
    tabs: HashMap<String, IdleTab>,
}

impl Default for TerminalIdleCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalIdleCoordinator {
    pub fn new() -> Self {
        Self {
            enabled: false,
            timeout_milliseconds: 300_000,
            tabs: HashMap::new(),
        }
    }

    pub fn set_settings(&mut self, enabled: bool, timeout_seconds: u64) {
        self.enabled = enabled;
        self.timeout_milliseconds = timeout_seconds.saturating_mul(1000);
        if enabled {
            return;
        }
        for tab in self.tabs.values_mut() {
            if tab.state.begin_wake() {
                tab.materialization_active = true;
            }
        }
    }

    pub fn begin_materialization(
        &mut self,
        tab_id: &str,
        backing_identity: Option<String>,
        persistent: bool,
        now: u64,
    ) {
        let tab = self
            .tabs
            .entry(tab_id.to_owned())
            .or_insert_with(|| IdleTab::new(tab_id, backing_identity.clone(), persistent, now));
        if tab.backing_identity != backing_identity {
            *tab = IdleTab::new(tab_id, backing_identity, persistent, now);
        }
        tab.persistent = persistent;
        tab.materialization_active = true;
        if !matches!(tab.state.lifecycle(), SurfaceLifecycle::Waking { .. }) {
            tab.state.begin_materialization();
        }
        tab.last_activity_milliseconds = now;
    }

    pub fn materialized(
        &mut self,
        tab_id: &str,
        materialization: IdleMaterialization,
    ) -> Vec<WakeOperation> {
        let IdleMaterialization {
            backing_identity,
            persistent,
            surface_identity,
            working_directory,
            host_activity_generation,
            now_milliseconds,
        } = materialization;
        let tab = self.tabs.entry(tab_id.to_owned()).or_insert_with(|| {
            IdleTab::new(
                tab_id,
                backing_identity.clone(),
                persistent,
                now_milliseconds,
            )
        });
        if tab.backing_identity != backing_identity {
            *tab = IdleTab::new(tab_id, backing_identity, persistent, now_milliseconds);
        }
        tab.persistent = persistent;
        tab.working_directory = working_directory;
        tab.materialization_active = false;
        tab.host_activity_generation = Some(host_activity_generation);
        tab.state.materialized(surface_identity);
        tab.last_activity_milliseconds = now_milliseconds;
        tab.wake_queue.drain()
    }

    pub fn sync_visibility(&mut self, visible: &[String], now: u64) {
        for (tab_id, tab) in &mut self.tabs {
            let next = visible.iter().any(|visible| visible == tab_id);
            if tab.visible != next {
                tab.visible = next;
                tab.record_activity(now);
            }
            if next && tab.state.begin_wake() {
                tab.materialization_active = true;
            }
        }
    }

    pub fn set_focused(&mut self, focused: Option<&str>, now: u64) {
        for (tab_id, tab) in &mut self.tabs {
            let next = focused == Some(tab_id.as_str());
            if tab.focused != next {
                tab.focused = next;
                tab.record_activity(now);
            }
            if next && tab.state.begin_wake() {
                tab.materialization_active = true;
            }
        }
    }

    pub fn record_activity(&mut self, tab_id: &str, _activity: IdleActivity, now: u64) {
        if let Some(tab) = self.tabs.get_mut(tab_id) {
            tab.record_activity(now);
        }
    }

    pub fn update_working_directory(&mut self, tab_id: &str, directory: Option<PathBuf>) {
        if let Some(tab) = self.tabs.get_mut(tab_id)
            && directory.is_some()
        {
            tab.working_directory = directory;
        }
    }

    pub fn set_input_transaction_active(&mut self, tab_id: &str, active: bool, now: u64) {
        if let Some(tab) = self.tabs.get_mut(tab_id)
            && tab.input_transaction_active != active
        {
            tab.input_transaction_active = active;
            tab.record_activity(now);
        }
    }

    pub fn set_resize_active(&mut self, tab_id: &str, active: bool, now: u64) {
        if let Some(tab) = self.tabs.get_mut(tab_id)
            && tab.resize_active != active
        {
            tab.resize_active = active;
            tab.record_activity(now);
        }
    }

    pub fn observe_lifecycle(
        &mut self,
        tab_id: &str,
        facts: TerminalLifecycleFacts,
        now: u64,
    ) -> Option<SleepRequest> {
        let tab = self.tabs.get_mut(tab_id)?;
        if tab.host_activity_generation != Some(facts.activity_generation) {
            tab.host_activity_generation = Some(facts.activity_generation);
            tab.record_activity(now);
            return None;
        }
        let timer = match tab.state.lifecycle() {
            SurfaceLifecycle::Live { surface_identity }
                if *surface_identity == facts.surface_identity =>
            {
                tab.state.schedule_sleep()?
            }
            SurfaceLifecycle::SleepPending { timer }
                if timer.surface_identity == facts.surface_identity =>
            {
                timer.clone()
            }
            _ => return None,
        };
        let candidate = OfflineCandidate {
            enabled: self.enabled,
            renderer_live: true,
            hidden: !tab.visible,
            focused: tab.focused,
            input_transaction_active: tab.input_transaction_active,
            queued_input: !tab.wake_queue.is_empty(),
            resize_active: tab.resize_active,
            materialization_active: tab.materialization_active,
            facts: facts.safety,
            activity_generation: tab.state.activity_generation(),
            timer_activity_generation: timer.activity_generation,
            last_activity_milliseconds: tab.last_activity_milliseconds,
            now_milliseconds: now,
            timeout_milliseconds: self.timeout_milliseconds,
        };
        if !tab.state.sleep_if_current(&timer, candidate) {
            tab.state.cancel_sleep(&timer);
            return None;
        }
        Some(SleepRequest {
            tab_id: tab_id.to_owned(),
            persistent: tab.persistent,
            working_directory: tab.working_directory.clone(),
            timer,
        })
    }

    pub fn queue_input(
        &mut self,
        tab_id: &str,
        bytes: &[u8],
        now: u64,
    ) -> Result<InputDisposition, WakeQueueError> {
        let Some(tab) = self.tabs.get_mut(tab_id) else {
            return Ok(InputDisposition::SendNow);
        };
        if !matches!(
            tab.state.lifecycle(),
            SurfaceLifecycle::Sleeping { .. } | SurfaceLifecycle::Waking { .. }
        ) {
            tab.record_activity(now);
            return Ok(InputDisposition::SendNow);
        }
        if tab.state.begin_wake() {
            tab.materialization_active = true;
        }
        if tab.wake_queue.is_empty()
            && let Some((columns, rows)) = tab.last_grid_size
        {
            tab.wake_queue
                .push(WakeOperation::Resize { columns, rows })?;
        }
        tab.wake_queue.push(WakeOperation::Input(bytes.to_vec()))?;
        tab.last_activity_milliseconds = now;
        Ok(InputDisposition::QueuedForWake)
    }

    pub fn queue_resize(
        &mut self,
        tab_id: &str,
        columns: u16,
        rows: u16,
        now: u64,
    ) -> Result<InputDisposition, WakeQueueError> {
        let Some(tab) = self.tabs.get_mut(tab_id) else {
            return Ok(InputDisposition::SendNow);
        };
        tab.last_grid_size = Some((columns, rows));
        if !matches!(
            tab.state.lifecycle(),
            SurfaceLifecycle::Sleeping { .. } | SurfaceLifecycle::Waking { .. }
        ) {
            tab.record_activity(now);
            return Ok(InputDisposition::SendNow);
        }
        if tab.state.begin_wake() {
            tab.materialization_active = true;
        }
        tab.wake_queue
            .push(WakeOperation::Resize { columns, rows })?;
        tab.last_activity_milliseconds = now;
        Ok(InputDisposition::QueuedForWake)
    }

    pub fn take_wake_operations(&mut self, tab_id: &str) -> Vec<WakeOperation> {
        self.tabs
            .get_mut(tab_id)
            .map(|tab| tab.wake_queue.drain())
            .unwrap_or_default()
    }

    pub fn restore_wake_operations(
        &mut self,
        tab_id: &str,
        operations: Vec<WakeOperation>,
    ) -> Result<(), WakeQueueError> {
        let Some(tab) = self.tabs.get_mut(tab_id) else {
            return Ok(());
        };
        for operation in operations {
            tab.wake_queue.push(operation)?;
        }
        Ok(())
    }

    pub fn wake_requested_tabs(&self) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|(_, tab)| matches!(tab.state.lifecycle(), SurfaceLifecycle::Waking { .. }))
            .map(|(tab_id, _)| tab_id.clone())
            .collect()
    }

    pub fn remove_unknown(&mut self, known: impl Fn(&str) -> bool) {
        self.tabs.retain(|tab_id, _| known(tab_id));
    }

    #[cfg(test)]
    fn lifecycle(&self, tab_id: &str) -> Option<&SurfaceLifecycle> {
        self.tabs.get(tab_id).map(|tab| tab.state.lifecycle())
    }

    #[cfg(test)]
    fn activity_generation(&self, tab_id: &str) -> Option<u64> {
        self.tabs
            .get(tab_id)
            .map(|tab| tab.state.activity_generation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_terminal::offline::policy::{ForegroundState, ProcessSafety, TerminalSafetyFacts};

    fn facts(surface_identity: u64, generation: u64) -> TerminalLifecycleFacts {
        TerminalLifecycleFacts {
            surface_identity,
            activity_generation: generation,
            safety: TerminalSafetyFacts {
                foreground: ForegroundState::Idle,
                process_safety: ProcessSafety::SafeToLoseOrdinaryShell,
                alternate_screen: false,
            },
        }
    }

    fn materialization(surface_identity: u64, now_milliseconds: u64) -> IdleMaterialization {
        IdleMaterialization {
            backing_identity: None,
            persistent: false,
            surface_identity,
            working_directory: Some(PathBuf::from("/tmp")),
            host_activity_generation: 3,
            now_milliseconds,
        }
    }

    fn live(hidden: bool) -> TerminalIdleCoordinator {
        let mut idle = TerminalIdleCoordinator::new();
        idle.set_settings(true, 1);
        idle.begin_materialization("tab", None, false, 0);
        idle.materialized("tab", materialization(7, 0));
        let visible = if hidden {
            Vec::new()
        } else {
            vec!["tab".to_owned()]
        };
        idle.sync_visibility(&visible, 0);
        idle
    }

    #[test]
    fn terminal_idle_only_sleeps_hidden_surfaces() {
        let mut hidden = live(true);
        assert!(hidden.observe_lifecycle("tab", facts(7, 3), 1000).is_some());
        let mut visible = live(false);
        assert!(
            visible
                .observe_lifecycle("tab", facts(7, 3), 1000)
                .is_none()
        );
        assert!(matches!(
            visible.lifecycle("tab"),
            Some(SurfaceLifecycle::Live { .. })
        ));
    }

    #[test]
    fn terminal_idle_stale_generation_surface_and_selection_fail_awake() {
        let mut idle = live(true);
        assert!(idle.observe_lifecycle("tab", facts(8, 3), 1000).is_none());
        assert!(idle.observe_lifecycle("tab", facts(7, 4), 1000).is_none());
        idle.sync_visibility(&["tab".to_owned()], 1000);
        assert!(idle.observe_lifecycle("tab", facts(7, 4), 2000).is_none());
    }

    #[test]
    fn terminal_idle_all_activity_sources_advance_the_generation() {
        let mut idle = live(true);
        let before = idle.activity_generation("tab").unwrap();
        for (index, activity) in [
            IdleActivity::Input,
            IdleActivity::Output,
            IdleActivity::Action,
        ]
        .into_iter()
        .enumerate()
        {
            idle.record_activity("tab", activity, index as u64 + 1);
        }
        idle.set_focused(Some("tab"), 4);
        idle.sync_visibility(&["tab".to_owned()], 5);
        idle.set_resize_active("tab", true, 6);
        idle.begin_materialization("tab", None, false, 7);
        assert!(idle.activity_generation("tab").unwrap() >= before + 7);
    }

    #[test]
    fn terminal_idle_transactions_and_unknown_process_facts_fail_awake() {
        let mut idle = live(true);
        idle.set_input_transaction_active("tab", true, 0);
        assert!(idle.observe_lifecycle("tab", facts(7, 3), 1000).is_none());
        idle.set_input_transaction_active("tab", false, 1000);
        let mut unknown = facts(7, 3);
        unknown.safety = TerminalSafetyFacts::unknown();
        assert!(idle.observe_lifecycle("tab", unknown, 2000).is_none());
    }

    #[test]
    fn terminal_idle_wake_queue_preserves_input_resize_order_and_bounds() {
        let mut idle = live(true);
        assert!(idle.observe_lifecycle("tab", facts(7, 3), 1000).is_some());
        assert_eq!(
            idle.queue_input("tab", b"one", 1001),
            Ok(InputDisposition::QueuedForWake)
        );
        assert_eq!(
            idle.queue_resize("tab", 80, 24, 1002),
            Ok(InputDisposition::QueuedForWake)
        );
        assert_eq!(
            idle.queue_input("tab", b"two", 1003),
            Ok(InputDisposition::QueuedForWake)
        );
        let operations = idle.materialized(
            "tab",
            IdleMaterialization {
                working_directory: None,
                host_activity_generation: 1,
                ..materialization(8, 1004)
            },
        );
        assert_eq!(
            operations,
            vec![
                WakeOperation::Input(b"one".to_vec()),
                WakeOperation::Resize {
                    columns: 80,
                    rows: 24,
                },
                WakeOperation::Input(b"two".to_vec()),
            ]
        );
    }

    #[test]
    fn terminal_idle_runtime_resize_is_replayed_before_wake_input() {
        let mut idle = live(true);
        assert_eq!(
            idle.queue_resize("tab", 120, 40, 1),
            Ok(InputDisposition::SendNow)
        );
        assert!(idle.observe_lifecycle("tab", facts(7, 3), 1001).is_some());
        assert_eq!(
            idle.queue_input("tab", b"input", 1002),
            Ok(InputDisposition::QueuedForWake)
        );
        let operations = idle.materialized(
            "tab",
            IdleMaterialization {
                host_activity_generation: 4,
                ..materialization(8, 1003)
            },
        );
        assert_eq!(
            operations,
            vec![
                WakeOperation::Resize {
                    columns: 120,
                    rows: 40,
                },
                WakeOperation::Input(b"input".to_vec()),
            ]
        );
    }

    #[test]
    fn terminal_idle_settings_disable_wakes_sleeping_surfaces() {
        let mut idle = live(true);
        assert!(idle.observe_lifecycle("tab", facts(7, 3), 1000).is_some());
        idle.set_settings(false, 1);
        assert_eq!(idle.wake_requested_tabs(), vec!["tab".to_owned()]);
    }

    #[test]
    fn terminal_idle_persistent_wake_preserves_session_identity_and_input() {
        let mut idle = TerminalIdleCoordinator::new();
        idle.set_settings(true, 1);
        idle.begin_materialization("tab", Some("session-one".to_owned()), true, 0);
        idle.materialized(
            "tab",
            IdleMaterialization {
                backing_identity: Some("session-one".to_owned()),
                persistent: true,
                ..materialization(7, 0)
            },
        );
        assert!(idle.observe_lifecycle("tab", facts(7, 3), 1000).is_some());
        assert_eq!(
            idle.queue_input("tab", b"queued", 1001),
            Ok(InputDisposition::QueuedForWake)
        );
        idle.begin_materialization("tab", Some("session-one".to_owned()), true, 1002);
        let operations = idle.materialized(
            "tab",
            IdleMaterialization {
                backing_identity: Some("session-one".to_owned()),
                persistent: true,
                ..materialization(8, 1003)
            },
        );
        assert_eq!(operations, vec![WakeOperation::Input(b"queued".to_vec())]);
        assert!(matches!(
            idle.lifecycle("tab"),
            Some(SurfaceLifecycle::Live {
                surface_identity: 8
            })
        ));
    }

    #[test]
    fn terminal_idle_ordinary_sleep_uses_latest_working_directory() {
        let mut idle = live(true);
        idle.update_working_directory("tab", Some(PathBuf::from("/tmp/current")));
        let request = idle.observe_lifecycle("tab", facts(7, 3), 1000).unwrap();
        assert!(!request.persistent);
        assert_eq!(
            request.working_directory,
            Some(PathBuf::from("/tmp/current"))
        );
        assert_eq!(request.timer.backing_identity, None);
    }
}
