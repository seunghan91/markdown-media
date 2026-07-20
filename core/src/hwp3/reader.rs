//! HWP3 binary stream reader.
//!
//! Cursor over an in-memory byte buffer that reads little-endian
//! primitives sequentially. Returns `io::Error` (`UnexpectedEof`) on
//! insufficient data so callers can fall back to partial-parse mode.
//!
//! Ported from kkdoc (MIT): src/hwp3/reader.ts

use std::io;

pub struct Hwp3Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Hwp3Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn eof(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn ensure(&self, n: usize) -> io::Result<()> {
        if self.pos + n > self.buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "HWP3: insufficient data (need {}, have {})",
                    n,
                    self.buf.len().saturating_sub(self.pos)
                ),
            ));
        }
        Ok(())
    }

    pub fn skip(&mut self, n: usize) -> io::Result<()> {
        self.ensure(n)?;
        self.pos += n;
        Ok(())
    }

    pub fn read_u8(&mut self) -> io::Result<u8> {
        self.ensure(1)?;
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn read_u16(&mut self) -> io::Result<u16> {
        self.ensure(2)?;
        let v = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    pub fn read_u32(&mut self) -> io::Result<u32> {
        self.ensure(4)?;
        let v = u32::from_le_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn read_bytes(&mut self, n: usize) -> io::Result<&'a [u8]> {
        self.ensure(n)?;
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    /// Consumes and returns all remaining bytes, moving the cursor to the end.
    pub fn read_to_end(&mut self) -> &'a [u8] {
        let slice = &self.buf[self.pos..];
        self.pos = self.buf.len();
        slice
    }
}
