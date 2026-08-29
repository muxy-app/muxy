#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoubleShiftConfiguration {
    pub maximum_tap_duration: f64,
    pub maximum_interval: f64,
}

impl Default for DoubleShiftConfiguration {
    fn default() -> Self {
        Self {
            maximum_tap_duration: 0.35,
            maximum_interval: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoubleShiftInput {
    ModifierChange {
        shift_pressed: bool,
        other_modifier_pressed: bool,
        timestamp: f64,
    },
    KeyDown {
        shift_pressed: bool,
        timestamp: f64,
    },
    PointerDown {
        shift_pressed: bool,
        timestamp: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    FirstPress(f64),
    AwaitingSecondPress(f64),
    SecondPress(f64),
    BlockedUntilShiftRelease,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoubleShiftDetector {
    configuration: DoubleShiftConfiguration,
    state: State,
    last_timestamp: Option<f64>,
}

impl Default for DoubleShiftDetector {
    fn default() -> Self {
        Self::new(DoubleShiftConfiguration::default())
    }
}

impl DoubleShiftDetector {
    pub fn new(configuration: DoubleShiftConfiguration) -> Self {
        Self {
            configuration,
            state: State::Idle,
            last_timestamp: None,
        }
    }

    pub fn process(&mut self, input: DoubleShiftInput) -> bool {
        let timestamp = match input {
            DoubleShiftInput::ModifierChange { timestamp, .. }
            | DoubleShiftInput::KeyDown { timestamp, .. }
            | DoubleShiftInput::PointerDown { timestamp, .. } => timestamp,
        };
        if self
            .last_timestamp
            .is_some_and(|last_timestamp| timestamp < last_timestamp)
        {
            self.state = State::Idle;
        }
        self.last_timestamp = Some(timestamp);
        match input {
            DoubleShiftInput::KeyDown { shift_pressed, .. }
            | DoubleShiftInput::PointerDown { shift_pressed, .. } => {
                self.state = if shift_pressed {
                    State::BlockedUntilShiftRelease
                } else {
                    State::Idle
                };
                false
            }
            DoubleShiftInput::ModifierChange {
                shift_pressed,
                other_modifier_pressed,
                timestamp,
            } => self.process_modifier_change(shift_pressed, other_modifier_pressed, timestamp),
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.last_timestamp = None;
    }

    fn process_modifier_change(
        &mut self,
        shift_pressed: bool,
        other_modifier_pressed: bool,
        timestamp: f64,
    ) -> bool {
        if other_modifier_pressed {
            self.state = if shift_pressed {
                State::BlockedUntilShiftRelease
            } else {
                State::Idle
            };
            return false;
        }
        if self.state == State::BlockedUntilShiftRelease {
            if !shift_pressed {
                self.state = State::Idle;
            }
            return false;
        }
        self.expire_state(timestamp, shift_pressed);
        if shift_pressed {
            match self.state {
                State::Idle => self.state = State::FirstPress(timestamp),
                State::AwaitingSecondPress(_) => self.state = State::SecondPress(timestamp),
                State::FirstPress(_) | State::SecondPress(_) | State::BlockedUntilShiftRelease => {}
            }
            return false;
        }
        match self.state {
            State::FirstPress(pressed_at) => {
                if timestamp - pressed_at <= self.configuration.maximum_tap_duration {
                    self.state = State::AwaitingSecondPress(timestamp);
                } else {
                    self.state = State::Idle;
                }
                false
            }
            State::SecondPress(pressed_at) => {
                self.state = State::Idle;
                timestamp - pressed_at <= self.configuration.maximum_tap_duration
            }
            State::Idle | State::AwaitingSecondPress(_) | State::BlockedUntilShiftRelease => false,
        }
    }

    fn expire_state(&mut self, timestamp: f64, shift_pressed: bool) {
        match self.state {
            State::FirstPress(pressed_at) | State::SecondPress(pressed_at)
                if timestamp - pressed_at > self.configuration.maximum_tap_duration =>
            {
                self.state = if shift_pressed {
                    State::BlockedUntilShiftRelease
                } else {
                    State::Idle
                };
            }
            State::AwaitingSecondPress(released_at)
                if timestamp - released_at > self.configuration.maximum_interval =>
            {
                self.state = State::Idle;
            }
            State::Idle
            | State::FirstPress(_)
            | State::AwaitingSecondPress(_)
            | State::SecondPress(_)
            | State::BlockedUntilShiftRelease => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DoubleShiftDetector, DoubleShiftInput};

    fn modifier(
        shift_pressed: bool,
        other_modifier_pressed: bool,
        timestamp: f64,
    ) -> DoubleShiftInput {
        DoubleShiftInput::ModifierChange {
            shift_pressed,
            other_modifier_pressed,
            timestamp,
        }
    }

    fn tap(detector: &mut DoubleShiftDetector, start: f64) -> bool {
        assert!(!detector.process(modifier(true, false, start)));
        detector.process(modifier(false, false, start + 0.1))
    }

    #[test]
    fn quick_terminal_double_shift_requires_two_complete_taps() {
        let mut detector = DoubleShiftDetector::default();
        assert!(!tap(&mut detector, 0.0));
        assert!(!detector.process(modifier(true, false, 0.2)));
        assert!(detector.process(modifier(false, false, 0.3)));
        assert!(!detector.process(modifier(false, false, 0.31)));
    }

    #[test]
    fn quick_terminal_double_shift_repeated_press_never_replaces_release() {
        let mut detector = DoubleShiftDetector::default();
        assert!(!detector.process(modifier(true, false, 0.0)));
        assert!(!detector.process(modifier(true, false, 0.1)));
        assert!(!detector.process(modifier(true, false, 0.2)));
        assert!(!detector.process(modifier(false, false, 0.6)));
    }

    #[test]
    fn quick_terminal_double_shift_intervening_inputs_reset_and_block() {
        for input in [
            DoubleShiftInput::KeyDown {
                shift_pressed: false,
                timestamp: 0.2,
            },
            DoubleShiftInput::PointerDown {
                shift_pressed: false,
                timestamp: 0.2,
            },
            modifier(false, true, 0.2),
        ] {
            let mut detector = DoubleShiftDetector::default();
            assert!(!tap(&mut detector, 0.0));
            assert!(!detector.process(input));
            assert!(!tap(&mut detector, 0.3));
        }
        let mut held = DoubleShiftDetector::default();
        assert!(!held.process(DoubleShiftInput::KeyDown {
            shift_pressed: true,
            timestamp: 0.0,
        }));
        assert!(!held.process(modifier(true, false, 0.1)));
        assert!(!held.process(modifier(false, false, 0.2)));
        assert!(!tap(&mut held, 0.3));
    }

    #[test]
    fn quick_terminal_double_shift_timeouts_and_reversed_time_reset_safely() {
        let mut detector = DoubleShiftDetector::default();
        assert!(!detector.process(modifier(true, false, 0.0)));
        assert!(!detector.process(modifier(false, false, 0.4)));
        assert!(!tap(&mut detector, 1.0));
        assert!(!tap(&mut detector, 1.7));
        assert!(!detector.process(modifier(true, false, 1.0)));
        assert!(!detector.process(modifier(false, false, 1.1)));
    }
}
