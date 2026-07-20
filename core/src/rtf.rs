//! RTF (Rich Text Format) parser.
//!
//! Converts `.rtf` documents to Markdown via `rtf-parser`.
//! RTF is a legacy word-processor interchange format still used by Korean
//! government systems and older document archives.
//!
//! Feature-gated behind `rtf` (see `core/Cargo.toml`).

use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RtfDocument {
    pub text: String,
}

pub struct RtfParser {
    data: Vec<u8>,
}

impl RtfParser {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        Ok(Self { data })
    }

    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        Ok(Self { data })
    }

    pub fn parse(&self) -> io::Result<RtfDocument> {
        parse_rtf(&self.data)
    }
}

pub fn parse_rtf(data: &[u8]) -> io::Result<RtfDocument> {
    let s = String::from_utf8_lossy(data);
    let doc = rtf_parser::parse_rtf(s.into_owned());
    Ok(RtfDocument {
        text: doc.get_text(),
    })
}

impl RtfDocument {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        for line in self.text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !out.is_empty() && !out.ends_with("\n\n") {
                    out.push('\n');
                }
                continue;
            }
            out.push_str(trimmed);
            out.push('\n');
        }

        out.trim().to_string()
    }

    pub fn to_mdx(&self, source_name: &str) -> String {
        format!(
            "---\nformat: rtf\nsource: \"{}\"\n---\n\n{}",
            source_name.replace('"', "\\\""),
            self.to_markdown(),
        )
    }
}

pub fn looks_like_rtf(data: &[u8]) -> bool {
    if data.len() < 5 {
        return false;
    }
    let start = &data[..std::cmp::min(5, data.len())];
    start == b"{\\rtf"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_rtf_true() {
        assert!(looks_like_rtf(b"{\\rtf1\\ansi\\deff0 {\\fonttbl"));
    }

    #[test]
    fn test_looks_like_rtf_false() {
        assert!(!looks_like_rtf(b"not rtf"));
        assert!(!looks_like_rtf(b""));
    }

    #[test]
    fn test_parse_simple_rtf() {
        let rtf = b"{\\rtf1\\ansi Hello world\\par Goodbye.}";
        let doc = parse_rtf(rtf).expect("parse");
        assert!(doc.text.contains("Hello world"));
        assert!(doc.text.contains("Goodbye"));
    }
}
