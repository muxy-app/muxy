use super::messages::MAX_REPLAY_BYTES;
use super::terminal_stream::{
    SCREEN_CONTROL_TAIL_LENGTH, next_alternate_screen_sequence, trailing_safe_end,
};
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaySnapshot {
    pub generation: u64,
    pub last_sequence: Option<u64>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplayParserState {
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    Osc,
    OscEscape,
    StringControl,
    StringControlEscape,
}

#[derive(Clone, Debug)]
pub struct ReplayBuffer {
    capacity: usize,
    storage: Vec<u8>,
    discarded: bool,
    storage_start_offset: u64,
    stream_offset: u64,
    safe_starts: VecDeque<u64>,
    parser_state: ReplayParserState,
    alternate_screen_active: bool,
    screen_control_tail: Vec<u8>,
    generation: u64,
    last_sequence: Option<u64>,
}

impl ReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.min(MAX_REPLAY_BYTES),
            storage: Vec::new(),
            discarded: false,
            storage_start_offset: 0,
            stream_offset: 0,
            safe_starts: VecDeque::new(),
            parser_state: ReplayParserState::Ground,
            alternate_screen_active: false,
            screen_control_tail: Vec::new(),
            generation: 0,
            last_sequence: None,
        }
    }

    pub fn append(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), ReplayError> {
        if self
            .last_sequence
            .is_some_and(|last| last.checked_add(1) != Some(sequence))
        {
            return Err(ReplayError::OutOfOrder {
                previous: self.last_sequence,
                received: sequence,
            });
        }
        self.last_sequence = Some(sequence);
        self.append_for_replay(bytes);
        Ok(())
    }

    pub fn snapshot(&self) -> ReplaySnapshot {
        let bytes = if self.alternate_screen_active {
            Vec::new()
        } else {
            self.safe_bytes()
        };
        ReplaySnapshot {
            generation: self.generation,
            last_sequence: self.last_sequence,
            bytes,
        }
    }

    pub fn byte_count(&self) -> usize {
        self.storage.len()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_alternate_screen_active(&self) -> bool {
        self.alternate_screen_active
    }

    pub fn clear(&mut self) {
        self.clear_storage();
        self.alternate_screen_active = false;
        self.screen_control_tail.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    fn append_for_replay(&mut self, bytes: &[u8]) {
        if self.capacity == 0 || bytes.is_empty() {
            return;
        }
        let previous_tail = self.screen_control_tail.clone();
        let mut combined = previous_tail.clone();
        combined.extend_from_slice(bytes);
        let new_byte_offset = previous_tail.len();
        let mut index = 0;
        let mut unhandled_new_byte_index = 0;
        while index < combined.len() {
            if self.alternate_screen_active {
                let Some(leave) = next_alternate_screen_sequence(&combined, index, false) else {
                    self.update_screen_control_tail(&combined);
                    return;
                };
                self.clear_storage();
                self.alternate_screen_active = false;
                self.generation = self.generation.wrapping_add(1);
                unhandled_new_byte_index =
                    unhandled_new_byte_index.max(leave.end.saturating_sub(new_byte_offset));
                index = leave.end;
                continue;
            }
            let Some(enter) = next_alternate_screen_sequence(&combined, index, true) else {
                if unhandled_new_byte_index < bytes.len() {
                    self.append_storage(&bytes[unhandled_new_byte_index..]);
                }
                self.update_screen_control_tail(&combined);
                return;
            };
            self.clear_storage();
            self.alternate_screen_active = true;
            self.generation = self.generation.wrapping_add(1);
            unhandled_new_byte_index =
                unhandled_new_byte_index.max(enter.end.saturating_sub(new_byte_offset));
            index = enter.end;
        }
        if !self.alternate_screen_active && unhandled_new_byte_index < bytes.len() {
            self.append_storage(&bytes[unhandled_new_byte_index..]);
        }
        self.update_screen_control_tail(&combined);
    }

    fn append_storage(&mut self, bytes: &[u8]) {
        if bytes.is_empty() || self.capacity == 0 {
            return;
        }
        for &byte in bytes {
            self.stream_offset = self.stream_offset.wrapping_add(1);
            if self.advance_parser(byte) {
                self.safe_starts.push_back(self.stream_offset);
            }
        }
        if bytes.len() >= self.capacity {
            self.discarded |= !self.storage.is_empty() || bytes.len() > self.capacity;
            self.storage.clear();
            self.storage
                .extend_from_slice(&bytes[bytes.len() - self.capacity..]);
            self.storage_start_offset = self.stream_offset.wrapping_sub(self.capacity as u64);
        } else {
            let overflow = self
                .storage
                .len()
                .saturating_add(bytes.len())
                .saturating_sub(self.capacity);
            if overflow > 0 {
                self.storage.drain(..overflow);
                self.storage_start_offset = self.storage_start_offset.wrapping_add(overflow as u64);
                self.discarded = true;
            }
            self.storage.extend_from_slice(bytes);
        }
        while self
            .safe_starts
            .front()
            .is_some_and(|offset| *offset < self.storage_start_offset)
        {
            self.safe_starts.pop_front();
        }
    }

    fn safe_bytes(&self) -> Vec<u8> {
        let start = if self.discarded {
            let Some(offset) = self
                .safe_starts
                .iter()
                .copied()
                .find(|offset| *offset > self.storage_start_offset)
            else {
                return Vec::new();
            };
            offset.wrapping_sub(self.storage_start_offset) as usize
        } else {
            0
        };
        if start >= self.storage.len() {
            return Vec::new();
        }
        let end = trailing_safe_end(&self.storage[start..]);
        self.storage[start..start + end].to_vec()
    }

    fn clear_storage(&mut self) {
        self.storage.clear();
        self.discarded = false;
        self.storage_start_offset = 0;
        self.stream_offset = 0;
        self.safe_starts.clear();
        self.parser_state = ReplayParserState::Ground;
    }

    fn advance_parser(&mut self, byte: u8) -> bool {
        use ReplayParserState::{
            Csi, Escape, EscapeIntermediate, Ground, Osc, OscEscape, StringControl,
            StringControlEscape,
        };
        let mut safe_start = false;
        self.parser_state = match (self.parser_state, byte) {
            (Ground, 0x1b) => Escape,
            (Ground, b'\n' | b'\r') => {
                safe_start = true;
                Ground
            }
            (Ground, _) => Ground,
            (Escape, b'[') => Csi,
            (Escape, b']') => Osc,
            (Escape, b'P' | b'X' | b'^' | b'_') => StringControl,
            (Escape, 0x20..=0x2f) => EscapeIntermediate,
            (Escape, _) => Ground,
            (EscapeIntermediate, 0x30..=0x7e) => Ground,
            (EscapeIntermediate, _) => EscapeIntermediate,
            (Csi, 0x40..=0x7e) => Ground,
            (Csi, _) => Csi,
            (Osc, 0x07) => Ground,
            (Osc, 0x1b) => OscEscape,
            (Osc, _) => Osc,
            (OscEscape, b'\\') => Ground,
            (OscEscape, 0x1b) => OscEscape,
            (OscEscape, 0x07) => Ground,
            (OscEscape, _) => Osc,
            (StringControl, 0x1b) => StringControlEscape,
            (StringControl, _) => StringControl,
            (StringControlEscape, b'\\') => Ground,
            (StringControlEscape, 0x1b) => StringControlEscape,
            (StringControlEscape, _) => StringControl,
        };
        safe_start
    }

    fn update_screen_control_tail(&mut self, bytes: &[u8]) {
        let start = bytes.len().saturating_sub(SCREEN_CONTROL_TAIL_LENGTH);
        self.screen_control_tail.clear();
        self.screen_control_tail.extend_from_slice(&bytes[start..]);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayError {
    OutOfOrder {
        previous: Option<u64>,
        received: u64,
    },
}

impl Display for ReplayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfOrder { previous, received } => {
                write!(
                    formatter,
                    "output sequence {received} does not follow {previous:?}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_is_bounded_and_drops_partial_leading_line() {
        let mut replay = ReplayBuffer::new(12);
        replay.append(1, b"first\n").unwrap();
        replay.append(2, b"second\nthird").unwrap();
        assert!(replay.byte_count() <= 12);
        assert_eq!(replay.snapshot().bytes, b"third");
    }

    #[test]
    fn replay_removes_partial_utf8_and_escape_tails() {
        let mut replay = ReplayBuffer::new(64);
        replay.append(1, b"ready ").unwrap();
        replay.append(2, &[0xe2, 0x82]).unwrap();
        assert_eq!(replay.snapshot().bytes, b"ready ");
        replay
            .append(3, &[0xac, b' ', 0x1b, b'[', b'3', b'1'])
            .unwrap();
        assert_eq!(replay.snapshot().bytes, "ready € ".as_bytes());
        replay.append(4, b"mred").unwrap();
        assert_eq!(replay.snapshot().bytes, "ready € \x1b[31mred".as_bytes());
    }

    #[test]
    fn alternate_screen_starts_clean_generations_across_chunk_boundaries() {
        let mut replay = ReplayBuffer::new(128);
        replay.append(1, b"before\n\x1b[?10").unwrap();
        replay.append(2, b"49halt-screen").unwrap();
        assert!(replay.is_alternate_screen_active());
        assert!(replay.snapshot().bytes.is_empty());
        assert_eq!(replay.generation(), 1);
        replay.append(3, b"ignored\x1b[?1049").unwrap();
        replay.append(4, b"lafter").unwrap();
        assert!(!replay.is_alternate_screen_active());
        assert_eq!(replay.snapshot().bytes, b"after");
        assert_eq!(replay.generation(), 2);
    }

    #[test]
    fn replay_truncation_never_starts_inside_control_payloads() {
        for control in [
            b"\x1b]0;TITLE\x07".as_slice(),
            b"\x1b[31m".as_slice(),
            b"\x1bPpayload\x1b\\".as_slice(),
        ] {
            let mut full = control.to_vec();
            full.extend_from_slice(b"visible\nready");
            for cut in 1..control.len() {
                let mut replay = ReplayBuffer::new(full.len() - cut);
                let split = control.len() / 2;
                replay.append(1, &full[..split]).unwrap();
                replay.append(2, &full[split..]).unwrap();
                assert_eq!(replay.snapshot().bytes, b"ready", "cut {cut}");
            }
        }
    }

    #[test]
    fn replay_requires_monotonic_contiguous_output_sequences() {
        let mut replay = ReplayBuffer::new(32);
        replay.append(8, b"a").unwrap();
        assert_eq!(
            replay.append(10, b"b"),
            Err(ReplayError::OutOfOrder {
                previous: Some(8),
                received: 10,
            })
        );
        assert_eq!(replay.snapshot().last_sequence, Some(8));

        let mut exhausted = ReplayBuffer::new(32);
        exhausted.append(u64::MAX, b"a").unwrap();
        assert!(matches!(
            exhausted.append(u64::MAX, b"b"),
            Err(ReplayError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn replay_capacity_is_capped_by_protocol_limit() {
        let mut replay = ReplayBuffer::new(MAX_REPLAY_BYTES * 2);
        replay
            .append(1, &vec![b'x'; MAX_REPLAY_BYTES + 50])
            .unwrap();
        assert_eq!(replay.byte_count(), MAX_REPLAY_BYTES);
    }
}
