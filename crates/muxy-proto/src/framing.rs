use thiserror::Error;

pub const DEFAULT_MAX_INPUT_BYTES: usize = 128 * 1024;
pub const MAX_READ_BYTES: usize = 4096;
pub const CLI_REPLY_TERMINATOR: u8 = 0;
pub const LINE_TERMINATOR: u8 = b'\n';

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputRecord {
    original: String,
    trimmed: String,
}

impl InputRecord {
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn trimmed(&self) -> &str {
        &self.trimmed
    }

    pub fn into_original(self) -> String {
        self.original
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InputError {
    #[error("input buffer exceeded {limit} bytes")]
    TooLarge { limit: usize },
}

#[derive(Clone, Debug)]
pub struct InputAccumulator {
    buffer: Vec<u8>,
    max_input_bytes: usize,
}

impl Default for InputAccumulator {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_INPUT_BYTES)
    }
}

impl InputAccumulator {
    pub fn new(max_input_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_input_bytes,
        }
    }

    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<InputRecord>, InputError> {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() > self.max_input_bytes {
            return Err(InputError::TooLarge {
                limit: self.max_input_bytes,
            });
        }

        let mut records = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == LINE_TERMINATOR) {
            let mut bytes = self.buffer.drain(..=newline).collect::<Vec<_>>();
            bytes.pop();
            let Ok(original) = String::from_utf8(bytes) else {
                continue;
            };
            let trimmed = original.trim().to_owned();
            if trimmed.is_empty() {
                continue;
            }
            records.push(InputRecord { original, trimmed });
        }
        Ok(records)
    }
}

pub fn frame_cli_reply(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + 1);
    bytes.extend_from_slice(text.as_bytes());
    bytes.push(CLI_REPLY_TERMINATOR);
    bytes
}

pub fn frame_persistent_line(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len() + 1);
    bytes.extend_from_slice(text.as_bytes());
    bytes.push(LINE_TERMINATOR);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_framing_is_exact() {
        assert_eq!(frame_cli_reply("ok"), b"ok\0");
        assert_eq!(frame_cli_reply(""), b"\0");
        assert_eq!(frame_persistent_line("ok"), b"ok\n");
        assert_eq!(frame_persistent_line(""), b"\n");
    }

    #[test]
    fn records_can_span_reads() {
        let mut input = InputAccumulator::default();
        assert!(input.push(b"wire-rec").unwrap().is_empty());
        assert_eq!(input.buffered_bytes(), 8);
        let records = input.push(b"ord\n").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].original(), "wire-record");
        assert_eq!(records[0].trimmed(), "wire-record");
        assert_eq!(input.buffered_bytes(), 0);
    }

    #[test]
    fn one_read_can_contain_multiple_records_and_a_partial_tail() {
        let mut input = InputAccumulator::default();
        let records = input.push(b"first\nsecond\nthi").unwrap();
        assert_eq!(
            records.iter().map(InputRecord::trimmed).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(input.buffered_bytes(), 3);
        let records = input.push(b"rd\n").unwrap();
        assert_eq!(records[0].trimmed(), "third");
    }

    #[test]
    fn routing_text_trims_crlf_and_unicode_whitespace_without_changing_original() {
        let mut input = InputAccumulator::default();
        let records = input
            .push("\u{2003}identify|id|token\r\n".as_bytes())
            .unwrap();
        assert_eq!(records[0].original(), "\u{2003}identify|id|token\r");
        assert_eq!(records[0].trimmed(), "identify|id|token");
    }

    #[test]
    fn empty_and_invalid_utf8_records_are_ignored() {
        let mut input = InputAccumulator::default();
        let records = input.push(b"\n \t\r\ninvalid-\xff\nvalid\n").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trimmed(), "valid");
    }

    #[test]
    fn exact_limit_is_accepted_before_complete_records_are_drained() {
        let mut input = InputAccumulator::default();
        let mut bytes = vec![b'x'; DEFAULT_MAX_INPUT_BYTES - 1];
        bytes.push(b'\n');
        let records = input.push(&bytes).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].original().len(), DEFAULT_MAX_INPUT_BYTES - 1);
        assert_eq!(input.buffered_bytes(), 0);
    }

    #[test]
    fn limit_plus_one_is_rejected() {
        let mut input = InputAccumulator::default();
        let bytes = vec![b'x'; DEFAULT_MAX_INPUT_BYTES + 1];
        assert_eq!(
            input.push(&bytes),
            Err(InputError::TooLarge {
                limit: DEFAULT_MAX_INPUT_BYTES
            })
        );
        assert_eq!(input.buffered_bytes(), DEFAULT_MAX_INPUT_BYTES + 1);
    }

    #[test]
    fn overflow_is_checked_before_a_complete_line_is_processed() {
        let mut input = InputAccumulator::new(5);
        assert!(input.push(b"1234").unwrap().is_empty());
        assert_eq!(input.push(b"\n5"), Err(InputError::TooLarge { limit: 5 }));
        assert_eq!(input.buffered_bytes(), 6);
    }
}
