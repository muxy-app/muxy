//! Takes Sixel and iTerm2 images out of program output before libghostty
//! sees it. libghostty implements neither protocol, and it never reports
//! their sequences to the embedder, so they are found here instead.
//!
//! Everything else passes through unchanged and in order. A sequence is
//! held back only while its first bytes could still start an image.

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;
/// Sixel parameters are a few short numbers; anything longer is not Sixel.
const MAX_PARAMS: usize = 32;
/// Encoded images larger than this are skipped.
const MAX_PAYLOAD: usize = 4 * crate::MAX_GRAPHICS_BYTES;
/// A held buffer that grew past this is freed once its image is done.
const KEEP_CAPACITY: usize = 4096;

/// A piece of program output, in the order it arrived.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece<'a> {
    /// Bytes for libghostty.
    Text(&'a [u8]),
    /// A complete Sixel image: the parameters before `q`, then its data.
    Sixel { params: &'a [u8], data: &'a [u8] },
    /// A complete iTerm2 file: `key=value;…:base64`.
    Iterm(&'a [u8]),
}

#[derive(Debug, Default)]
pub(crate) struct Interceptor {
    state: State,
    /// Bytes held back while deciding, or the payload being collected.
    held: Vec<u8>,
    /// Where the Sixel data starts in `held`.
    params: usize,
    /// An iTerm2 multipart file collected so far. Any other output drops it.
    multipart: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    /// Saw `ESC` at the end of the last output.
    Escape,
    /// Inside `ESC P`, reading what could be Sixel parameters.
    DcsParams,
    /// Inside `ESC ]`, matching an iTerm2 file command.
    OscPrefix,
    /// Collecting a payload until its terminator.
    Body { kind: Kind, escape: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Sixel,
    File,
    MultipartFile,
    FilePart,
    FileEnd,
    /// A payload over the size limit, dropped up to its terminator.
    Skip {
        osc: bool,
    },
}

const COMMANDS: [(&[u8], Kind); 4] = [
    (b"1337;File=", Kind::File),
    (b"1337;MultipartFile=", Kind::MultipartFile),
    (b"1337;FilePart=", Kind::FilePart),
    (b"1337;FileEnd", Kind::FileEnd),
];

impl Interceptor {
    /// Splits `input` into pieces, holding back an unfinished image for the
    /// next call.
    pub(crate) fn feed(&mut self, input: &[u8], mut out: impl FnMut(Piece<'_>)) {
        let mut pending = 0;
        let mut i = 0;
        while i < input.len() {
            if self.state == State::Ground {
                (i, pending) = self.ground(input, i, pending, &mut out);
                continue;
            }
            i = match self.state {
                State::Ground => i,
                State::Escape => self.escape(input[i], i, &mut out),
                State::DcsParams => self.dcs_params(input[i], i, &mut out),
                State::OscPrefix => self.osc_prefix(input[i], i, &mut out),
                State::Body { kind, escape } => self.body(input, i, kind, escape, &mut out),
            };
            pending = i;
        }
        if self.state == State::Ground {
            self.emit(&mut out, &input[pending..i]);
        }
    }

    /// Passes text through up to the next `ESC P` or `ESC ]`. Returns the
    /// next byte to read and the start of the text not yet handed on.
    fn ground(
        &mut self,
        input: &[u8],
        i: usize,
        pending: usize,
        out: &mut impl FnMut(Piece<'_>),
    ) -> (usize, usize) {
        let Some(offset) = input[i..].iter().position(|&b| b == ESC) else {
            return (input.len(), pending);
        };
        let at = i + offset;
        match input.get(at + 1) {
            Some(&next @ (b'P' | b']')) => {
                self.emit(out, &input[pending..at]);
                self.hold(&[ESC, next]);
                self.state = introducer(next);
                (at + 2, at + 2)
            }
            Some(_) => (at + 1, pending),
            None => {
                self.emit(out, &input[pending..at]);
                self.hold(&[ESC]);
                self.state = State::Escape;
                (input.len(), input.len())
            }
        }
    }

    fn escape(&mut self, byte: u8, i: usize, out: &mut impl FnMut(Piece<'_>)) -> usize {
        if matches!(byte, b'P' | b']') {
            self.held.push(byte);
            self.state = introducer(byte);
            i + 1
        } else {
            self.release(out, i)
        }
    }

    fn dcs_params(&mut self, byte: u8, i: usize, out: &mut impl FnMut(Piece<'_>)) -> usize {
        match byte {
            b'0'..=b'9' | b';' if self.held.len() < 2 + MAX_PARAMS => {
                self.held.push(byte);
                i + 1
            }
            b'q' => {
                self.held.drain(..2);
                self.params = self.held.len();
                self.state = State::Body {
                    kind: Kind::Sixel,
                    escape: false,
                };
                i + 1
            }
            _ => self.release(out, i),
        }
    }

    fn osc_prefix(&mut self, byte: u8, i: usize, out: &mut impl FnMut(Piece<'_>)) -> usize {
        let len = self.held.len() - 2;
        let mut prefix = false;
        let mut matched = None;
        for (command, kind) in COMMANDS {
            if command.get(len) == Some(&byte) && command[..len] == self.held[2..] {
                prefix = true;
                if command.len() == len + 1 {
                    matched = Some(kind);
                }
            }
        }
        if !prefix {
            return self.release(out, i);
        }
        if let Some(kind) = matched {
            self.held.clear();
            self.state = State::Body {
                kind,
                escape: false,
            };
        } else {
            self.held.push(byte);
        }
        i + 1
    }

    /// Collects a payload until its terminator. Returns the next byte to read.
    fn body(
        &mut self,
        input: &[u8],
        i: usize,
        kind: Kind,
        escape: bool,
        out: &mut impl FnMut(Piece<'_>),
    ) -> usize {
        let byte = input[i];
        if escape {
            if byte == b'\\' {
                self.finish(kind, out);
            } else {
                self.reset();
                self.hold(&[ESC]);
                self.state = State::Escape;
                return i;
            }
            return i + 1;
        }
        let osc = !matches!(kind, Kind::Sixel | Kind::Skip { osc: false });
        match byte {
            ESC => {
                self.state = State::Body { kind, escape: true };
                i + 1
            }
            BEL if osc => {
                self.finish(kind, out);
                i + 1
            }
            // libghostty sees the cancel, as it would have mid-sequence.
            CAN | SUB => {
                self.reset();
                i
            }
            // Sixel ignores BEL, so it is collected with the data.
            _ => {
                let end = input[i + 1..]
                    .iter()
                    .position(|&b| matches!(b, ESC | BEL | CAN | SUB))
                    .map_or(input.len(), |offset| i + 1 + offset);
                self.collect(kind, &input[i..end]);
                end
            }
        }
    }

    fn hold(&mut self, bytes: &[u8]) {
        self.clear_held();
        self.held.extend_from_slice(bytes);
    }

    /// Hands the held bytes to libghostty after all, and returns to plain text
    /// at `i` without consuming it.
    fn release(&mut self, out: &mut impl FnMut(Piece<'_>), i: usize) -> usize {
        let held = std::mem::take(&mut self.held);
        self.emit(out, &held);
        self.held = held;
        self.reset();
        i
    }

    fn reset(&mut self) {
        self.clear_held();
        self.state = State::Ground;
    }

    fn clear_held(&mut self) {
        if self.held.capacity() > KEEP_CAPACITY {
            self.held = Vec::new();
        } else {
            self.held.clear();
        }
    }

    fn emit(&mut self, out: &mut impl FnMut(Piece<'_>), text: &[u8]) {
        if !text.is_empty() {
            self.multipart = None;
            out(Piece::Text(text));
        }
    }

    fn collect(&mut self, kind: Kind, bytes: &[u8]) {
        if matches!(kind, Kind::Skip { .. }) {
            return;
        }
        if self.held.len() + bytes.len() > MAX_PAYLOAD {
            self.clear_held();
            self.state = State::Body {
                kind: Kind::Skip {
                    osc: kind != Kind::Sixel,
                },
                escape: false,
            };
            if kind != Kind::Sixel {
                self.multipart = None;
            }
            return;
        }
        self.held.extend_from_slice(bytes);
    }

    fn finish(&mut self, kind: Kind, out: &mut impl FnMut(Piece<'_>)) {
        match kind {
            Kind::Sixel => {
                let (params, data) = self.held.split_at(self.params);
                out(Piece::Sixel { params, data });
            }
            Kind::File => out(Piece::Iterm(&self.held)),
            Kind::MultipartFile => {
                let mut file = std::mem::take(&mut self.held);
                file.push(b':');
                self.multipart = Some(file);
            }
            Kind::FilePart => {
                if let Some(file) = &mut self.multipart {
                    if file.len() + self.held.len() > MAX_PAYLOAD {
                        self.multipart = None;
                    } else {
                        file.extend_from_slice(&self.held);
                    }
                }
            }
            Kind::FileEnd => {
                if let Some(file) = self.multipart.take() {
                    out(Piece::Iterm(&file));
                }
            }
            Kind::Skip { .. } => {}
        }
        self.reset();
    }
}

fn introducer(byte: u8) -> State {
    if byte == b'P' {
        State::DcsParams
    } else {
        State::OscPrefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Owned {
        Text(Vec<u8>),
        Sixel(Vec<u8>, Vec<u8>),
        Iterm(Vec<u8>),
    }

    /// Feeds `input` in pieces of `size` bytes and merges adjacent text.
    fn run(input: &[u8], size: usize) -> Vec<Owned> {
        let mut interceptor = Interceptor::default();
        let mut pieces: Vec<Owned> = Vec::new();
        for chunk in input.chunks(size) {
            interceptor.feed(chunk, |piece| match piece {
                Piece::Text(text) => {
                    if let Some(Owned::Text(last)) = pieces.last_mut() {
                        last.extend_from_slice(text);
                    } else {
                        pieces.push(Owned::Text(text.to_vec()));
                    }
                }
                Piece::Sixel { params, data } => {
                    pieces.push(Owned::Sixel(params.to_vec(), data.to_vec()));
                }
                Piece::Iterm(file) => pieces.push(Owned::Iterm(file.to_vec())),
            });
        }
        pieces
    }

    fn every_split(input: &[u8], expected: &[Owned]) {
        for size in 1..=input.len() {
            assert_eq!(run(input, size), expected, "chunk size {size}");
        }
    }

    #[test]
    fn takes_sixel_out_and_keeps_text_around_it() {
        every_split(
            b"a\x1b[1mb\x1bP0;1;0q\"1;1;2;6#0~~\x1b\\c",
            &[
                Owned::Text(b"a\x1b[1mb".to_vec()),
                Owned::Sixel(b"0;1;0".to_vec(), b"\"1;1;2;6#0~~".to_vec()),
                Owned::Text(b"c".to_vec()),
            ],
        );
    }

    #[test]
    fn bel_inside_sixel_data_does_not_end_it() {
        every_split(
            b"\x1bPq#0~\x07~\x1b\\",
            &[Owned::Sixel(Vec::new(), b"#0~\x07~".to_vec())],
        );
    }

    #[test]
    fn leaves_other_dcs_and_osc_to_libghostty() {
        let input = b"\x1bP$qm\x1b\\\x1bP+q544e\x1b\\\x1b]0;title\x07\x1b]1337;SetUserVar=a=Yg==\x07\x1b]133;A\x1b\\";
        every_split(input, &[Owned::Text(input.to_vec())]);
    }

    #[test]
    fn takes_iterm_files_out_with_either_terminator() {
        every_split(
            b"x\x1b]1337;File=inline=1:AAAA\x07y\x1b]1337;File=inline=1;width=2:BBBB\x1b\\",
            &[
                Owned::Text(b"x".to_vec()),
                Owned::Iterm(b"inline=1:AAAA".to_vec()),
                Owned::Text(b"y".to_vec()),
                Owned::Iterm(b"inline=1;width=2:BBBB".to_vec()),
            ],
        );
    }

    #[test]
    fn joins_multipart_iterm_files() {
        every_split(
            b"\x1b]1337;MultipartFile=inline=1\x07\x1b]1337;FilePart=AA\x07\x1b]1337;FilePart=BB\x07\x1b]1337;FileEnd\x07z",
            &[
                Owned::Iterm(b"inline=1:AABB".to_vec()),
                Owned::Text(b"z".to_vec()),
            ],
        );
    }

    #[test]
    fn cancel_and_new_escape_drop_the_image() {
        every_split(
            b"\x1bPq~~\x18a\x1b]1337;File=inline=1:AA\x1b[2Jb",
            &[Owned::Text(b"\x18a\x1b[2Jb".to_vec())],
        );
    }

    #[test]
    fn frees_a_large_payload_once_the_image_is_done() {
        let mut interceptor = Interceptor::default();
        let mut input = b"\x1bPq".to_vec();
        input.resize(input.len() + 1024 * 1024, b'~');
        input.extend_from_slice(b"\x1b\\");
        let mut images = 0;
        interceptor.feed(&input, |piece| {
            if matches!(piece, Piece::Sixel { .. }) {
                images += 1;
            }
        });
        assert_eq!(images, 1);
        assert!(interceptor.held.capacity() <= KEEP_CAPACITY);
    }

    #[test]
    fn other_output_drops_an_unfinished_multipart_file() {
        every_split(
            b"\x1b]1337;MultipartFile=inline=1\x07\x1b]1337;FilePart=AA\x07x\x1b]1337;FileEnd\x07",
            &[Owned::Text(b"x".to_vec())],
        );
    }

    #[test]
    fn oversized_payloads_are_skipped_to_their_end() {
        let mut input = b"\x1bPq".to_vec();
        input.resize(input.len() + MAX_PAYLOAD + 1, b'~');
        input.extend_from_slice(b"\x1b\\after");
        assert_eq!(run(&input, 64 * 1024), [Owned::Text(b"after".to_vec())]);
    }
}
