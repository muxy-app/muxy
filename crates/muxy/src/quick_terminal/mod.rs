pub mod panel;
mod platform;
pub mod runtime;
pub mod session;
pub mod settings_transaction;
pub mod shortcut_service;
pub mod view;

use async_channel::Receiver;
use gpui::{KeyBinding, actions};
use muxy_core::quick_terminal::QuickTerminalShortcut;
use muxy_core::shortcuts::{COMMAND, CONTROL, KeyCombo, OPTION};
use shortcut_service::{
    MonitoringState, PreparedShortcutUpdate, QuickTerminalShortcutService, ShortcutBackend,
    ShortcutBackendFactory, ShortcutServiceError,
};
use std::cell::Cell;
use std::rc::Rc;

pub const KEY_CONTEXT: &str = "QuickTerminal";

actions!(quick_terminal, [CloseSurface]);

pub fn key_bindings() -> Vec<KeyBinding> {
    vec![KeyBinding::new("cmd-w", CloseSurface, Some(KEY_CONTEXT))]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutCapture {
    pub combo: KeyCombo,
    pub virtual_key_code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShortcutRecordingEvent {
    Captured(ShortcutCapture),
    Cancelled,
    Rejected(String),
}

pub struct ShortcutRecording {
    _native: platform::ShortcutRecorder,
}

pub enum ShortcutRecordingAction {
    Ignore,
    Capture(ShortcutCapture),
    Cancel,
    Reject(String),
}

pub fn shortcut_recording_action(
    active_generation: u64,
    event_generation: u64,
    event: ShortcutRecordingEvent,
) -> ShortcutRecordingAction {
    if active_generation != event_generation {
        return ShortcutRecordingAction::Ignore;
    }
    match event {
        ShortcutRecordingEvent::Captured(capture) => ShortcutRecordingAction::Capture(capture),
        ShortcutRecordingEvent::Cancelled => ShortcutRecordingAction::Cancel,
        ShortcutRecordingEvent::Rejected(error) => ShortcutRecordingAction::Reject(error),
    }
}

pub fn start_shortcut_recording()
-> Result<(ShortcutRecording, Receiver<ShortcutRecordingEvent>), String> {
    let (sender, receiver) = async_channel::unbounded();
    let native = platform::start_shortcut_recorder(sender)?;
    Ok((ShortcutRecording { _native: native }, receiver))
}

const STAGED_CASE_ENV: &str = "MUXY_TEST_P6_SHORTCUT_CASE";
const STAGED_STATUS_FILE: &str = ".muxy-p6-shortcut-status.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StagedShortcutCase {
    BackendFailure,
    PersistenceFailure,
}

pub struct QuickTerminalApplicationService {
    service: QuickTerminalShortcutService,
    staged_case: Option<StagedShortcutCase>,
    staged_candidate_stops: Rc<Cell<usize>>,
}

impl QuickTerminalApplicationService {
    pub fn load() -> Self {
        let shortcut = muxy_core::prefs::settings::quick_terminal_shortcut();
        let enabled = muxy_core::prefs::settings::bool_value("muxy.quickTerminal.enabled", true);
        let staged_case = staged_shortcut_case(
            muxy_core::prefs::is_test_process(),
            std::env::var(STAGED_CASE_ENV).ok().as_deref(),
        );
        let mut factory = platform::factory();
        let staged_candidate_stops = Rc::new(Cell::new(0));
        if let Some(case) = staged_case {
            factory = Box::new(StagedFactory {
                inner: factory,
                case,
                candidate_stops: staged_candidate_stops.clone(),
            });
        }
        let fail_persistence = staged_case == Some(StagedShortcutCase::PersistenceFailure);
        Self {
            service: QuickTerminalShortcutService::new(
                shortcut,
                enabled,
                factory,
                Box::new(move |shortcut| {
                    if fail_persistence {
                        Err(std::io::Error::other("forced staged persistence failure"))
                    } else {
                        muxy_core::prefs::settings::set_quick_terminal_shortcut(shortcut)
                    }
                }),
                Box::new(platform::resolve_key),
            ),
            staged_case,
            staged_candidate_stops,
        }
    }

    #[cfg(test)]
    fn from_service(service: QuickTerminalShortcutService) -> Self {
        Self {
            service,
            staged_case: None,
            staged_candidate_stops: Rc::new(Cell::new(0)),
        }
    }

    pub fn start(&mut self) -> Result<(), shortcut_service::ShortcutServiceError> {
        self.service.start()
    }

    pub fn run_staged_control(&mut self) {
        let Some(case) = self.staged_case else {
            return;
        };
        let candidate = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND | CONTROL | OPTION),
            virtual_key_code: 0,
        };
        let result = self.service.update_shortcut(candidate, &[]);
        let status = serde_json::json!({
            "case": staged_case_name(case),
            "result": if result.is_ok() { "success" } else { "error" },
            "error": result.err().map(|error| error.to_string()),
            "shortcut": shortcut_name(self.service.shortcut()),
            "monitoring": monitoring_state_name(self.service.monitoring_state()),
            "candidateStops": self.staged_candidate_stops.get(),
        });
        let path = muxy_core::prefs::app_support_dir().join(STAGED_STATUS_FILE);
        if let Err(error) = muxy_core::store::write_private(
            &path,
            serde_json::to_string_pretty(&status)
                .expect("staged shortcut status must encode")
                .as_bytes(),
        ) {
            log::warn!("failed to write staged Quick Terminal shortcut status: {error}");
        }
    }

    pub fn trigger_receiver(&self) -> Receiver<()> {
        self.service.trigger_receiver()
    }

    pub fn stop(&mut self) {
        self.service.stop();
    }

    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), ShortcutServiceError> {
        self.service.set_enabled(enabled)
    }

    pub fn update_shortcut(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[muxy_core::quick_terminal::ConflictCandidate],
    ) -> Result<(), ShortcutServiceError> {
        self.service.update_shortcut(shortcut, conflicts)
    }

    pub fn prepare_shortcut(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[muxy_core::quick_terminal::ConflictCandidate],
    ) -> Result<PreparedShortcutUpdate, ShortcutServiceError> {
        self.service.prepare_shortcut(shortcut, conflicts)
    }

    pub fn prepare_shortcut_for_enabled(
        &mut self,
        shortcut: QuickTerminalShortcut,
        conflicts: &[muxy_core::quick_terminal::ConflictCandidate],
        enabled: bool,
    ) -> Result<PreparedShortcutUpdate, ShortcutServiceError> {
        self.service
            .prepare_shortcut_for_enabled(shortcut, conflicts, enabled)
    }

    pub fn commit_shortcut(&mut self, prepared: PreparedShortcutUpdate) {
        self.service.commit_prepared(prepared);
    }

    pub fn cancel_shortcut(&mut self, prepared: PreparedShortcutUpdate) {
        self.service.cancel_prepared(prepared);
    }

    pub fn shortcut(&self) -> &QuickTerminalShortcut {
        self.service.shortcut()
    }

    pub fn request_input_monitoring_access(&mut self) -> bool {
        self.service.request_input_monitoring_access()
    }

    pub fn refresh_input_monitoring_access(&mut self) -> bool {
        self.service.refresh_input_monitoring_access()
    }

    pub fn shortcut_label(&self) -> String {
        match self.service.shortcut() {
            QuickTerminalShortcut::Unassigned => "Unassigned".to_owned(),
            QuickTerminalShortcut::DoubleShift => "Double Shift".to_owned(),
            QuickTerminalShortcut::KeyCombo { key_combo, .. } => key_combo.display(),
        }
    }

    pub fn monitoring_label(&self) -> String {
        if let Some(error) = self.service.error_message() {
            return error.to_owned();
        }
        monitoring_state_label(self.service.monitoring_state()).to_owned()
    }
}

fn monitoring_state_label(state: MonitoringState) -> &'static str {
    match state {
        MonitoringState::Stopped => "Inactive",
        MonitoringState::Unavailable => "Unavailable",
        MonitoringState::LocalOnly => "Local only",
        MonitoringState::SystemWide => "System-wide",
        MonitoringState::CarbonHotKey => "Active system-wide",
    }
}

fn staged_shortcut_case(is_test_process: bool, value: Option<&str>) -> Option<StagedShortcutCase> {
    if !is_test_process {
        return None;
    }
    match value {
        Some("backend-failure") => Some(StagedShortcutCase::BackendFailure),
        Some("persistence-failure") => Some(StagedShortcutCase::PersistenceFailure),
        _ => None,
    }
}

fn staged_case_name(case: StagedShortcutCase) -> &'static str {
    match case {
        StagedShortcutCase::BackendFailure => "backend-failure",
        StagedShortcutCase::PersistenceFailure => "persistence-failure",
    }
}

fn shortcut_name(shortcut: &QuickTerminalShortcut) -> &'static str {
    match shortcut {
        QuickTerminalShortcut::Unassigned => "unassigned",
        QuickTerminalShortcut::DoubleShift => "doubleShift",
        QuickTerminalShortcut::KeyCombo { .. } => "keyCombo",
    }
}

fn monitoring_state_name(state: MonitoringState) -> &'static str {
    match state {
        MonitoringState::Stopped => "stopped",
        MonitoringState::Unavailable => "unavailable",
        MonitoringState::LocalOnly => "localOnly",
        MonitoringState::SystemWide => "systemWide",
        MonitoringState::CarbonHotKey => "carbonHotKey",
    }
}

struct StagedFactory {
    inner: Box<dyn ShortcutBackendFactory>,
    case: StagedShortcutCase,
    candidate_stops: Rc<Cell<usize>>,
}

impl ShortcutBackendFactory for StagedFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
        if !matches!(shortcut, QuickTerminalShortcut::KeyCombo { .. }) {
            return self.inner.create(shortcut);
        }
        match self.case {
            StagedShortcutCase::BackendFailure => Some(Box::new(StagedCandidateBackend {
                fail_start: true,
                candidate_stops: self.candidate_stops.clone(),
            })),
            StagedShortcutCase::PersistenceFailure => Some(Box::new(StagedCandidateBackend {
                fail_start: false,
                candidate_stops: self.candidate_stops.clone(),
            })),
        }
    }

    fn request_input_monitoring_access(&mut self) -> bool {
        self.inner.request_input_monitoring_access()
    }
}

struct StagedCandidateBackend {
    fail_start: bool,
    candidate_stops: Rc<Cell<usize>>,
}

impl ShortcutBackend for StagedCandidateBackend {
    fn start(&mut self, _trigger: Rc<dyn Fn()>) -> Result<(), String> {
        if self.fail_start {
            Err("forced staged backend failure".to_owned())
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) {
        self.candidate_stops
            .set(self.candidate_stops.get().saturating_add(1));
    }

    fn monitoring_state(&self) -> MonitoringState {
        MonitoringState::CarbonHotKey
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MonitoringState, ShortcutCapture, ShortcutRecordingAction, ShortcutRecordingEvent,
        StagedShortcutCase, monitoring_state_label, shortcut_recording_action,
        staged_shortcut_case,
    };
    use muxy_core::shortcuts::{COMMAND, KeyCombo};

    #[test]
    fn quick_terminal_settings_recorder_handles_capture_cancel_rejection_and_stale_events() {
        let capture = ShortcutCapture {
            combo: KeyCombo::new("space", COMMAND),
            virtual_key_code: 49,
        };
        assert!(matches!(
            shortcut_recording_action(2, 1, ShortcutRecordingEvent::Captured(capture.clone())),
            ShortcutRecordingAction::Ignore
        ));
        assert!(matches!(
            shortcut_recording_action(2, 2, ShortcutRecordingEvent::Captured(capture)),
            ShortcutRecordingAction::Capture(ShortcutCapture {
                virtual_key_code: 49,
                ..
            })
        ));
        assert!(matches!(
            shortcut_recording_action(2, 2, ShortcutRecordingEvent::Cancelled),
            ShortcutRecordingAction::Cancel
        ));
        assert!(matches!(
            shortcut_recording_action(
                2,
                2,
                ShortcutRecordingEvent::Rejected("unsupported".to_owned())
            ),
            ShortcutRecordingAction::Reject(error) if error == "unsupported"
        ));
    }

    #[test]
    fn quick_terminal_settings_status_labels_match_runtime_monitoring() {
        assert_eq!(monitoring_state_label(MonitoringState::Stopped), "Inactive");
        assert_eq!(
            monitoring_state_label(MonitoringState::Unavailable),
            "Unavailable"
        );
        assert_eq!(
            monitoring_state_label(MonitoringState::LocalOnly),
            "Local only"
        );
        assert_eq!(
            monitoring_state_label(MonitoringState::SystemWide),
            "System-wide"
        );
        assert_eq!(
            monitoring_state_label(MonitoringState::CarbonHotKey),
            "Active system-wide"
        );
    }

    #[test]
    fn quick_terminal_shortcut_staged_control_is_test_process_only() {
        assert_eq!(staged_shortcut_case(false, Some("backend-failure")), None);
        assert_eq!(staged_shortcut_case(true, Some("unknown")), None);
        assert_eq!(
            staged_shortcut_case(true, Some("backend-failure")),
            Some(StagedShortcutCase::BackendFailure)
        );
        assert_eq!(
            staged_shortcut_case(true, Some("persistence-failure")),
            Some(StagedShortcutCase::PersistenceFailure)
        );
    }
}
