use std::io::{self, Read};

pub(super) const READY: &[u8] = b"MUXY-STDIO/";
pub(super) const VERSION: &[u8] = b"1";
const MAX_NOISE: usize = 64 * 1024;
const MAX_VERSION: usize = 16;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum NotReady {
    Ended(Vec<u8>),
    Noisy(Vec<u8>),
    Version(Vec<u8>),
}

pub(super) fn ready<R: Read>(mut reader: R) -> Result<io::Chain<io::Cursor<Vec<u8>>, R>, NotReady> {
    let mut line = ReadyLine::default();
    let mut chunk = [0; 4096];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Err(line.ended()),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(line.ended()),
        };
        if let Some(outcome) = line.push(&chunk[..read]) {
            return outcome.map(|rest| io::Cursor::new(rest).chain(reader));
        }
    }
}

#[derive(Default)]
struct ReadyLine {
    seen: Vec<u8>,
    start: Option<usize>,
    searched: usize,
}

impl ReadyLine {
    fn push(&mut self, bytes: &[u8]) -> Option<Result<Vec<u8>, NotReady>> {
        self.seen.extend_from_slice(bytes);
        let Some(start) = self.start.or_else(|| self.find()) else {
            return (self.seen.len() >= MAX_NOISE + READY.len())
                .then(|| Err(NotReady::Noisy(std::mem::take(&mut self.seen))));
        };
        if start > MAX_NOISE {
            return Some(Err(NotReady::Noisy(self.seen[..start].to_vec())));
        }
        let line = &self.seen[start + READY.len()..];
        match line.iter().position(|&byte| byte == b'\n') {
            Some(end) if &line[..end] == VERSION => {
                Some(Ok(self.seen.split_off(start + READY.len() + end + 1)))
            }
            Some(end) => Some(Err(NotReady::Version(line[..end].to_vec()))),
            None if line.len() > MAX_VERSION => {
                Some(Err(NotReady::Version(line[..MAX_VERSION].to_vec())))
            }
            None => None,
        }
    }

    fn find(&mut self) -> Option<usize> {
        self.start = self.seen[self.searched..]
            .windows(READY.len())
            .position(|window| window == READY)
            .map(|offset| self.searched + offset);
        self.searched = self.seen.len().saturating_sub(READY.len() - 1);
        self.start
    }

    fn ended(self) -> NotReady {
        NotReady::Ended(self.seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Debug)]
    struct Reads<'a>(VecDeque<Result<&'a [u8], io::ErrorKind>>);

    impl Read for Reads<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            match self.0.pop_front() {
                Some(Ok(bytes)) => {
                    let length = bytes.len().min(buffer.len());
                    buffer[..length].copy_from_slice(&bytes[..length]);
                    if length < bytes.len() {
                        self.0.push_front(Ok(&bytes[length..]));
                    }
                    Ok(length)
                }
                Some(Err(kind)) => Err(kind.into()),
                None => Ok(0),
            }
        }
    }

    fn scan(chunks: &[&[u8]]) -> Result<Vec<u8>, NotReady> {
        let mut line = ReadyLine::default();
        for chunk in chunks {
            if let Some(outcome) = line.push(chunk) {
                return outcome;
            }
        }
        Err(line.ended())
    }

    #[test]
    fn the_ready_line_is_found_after_any_noise_and_keeps_later_bytes() {
        assert_eq!(scan(&[b"MUXY-STDIO/1\n"]), Ok(vec![]));
        assert_eq!(
            scan(&[b"Welcome!\nlast login\n", b"MUXY-STDIO/1\nhello"]),
            Ok(b"hello".to_vec())
        );
        assert_eq!(scan(&[b"no newline", b"MUXY-STDIO/1\n"]), Ok(vec![]));
        assert_eq!(
            scan(&[b"noise MUXY-", b"ST", b"DIO/", b"1", b"\nrest"]),
            Ok(b"rest".to_vec())
        );
    }

    #[test]
    fn fragmented_readiness_preserves_buffered_and_unread_protocol_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let preamble = b"Welcome MUXY-MUXY-STDIO/1\n";
        let payload = b"\0\xffprotocol bytes\n";
        for split in 1..preamble.len() {
            let next = [&preamble[split..], &payload[..5]].concat();
            let reader = Reads(VecDeque::from([
                Err(io::ErrorKind::Interrupted),
                Ok(&preamble[..split]),
                Err(io::ErrorKind::Interrupted),
                Ok(next.as_slice()),
                Ok(&payload[5..]),
            ]));
            let mut reader =
                ready(reader).map_err(|error| io::Error::other(format!("{error:?}")))?;
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes)?;
            assert_eq!(bytes, payload, "preamble split at {split}");
        }
        Ok(())
    }

    #[test]
    fn noise_limits_reject_excess_output_and_keep_the_failure_detail() {
        let noise = vec![b'x'; MAX_NOISE];
        assert_eq!(
            scan(&[&noise, b"MUX", b"Y-STDIO/1\nrest"]),
            Ok(b"rest".to_vec())
        );
        let excessive = vec![b'x'; MAX_NOISE + 1];
        assert_eq!(
            scan(&[&excessive, b"MUXY-STDIO/1\nrest"]),
            Err(NotReady::Noisy(excessive))
        );
        let unmarked = vec![b'x'; MAX_NOISE + READY.len()];
        assert_eq!(scan(&[&unmarked]), Err(NotReady::Noisy(unmarked)));
    }

    #[test]
    fn unsupported_and_unterminated_versions_keep_the_reported_bytes() {
        let long_version = vec![b'2'; MAX_VERSION + 1];
        for version in [&b""[..], b"2", b"1\r", long_version.as_slice()] {
            let line = [READY, version, b"\nrest"].concat();
            assert_eq!(scan(&[&line]), Err(NotReady::Version(version.to_vec())));
        }
        let pending = [READY, &long_version[..MAX_VERSION]].concat();
        assert_eq!(scan(&[&pending]), Err(NotReady::Ended(pending)));
        let too_long = [READY, &long_version].concat();
        assert_eq!(
            scan(&[&too_long]),
            Err(NotReady::Version(long_version[..MAX_VERSION].to_vec()))
        );
    }

    #[test]
    fn eof_and_read_failures_keep_all_output_before_readiness() {
        for printed in [&b""[..], b"Welcome\n", b"Welcome\nMUXY-STDIO/1"] {
            assert!(matches!(
                ready(printed),
                Err(NotReady::Ended(bytes)) if bytes == printed
            ));
            let reader = Reads(VecDeque::from([
                Ok(printed),
                Err(io::ErrorKind::ConnectionReset),
            ]));
            assert!(matches!(
                ready(reader),
                Err(NotReady::Ended(bytes)) if bytes == printed
            ));
        }
    }
}
