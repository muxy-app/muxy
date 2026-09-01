use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentCommandActivity {
    Idle,
    Running,
    Unknown,
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
