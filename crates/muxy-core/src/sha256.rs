//! SHA-256 digests, computed in the process.

use std::io::{self, Write};

use ring::digest::{Context, SHA256};

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut context = Context::new(&SHA256);
    context.update(bytes);
    finish(context)
}

/// Passes everything written on to `inner` and hashes it on the way.
pub struct Writer<W> {
    inner: W,
    context: Context,
}

impl<W: std::fmt::Debug> std::fmt::Debug for Writer<W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<W: Write> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            context: Context::new(&SHA256),
        }
    }

    /// The writer, flushed, and the digest of everything it accepted.
    pub fn finish(mut self) -> io::Result<(W, [u8; 32])> {
        self.inner.flush()?;
        Ok((self.inner, finish(self.context)))
    }
}

impl<W: Write> Write for Writer<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.context.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn finish(context: Context) -> [u8; 32] {
    let mut digest = [0; 32];
    digest.copy_from_slice(context.finish().as_ref());
    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn matches_the_standard_vectors() {
        assert_eq!(
            hex::encode(&digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(hex::encode(&digest(b"abc")), ABC);
    }

    #[test]
    fn the_writer_hashes_exactly_what_it_passes_on() -> io::Result<()> {
        let mut writer = Writer::new(Vec::new());
        writer.write_all(b"a")?;
        writer.write_all(b"bc")?;
        let (written, digest) = writer.finish()?;
        assert_eq!(written, b"abc");
        assert_eq!(hex::encode(&digest), ABC);
        Ok(())
    }
}
