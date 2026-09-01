use crate::terminal::session::client::{QueryOutcome, SessionClient};
use muxy_core::terminal_activity::{
    PersistentCommandActivity, ShellActivitySession, ShellActivityTracker,
};
use muxy_core::workspace::TabId;
use muxy_proto::session::CommandActivity;
use muxy_terminal::offline::{
    OfflineCandidate, has_running_process, persistent_session_is_idle, should_take_offline,
};
use muxy_terminal::process::{ProcessInspector, SystemProcessInspector, is_running_command};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

pub const ENABLED_SETTING: &str = "muxy.terminalOffline.enabled";
pub const IDLE_THRESHOLD_SETTING: &str = "muxy.terminalOffline.idleThresholdSeconds";
pub const DEFAULT_IDLE_THRESHOLD: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineRecord {
    pub directory: PathBuf,
    pub persistent: bool,
}

#[derive(Clone)]
struct DirectActivity {
    tracker: ShellActivityTracker,
    session: ShellActivitySession,
}

#[derive(Default)]
pub struct OfflineRuntime {
    enabled: bool,
    idle_threshold: Duration,
    invisible_since: HashMap<TabId, Duration>,
    records: HashMap<TabId, OfflineRecord>,
    direct_activity: HashMap<TabId, DirectActivity>,
    staged_ready: HashSet<TabId>,
}

#[derive(Clone)]
pub struct OfflineProbe {
    pub tab_id: TabId,
    pub foreground_pid: Option<u64>,
    pub alternate_screen: Option<bool>,
    pub direct_activity: PersistentCommandActivity,
    pub needs_confirm_close: bool,
    pub persistent_client: Option<SessionClient>,
    pub persistent: bool,
    pub directory: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineDecision {
    pub tab_id: TabId,
    pub directory: PathBuf,
    pub persistent: bool,
    pub is_idle: bool,
}

impl OfflineRuntime {
    pub fn configure(&mut self, enabled: bool, idle_threshold: Duration) {
        self.enabled = enabled;
        self.idle_threshold = idle_threshold;
        if !enabled {
            self.staged_ready.clear();
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn observe(
        &mut self,
        known: impl IntoIterator<Item = TabId>,
        awake: &HashSet<TabId>,
        now: Duration,
    ) {
        let known = known.into_iter().collect::<HashSet<_>>();
        self.invisible_since
            .retain(|tab_id, _| known.contains(tab_id));
        self.records.retain(|tab_id, _| known.contains(tab_id));
        self.direct_activity
            .retain(|tab_id, _| known.contains(tab_id));
        self.staged_ready.retain(|tab_id| known.contains(tab_id));
        for tab_id in known {
            if awake.contains(&tab_id) {
                self.invisible_since.remove(&tab_id);
                self.staged_ready.remove(&tab_id);
            } else {
                self.invisible_since.entry(tab_id).or_insert(now);
            }
        }
    }

    pub fn begin_direct_session(&mut self, tab_id: &str) {
        if let Some(activity) = self.direct_activity.remove(tab_id) {
            activity.session.invalidate();
        }
        let tracker = ShellActivityTracker::default();
        let session = tracker.begin_session();
        self.direct_activity
            .insert(tab_id.to_owned(), DirectActivity { tracker, session });
    }

    pub fn record_output(&self, tab_id: &str, bytes: &[u8]) {
        if let Some(activity) = self.direct_activity.get(tab_id) {
            activity.session.record_output(bytes);
        }
    }

    pub fn record_gap(&self, tab_id: &str) {
        if let Some(activity) = self.direct_activity.get(tab_id) {
            activity.session.record_gap();
        }
    }

    pub fn direct_activity(&self, tab_id: &str) -> PersistentCommandActivity {
        self.direct_activity
            .get(tab_id)
            .map(|activity| activity.tracker.activity())
            .unwrap_or(PersistentCommandActivity::Unknown)
    }

    pub fn should_probe(&self, tab_id: &str, now: Duration, has_live_surface: bool) -> bool {
        should_take_offline(
            OfflineCandidate {
                has_live_surface,
                is_already_offline: self.records.contains_key(tab_id),
                invisible_duration: self
                    .staged_ready
                    .contains(tab_id)
                    .then_some(self.idle_threshold)
                    .or_else(|| {
                        self.invisible_since
                            .get(tab_id)
                            .map(|started| now.saturating_sub(*started))
                    }),
                is_idle: true,
            },
            self.enabled,
            self.idle_threshold,
        )
    }

    pub fn stage_invisible_for(&mut self, tab_id: &str, duration: Duration, now: Duration) {
        if duration >= self.idle_threshold {
            self.staged_ready.insert(tab_id.to_owned());
        } else {
            self.invisible_since
                .insert(tab_id.to_owned(), now.saturating_sub(duration));
        }
    }

    pub fn take_offline(&mut self, decision: OfflineDecision, now: Duration) -> bool {
        if !decision.is_idle || !self.should_probe(&decision.tab_id, now, true) {
            return false;
        }
        if let Some(activity) = self.direct_activity.remove(&decision.tab_id) {
            activity.session.invalidate();
        }
        self.staged_ready.remove(&decision.tab_id);
        self.records.insert(
            decision.tab_id,
            OfflineRecord {
                directory: decision.directory,
                persistent: decision.persistent,
            },
        );
        true
    }

    pub fn is_offline(&self, tab_id: &str) -> bool {
        self.records.contains_key(tab_id)
    }

    pub fn wake(&mut self, tab_id: &str) -> Option<OfflineRecord> {
        self.records.remove(tab_id)
    }

    pub fn wake_all(&mut self) -> Vec<(TabId, OfflineRecord)> {
        self.records.drain().collect()
    }
}

pub fn evaluate_probe(probe: OfflineProbe) -> OfflineDecision {
    let mut directory = probe.directory;
    let is_idle =
        if probe.alternate_screen != Some(false) {
            false
        } else if probe.persistent {
            let state = probe.persistent_client.as_ref().and_then(|client| {
                match client.query(&probe.tab_id) {
                    QueryOutcome::Found(descriptor) => {
                        let reported_directory = PathBuf::from(&descriptor.working_directory);
                        if reported_directory.is_absolute() {
                            directory = reported_directory;
                        }
                        let activity = match descriptor.command_activity {
                            CommandActivity::Idle => PersistentCommandActivity::Idle,
                            CommandActivity::Running => PersistentCommandActivity::Running,
                            CommandActivity::Unknown => PersistentCommandActivity::Unknown,
                        };
                        let foreground =
                            SystemProcessInspector.foreground_process_id(descriptor.tty_device)?;
                        Some((
                            activity,
                            is_running_command(Some(foreground), descriptor.shell_pid),
                        ))
                    }
                    QueryOutcome::Missing | QueryOutcome::Unreachable(_) => None,
                }
            });
            state.is_some_and(|(activity, running)| {
                persistent_session_is_idle(Some(activity), running, false)
            })
        } else {
            let process = probe
                .foreground_pid
                .and_then(|pid| u32::try_from(pid).ok())
                .and_then(|pid| SystemProcessInspector.process(pid));
            !probe.needs_confirm_close
                && !has_running_process(
                    process.as_ref().map(|process| process.name.as_str()),
                    process.as_ref().map(|process| process.arguments.as_slice()),
                    probe.direct_activity != PersistentCommandActivity::Idle,
                )
        };
    OfflineDecision {
        tab_id: probe.tab_id,
        directory,
        persistent: probe.persistent,
        is_idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(value: &str) -> String {
        value.to_owned()
    }

    #[test]
    fn clocks_survive_enable_and_timeout_changes() {
        let mut runtime = OfflineRuntime::default();
        let pane = tab("pane");
        runtime.observe([pane.clone()], &HashSet::new(), Duration::from_secs(10));
        runtime.configure(true, Duration::from_secs(300));
        assert!(!runtime.should_probe(&pane, Duration::from_secs(309), true));
        runtime.configure(true, Duration::from_secs(60));
        assert!(runtime.should_probe(&pane, Duration::from_secs(70), true));
        runtime.configure(true, Duration::from_secs(600));
        assert!(!runtime.should_probe(&pane, Duration::from_secs(70), true));
        assert!(runtime.should_probe(&pane, Duration::from_secs(610), true));
    }

    #[test]
    fn focus_resets_only_the_affected_clock() {
        let mut runtime = OfflineRuntime::default();
        let first = tab("first");
        let second = tab("second");
        runtime.observe(
            [first.clone(), second.clone()],
            &HashSet::new(),
            Duration::from_secs(10),
        );
        runtime.observe(
            [first.clone(), second.clone()],
            &HashSet::from([first.clone()]),
            Duration::from_secs(50),
        );
        runtime.configure(true, Duration::from_secs(50));
        assert!(!runtime.should_probe(&first, Duration::from_secs(60), true));
        assert!(runtime.should_probe(&second, Duration::from_secs(60), true));
    }

    #[test]
    fn focus_invalidates_an_in_flight_idle_decision() {
        let mut runtime = OfflineRuntime::default();
        let pane = tab("pane");
        runtime.configure(true, Duration::from_secs(50));
        runtime.observe([pane.clone()], &HashSet::new(), Duration::from_secs(10));
        assert!(runtime.should_probe(&pane, Duration::from_secs(60), true));
        runtime.observe(
            [pane.clone()],
            &HashSet::from([pane.clone()]),
            Duration::from_secs(61),
        );
        assert!(!runtime.take_offline(
            OfflineDecision {
                tab_id: pane,
                directory: PathBuf::from("/tmp"),
                persistent: false,
                is_idle: true,
            },
            Duration::from_secs(61),
        ));
    }

    #[test]
    fn disabling_wakes_every_record_without_resetting_clocks() {
        let mut runtime = OfflineRuntime::default();
        runtime.configure(true, Duration::from_secs(10));
        runtime.observe(
            [tab("first"), tab("second")],
            &HashSet::new(),
            Duration::ZERO,
        );
        for pane in ["first", "second"] {
            assert!(runtime.take_offline(
                OfflineDecision {
                    tab_id: tab(pane),
                    directory: PathBuf::from("/tmp"),
                    persistent: pane == "second",
                    is_idle: true,
                },
                Duration::from_secs(10),
            ));
        }
        runtime.configure(false, Duration::from_secs(10));
        let mut woken = runtime.wake_all();
        woken.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(woken.len(), 2);
        assert!(!runtime.is_offline("first"));
        assert!(!runtime.is_offline("second"));
    }

    #[test]
    fn raw_activity_gap_is_busy_until_a_new_surface_session() {
        let mut runtime = OfflineRuntime::default();
        runtime.begin_direct_session("pane");
        assert_eq!(
            runtime.direct_activity("pane"),
            PersistentCommandActivity::Idle
        );
        runtime.record_output("pane", b"\x1b]133;C\x07");
        assert_eq!(
            runtime.direct_activity("pane"),
            PersistentCommandActivity::Running
        );
        runtime.record_output("pane", b"\x1b]133;D;0\x07");
        assert_eq!(
            runtime.direct_activity("pane"),
            PersistentCommandActivity::Idle
        );
        runtime.record_gap("pane");
        assert_eq!(
            runtime.direct_activity("pane"),
            PersistentCommandActivity::Unknown
        );
        runtime.begin_direct_session("pane");
        assert_eq!(
            runtime.direct_activity("pane"),
            PersistentCommandActivity::Idle
        );
    }
}
