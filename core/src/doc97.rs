//! DOC (Word 97-2003) parser — minimal text extraction from OLE2/CFB.
//!
//! Legacy `.doc` files use the same OLE2 Compound File Binary container as
//! HWP 5.x and XLS. This module extracts the WordDocument stream and
//! pulls out plain text via the FIB (File Information Block).
//!
//! This is a best-effort text extractor — formatting, tables, images, and
//! complex layout are not preserved. For full-fidelity DOC conversion, use
//! LibreOffice in headless mode.

use std::io::{self, Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DocDocument {
    pub text: String,
}

pub struct DocParser {
    data: Vec<u8>,
}

impl DocParser {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    pub fn parse(&self) -> io::Result<DocDocument> {
        parse_doc(&self.data)
    }
}

pub fn parse_doc(data: &[u8]) -> io::Result<DocDocument> {
    let cursor = Cursor::new(data);
    let mut cf = cfb::CompoundFile::open(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let text = extract_text_from_cfb(&mut cf)?;
    Ok(DocDocument { text })
}

fn extract_text_from_cfb<R: Read + std::io::Seek>(cf: &mut cfb::CompoundFile<R>) -> io::Result<String> {
    // Try to read the WordDocument stream first
    if let Ok(mut stream) = cf.open_stream("WordDocument") {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;

        if buf.len() < 0x00A2 {
            return Ok(String::new());
        }

        // FIB: bytes 0x00A2-0x00A5 = ccpText (character count of main text)
        let ccp_text = u32::from_le_bytes([
            buf[0x00A2], buf[0x00A3], buf[0x00A4], buf[0x00A5],
        ]) as usize;

        if ccp_text == 0 || ccp_text > buf.len() / 2 {
            return extract_text_fallback(cf);
        }

        // The text in WordDocument stream is at variable offset.
        // For simplicity, try to extract printable UTF-16LE characters
        // from the tail of the WordDocument stream where body text lives.
        let tail_start = buf.len().saturating_sub(ccp_text * 2 + 512);
        let tail = &buf[tail_start..];

        let mut text = String::new();
        let mut i = 0;
        while i + 1 < tail.len() {
            let lo = tail[i] as u16;
            let hi = tail[i + 1] as u16;
            let cp = lo | (hi << 8);
            i += 2;

            if cp == 0x000D || cp == 0x0007 {
                text.push('\n');
            } else if cp == 0x0013 {
                // hyphens/special
            } else if (0x0020..=0xD7FF).contains(&cp) && cp != 0x000D {
                if let Some(c) = char::from_u32(cp as u32) {
                    text.push(c);
                }
            }
        }

        if !text.trim().is_empty() {
            return Ok(clean_text(&text));
        }
    }

    extract_text_fallback(cf)
}

fn extract_text_fallback<R: Read + std::io::Seek>(cf: &mut cfb::CompoundFile<R>) -> io::Result<String> {
    let paths: Vec<String> = cf.walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();

    let mut all_text = String::new();
    for path in &paths {
        if let Ok(mut stream) = cf.open_stream(path) {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf)?;
            let s = String::from_utf8_lossy(&buf);
            for ch in s.chars() {
                if ch.is_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation() || (ch as u32) >= 0xAC00 {
                    all_text.push(ch);
                }
            }
            all_text.push('\n');
        }
    }
    Ok(clean_text(&all_text))
}

fn clean_text(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.len() < 2 {
            continue;
        }
        // Skip lines that are mostly garbage (high ratio of non-Korean/non-ASCII)
        let alpha_count = trimmed.chars().filter(|c| c.is_alphanumeric() || (*c as u32) >= 0xAC00).count();
        if alpha_count * 2 >= trimmed.len() {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

impl DocDocument {
    pub fn to_markdown(&self) -> String {
        self.text.clone()
    }

    pub fn to_mdx(&self, source_name: &str) -> String {
        format!(
            "---\nformat: doc\nsource: \"{}\"\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.to_markdown(),
        )
    }
}

pub fn looks_like_doc(data: &[u8]) -> bool {
    const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    if data.len() < 8 || data[..8] != CFB_MAGIC {
        return false;
    }
    // Check for WordDocument stream
    let cursor = Cursor::new(data);
    if let Ok(cf) = cfb::CompoundFile::open(cursor) {
        return cf.walk().any(|e| {
            e.is_stream() && e.path().to_string_lossy().contains("WordDocument")
        });
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_doc_true() {
        let cfb_magic = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        // A minimal CFB with WordDocument stream would require more bytes,
        // but this at least checks magic bytes
        assert!(!looks_like_doc(&cfb_magic)); // No WordDocument inside
    }

    #[test]
    fn test_looks_like_doc_false() {
        assert!(!looks_like_doc(b"not a doc"));
        assert!(!looks_like_doc(b"PK\x03\x04"));
    }
}
