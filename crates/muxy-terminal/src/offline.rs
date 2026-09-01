use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const MIN_SCAN_INTERVAL: Duration = Duration::from_secs(5);
const MAX_SCAN_INTERVAL: Duration = Duration::from_secs(30);
const SHELL_PROCESS_NAMES: [&str; 11] = [
    "bash", "csh", "dash", "fish", "ksh", "nu", "pwsh", "sh", "tcsh", "xonsh", "zsh",
];
const MULTIPLEXER_PROCESS_NAMES: [&str; 3] = ["screen", "tmux", "zellij"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfflineCandidate {
    pub has_live_surface: bool,
    pub is_already_offline: bool,
    pub invisible_duration: Option<Duration>,
    pub is_idle: bool,
}

pub fn is_idle(has_running_process: bool, is_alternate_screen: bool) -> bool {
    !has_running_process && !is_alternate_screen
}

pub fn has_running_process(
    foreground_process_name: Option<&str>,
    foreground_process_arguments: Option<&[String]>,
    is_shell_command_running: bool,
) -> bool {
    is_shell_command_running
        || !is_interactive_shell(foreground_process_name, foreground_process_arguments)
}

pub fn keeps_awake(is_on_screen: bool, is_focused: bool) -> bool {
    is_on_screen && is_focused
}

pub fn should_take_offline(
    candidate: OfflineCandidate,
    is_enabled: bool,
    idle_threshold: Duration,
) -> bool {
    is_enabled
        && candidate.has_live_surface
        && !candidate.is_already_offline
        && candidate
            .invisible_duration
            .is_some_and(|duration| duration >= idle_threshold)
        && candidate.is_idle
}

pub fn should_present_placeholder(
    is_visible: bool,
    is_offline: bool,
    is_remotely_owned: bool,
) -> bool {
    is_visible && is_offline && !is_remotely_owned
}

pub fn scan_interval(idle_threshold: Duration) -> Duration {
    idle_threshold.clamp(MIN_SCAN_INTERVAL, MAX_SCAN_INTERVAL)
}

pub fn is_shell(process_name: Option<&str>) -> bool {
    normalized_process_name(process_name)
        .is_some_and(|name| SHELL_PROCESS_NAMES.contains(&name.as_str()))
}

pub fn accepts_shell_input(process_name: Option<&str>) -> bool {
    normalized_process_name(process_name).is_some_and(|name| {
        SHELL_PROCESS_NAMES.contains(&name.as_str())
            || MULTIPLEXER_PROCESS_NAMES.contains(&name.as_str())
    })
}

pub fn is_interactive_shell(process_name: Option<&str>, arguments: Option<&[String]>) -> bool {
    let Some(process_name) = normalized_process_name(process_name) else {
        return false;
    };
    if !SHELL_PROCESS_NAMES.contains(&process_name.as_str()) {
        return false;
    }
    let Some(arguments) = arguments.filter(|arguments| !arguments.is_empty()) else {
        return false;
    };
    let mut shell_arguments = &arguments[1..];
    if process_name == "bash" && shell_arguments.first().map(String::as_str) == Some("--posix") {
        shell_arguments = &shell_arguments[1..];
    }
    if process_name == "nu"
        && shell_arguments
            .get(..2)
            .is_some_and(|prefix| prefix[0] == "--execute" && prefix[1] == "use ghostty *")
    {
        shell_arguments = &shell_arguments[2..];
    }
    shell_arguments.iter().all(|argument| {
        let argument = argument.to_ascii_lowercase();
        argument == "--interactive"
            || argument == "--login"
            || argument.strip_prefix('-').is_some_and(|options| {
                !options.is_empty()
                    && options
                        .bytes()
                        .all(|option| option == b'i' || option == b'l')
            })
    })
}

fn normalized_process_name(process_name: Option<&str>) -> Option<String> {
    process_name
        .filter(|name| !name.is_empty())
        .map(|name| name.strip_prefix('-').unwrap_or(name))
        .filter(|name| !name.is_empty())
        .map(str::to_ascii_lowercase)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentCommandActivity {
    Idle,
    Running,
    Unknown,
}

pub fn uses_persistent_session(
    preference_enabled: bool,
    is_remote: bool,
    is_available: bool,
) -> bool {
    preference_enabled && is_available && !is_remote
}

pub fn persistent_session_is_idle(
    activity: Option<PersistentCommandActivity>,
    is_shell_command_running: bool,
    is_alternate_screen: bool,
) -> bool {
    let Some(PersistentCommandActivity::Idle) = activity else {
        return false;
    };
    is_idle(is_shell_command_running, is_alternate_screen)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionAvailability<T> {
    Found(T),
    Missing,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryState {
    Ready,
    Reconnecting { attempt: u8 },
    Unreachable,
    Missing,
    InvalidIdentity { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    Reconnect,
    StartFresh,
}

impl RecoveryState {
    pub const fn action(&self) -> Option<RecoveryAction> {
        match self {
            Self::Unreachable => Some(RecoveryAction::Reconnect),
            Self::Missing => Some(RecoveryAction::StartFresh),
            Self::Ready | Self::Reconnecting { .. } | Self::InvalidIdentity { .. } => None,
        }
    }
}

#[derive(Clone, Default)]
pub struct ShellActivityTracker {
    state: Arc<Mutex<ActivityState>>,
}

#[derive(Default)]
struct ActivityState {
    parser: SemanticCommandParser,
    command_running: bool,
    reliable: bool,
    generation: u64,
}

#[derive(Clone)]
pub struct ShellActivitySession {
    tracker: ShellActivityTracker,
    generation: u64,
}

impl ShellActivityTracker {
    pub fn begin_session(&self) -> ShellActivitySession {
        let mut state = self.lock();
        state.generation = state.generation.wrapping_add(1);
        state.parser.reset();
        state.command_running = false;
        state.reliable = true;
        ShellActivitySession {
            tracker: self.clone(),
            generation: state.generation,
        }
    }

    pub fn is_command_running(&self) -> bool {
        self.lock().command_running
    }

    pub fn activity(&self) -> PersistentCommandActivity {
        let state = self.lock();
        if !state.reliable {
            PersistentCommandActivity::Unknown
        } else if state.command_running {
            PersistentCommandActivity::Running
        } else {
            PersistentCommandActivity::Idle
        }
    }

    fn lock(&self) -> MutexGuard<'_, ActivityState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ShellActivitySession {
    pub fn record_output(&self, bytes: &[u8]) {
        let mut state = self.tracker.lock();
        if state.generation != self.generation {
            return;
        }
        for byte in bytes {
            if let Some(event) = state.parser.event(*byte) {
                state.command_running = matches!(event, SemanticEvent::Started);
            }
        }
    }

    pub fn record_gap(&self) {
        let mut state = self.tracker.lock();
        if state.generation != self.generation {
            return;
        }
        state.parser.reset();
        state.command_running = false;
        state.reliable = false;
    }

    pub fn invalidate(&self) {
        let mut state = self.tracker.lock();
        if state.generation != self.generation {
            return;
        }
        state.generation = state.generation.wrapping_add(1);
        state.parser.reset();
        state.command_running = false;
        state.reliable = false;
    }
}

#[derive(Clone, Copy)]
enum SemanticEvent {
    Started,
    Finished,
}

#[derive(Clone, Copy, Default)]
enum ParserState {
    #[default]
    Idle,
    Escape,
    Osc,
    OscOne,
    OscThirteen,
    OscOneThirtyThree,
    Action,
    Payload(SemanticEvent),
    PayloadEscape(SemanticEvent),
}

#[derive(Default)]
struct SemanticCommandParser {
    state: ParserState,
}

impl SemanticCommandParser {
    fn event(&mut self, byte: u8) -> Option<SemanticEvent> {
        let mut event = None;
        self.state = match self.state {
            ParserState::Idle => escape_or_idle(byte),
            ParserState::Escape => {
                if byte == b']' {
                    ParserState::Osc
                } else {
                    escape_or_idle(byte)
                }
            }
            ParserState::Osc => expected(byte, b'1', ParserState::OscOne),
            ParserState::OscOne => expected(byte, b'3', ParserState::OscThirteen),
            ParserState::OscThirteen => expected(byte, b'3', ParserState::OscOneThirtyThree),
            ParserState::OscOneThirtyThree => expected(byte, b';', ParserState::Action),
            ParserState::Action => match byte {
                b'C' => ParserState::Payload(SemanticEvent::Started),
                b'D' => ParserState::Payload(SemanticEvent::Finished),
                _ => escape_or_idle(byte),
            },
            ParserState::Payload(held) => {
                if byte == 0x07 {
                    event = Some(held);
                    ParserState::Idle
                } else if byte == 0x1B {
                    ParserState::PayloadEscape(held)
                } else {
                    ParserState::Payload(held)
                }
            }
            ParserState::PayloadEscape(held) => {
                if byte == b'\\' {
                    event = Some(held);
                    ParserState::Idle
                } else if byte == b']' {
                    ParserState::Osc
                } else if byte == 0x1B {
                    ParserState::PayloadEscape(held)
                } else {
                    ParserState::Payload(held)
                }
            }
        };
        event
    }

    fn reset(&mut self) {
        self.state = ParserState::Idle;
    }
}

fn escape_or_idle(byte: u8) -> ParserState {
    if byte == 0x1B {
        ParserState::Escape
    } else {
        ParserState::Idle
    }
}

fn expected(byte: u8, target: u8, matched: ParserState) -> ParserState {
    if byte == target {
        matched
    } else {
        escape_or_idle(byte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> OfflineCandidate {
        OfflineCandidate {
            has_live_surface: true,
            is_already_offline: false,
            invisible_duration: Some(Duration::from_secs(600)),
            is_idle: true,
        }
    }

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn idle_requires_no_running_process_and_no_alternate_screen() {
        assert!(is_idle(false, false));
        assert!(!is_idle(true, false));
        assert!(!is_idle(false, true));
        assert!(!is_idle(true, true));
    }

    #[test]
    fn retained_shell_prompt_fixtures_are_idle() {
        for (name, values) in [
            ("zsh", vec!["-zsh"]),
            ("-zsh", vec!["-zsh"]),
            ("bash", vec!["bash", "-l"]),
            ("bash", vec!["bash", "--posix"]),
            ("fish", vec!["fish", "-i"]),
            ("nu", vec!["nu", "--execute", "use ghostty *"]),
            ("xonsh", vec!["xonsh"]),
        ] {
            assert!(!has_running_process(
                Some(name),
                Some(&arguments(&values)),
                false
            ));
        }
    }

    #[test]
    fn commands_unknown_processes_and_scripts_are_busy() {
        for (name, values) in [
            ("bun", vec!["bun", "run", "agent"]),
            ("node", vec!["node", "agent.js"]),
            ("ssh", vec!["ssh", "host"]),
            ("make", vec!["make", "test"]),
            ("bash", vec!["bash", "-c", "sleep 600"]),
            ("bash", vec!["bash", "--posix", "-c", "sleep 600"]),
            ("nu", vec!["nu", "--execute", "sleep 600"]),
            (
                "nu",
                vec!["nu", "--execute", "use ghostty *", "--command", "sleep 600"],
            ),
            ("zsh", vec!["zsh", "script.zsh"]),
            ("pwsh", vec!["pwsh", "-Command", "Start-Sleep 600"]),
        ] {
            assert!(has_running_process(
                Some(name),
                Some(&arguments(&values)),
                false
            ));
        }
        assert!(has_running_process(None, None, false));
        assert!(has_running_process(Some(""), Some(&[]), false));
        assert!(has_running_process(Some("zsh"), None, false));
        assert!(has_running_process(
            Some("someunknownshell"),
            Some(&arguments(&["someunknownshell"])),
            false
        ));
        assert!(has_running_process(
            Some("zsh"),
            Some(&arguments(&["-zsh"])),
            true
        ));
    }

    #[test]
    fn shell_and_multiplexer_classification_matches_retained_behavior() {
        assert!(is_shell(Some("zsh")));
        assert!(is_shell(Some("-ZSH")));
        assert!(accepts_shell_input(Some("zsh")));
        assert!(accepts_shell_input(Some("tmux")));
        assert!(accepts_shell_input(Some("screen")));
        assert!(!accepts_shell_input(Some("ssh")));
    }

    #[test]
    fn focus_visibility_threshold_and_candidate_gates_match_retained_behavior() {
        assert!(keeps_awake(true, true));
        assert!(!keeps_awake(true, false));
        assert!(!keeps_awake(false, true));
        assert!(!keeps_awake(false, false));
        assert!(should_take_offline(
            OfflineCandidate {
                invisible_duration: Some(Duration::from_secs(300)),
                ..candidate()
            },
            true,
            Duration::from_secs(300)
        ));
        for value in [
            OfflineCandidate {
                has_live_surface: false,
                ..candidate()
            },
            OfflineCandidate {
                is_already_offline: true,
                ..candidate()
            },
            OfflineCandidate {
                invisible_duration: None,
                ..candidate()
            },
            OfflineCandidate {
                invisible_duration: Some(Duration::from_secs(299)),
                ..candidate()
            },
            OfflineCandidate {
                is_idle: false,
                ..candidate()
            },
        ] {
            assert!(!should_take_offline(value, true, Duration::from_secs(300)));
        }
        assert!(!should_take_offline(
            candidate(),
            false,
            Duration::from_secs(300)
        ));
    }

    #[test]
    fn scan_interval_is_clamped_to_five_through_thirty_seconds() {
        assert_eq!(
            scan_interval(Duration::from_secs(3)),
            Duration::from_secs(5)
        );
        assert_eq!(
            scan_interval(Duration::from_secs(10)),
            Duration::from_secs(10)
        );
        assert_eq!(
            scan_interval(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
        assert_eq!(
            scan_interval(Duration::from_secs(300)),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn persistent_policy_excludes_remote_unavailable_and_unknown_activity() {
        assert!(uses_persistent_session(true, false, true));
        assert!(!uses_persistent_session(false, false, true));
        assert!(!uses_persistent_session(true, false, false));
        assert!(!uses_persistent_session(true, true, true));
        assert!(persistent_session_is_idle(
            Some(PersistentCommandActivity::Idle),
            false,
            false
        ));
        assert!(!persistent_session_is_idle(None, false, false));
        assert!(!persistent_session_is_idle(
            Some(PersistentCommandActivity::Unknown),
            false,
            false
        ));
        assert!(!persistent_session_is_idle(
            Some(PersistentCommandActivity::Running),
            false,
            false
        ));
        assert!(!persistent_session_is_idle(
            Some(PersistentCommandActivity::Idle),
            true,
            false
        ));
        assert!(!persistent_session_is_idle(
            Some(PersistentCommandActivity::Idle),
            false,
            true
        ));
    }

    #[test]
    fn placeholder_and_recovery_actions_preserve_failure_distinctions() {
        assert!(should_present_placeholder(true, true, false));
        assert!(!should_present_placeholder(false, true, false));
        assert!(!should_present_placeholder(true, false, false));
        assert!(!should_present_placeholder(true, true, true));
        assert_eq!(
            RecoveryState::Unreachable.action(),
            Some(RecoveryAction::Reconnect)
        );
        assert_eq!(
            RecoveryState::Missing.action(),
            Some(RecoveryAction::StartFresh)
        );
        assert_eq!(RecoveryState::Ready.action(), None);
        assert_ne!(
            SessionAvailability::<()>::Missing,
            SessionAvailability::Unreachable
        );
    }

    #[test]
    fn tracker_follows_complete_split_and_long_osc_133_sequences() {
        let tracker = ShellActivityTracker::default();
        let session = tracker.begin_session();
        session.record_output(b"\x1b]133;C\x07");
        assert!(tracker.is_command_running());
        session.record_output(b"\x1b]133;D;0\x07");
        assert!(!tracker.is_command_running());
        session.record_output(b"\x1b]13");
        session.record_output(b"3;C;cmdline=read");
        assert!(!tracker.is_command_running());
        session.record_output(b"\x07");
        assert!(tracker.is_command_running());
        session.record_output(b"\x1b]133;D");
        session.record_output(&[0x1B, b'\\']);
        assert!(!tracker.is_command_running());
        let mut long = b"\x1b]133;C;cmdline=".to_vec();
        long.extend(vec![b'x'; 16_384]);
        long.push(0x07);
        session.record_output(&long);
        assert!(tracker.is_command_running());
    }

    #[test]
    fn tracker_ignores_incomplete_unrelated_and_stale_session_output() {
        let tracker = ShellActivityTracker::default();
        let stale = tracker.begin_session();
        stale.record_output(b"output \x1b]133;C");
        assert!(!tracker.is_command_running());
        stale.record_output(b"\x1b]133;A\x07\x1b]133;B\x07");
        assert!(!tracker.is_command_running());
        stale.record_output(b"\x1b]133;C\x07");
        let current = tracker.begin_session();
        current.record_output(b"\x1b]133;C\x07");
        stale.record_output(b"\x1b]133;D\x07");
        assert!(tracker.is_command_running());
        current.invalidate();
        assert!(!tracker.is_command_running());
        assert_eq!(tracker.activity(), PersistentCommandActivity::Unknown);
    }

    #[test]
    fn tracker_stays_unknown_after_a_data_gap_until_a_new_session() {
        let tracker = ShellActivityTracker::default();
        assert_eq!(tracker.activity(), PersistentCommandActivity::Unknown);
        let session = tracker.begin_session();
        assert_eq!(tracker.activity(), PersistentCommandActivity::Idle);
        session.record_output(b"\x1b]133;C\x07");
        assert_eq!(tracker.activity(), PersistentCommandActivity::Running);
        session.record_gap();
        session.record_output(b"\x1b]133;D\x07");
        assert_eq!(tracker.activity(), PersistentCommandActivity::Unknown);
        tracker.begin_session();
        assert_eq!(tracker.activity(), PersistentCommandActivity::Idle);
    }
}
