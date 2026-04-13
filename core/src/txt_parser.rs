//! Plain text pass-through parser.
//!
//! Detects encoding (UTF-8 / EUC-KR via `encoding_rs`), normalises line
//! endings, and trims trailing whitespace. The output is essentially the
//! original text with minimal cleanup.

use std::io::{self, Read};
use std::path::Path;

/// Plain text parser.
pub struct TxtParser {
    content: String,
}

impl TxtParser {
    /// Open a text file from disk with encoding detection.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        std::fs::File::open(path.as_ref())?.read_to_end(&mut data)?;
        let content = decode_text(&data);
        Ok(Self { content })
    }

    /// Create a parser from raw bytes.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        let content = decode_text(&data);
        Ok(Self { content })
    }

    /// Return the cleaned Markdown (plain text pass-through).
    pub fn to_markdown(&self) -> String {
        normalise(&self.content)
    }

    /// Convenience: render to MDX with front-matter.
    pub fn to_mdx(&self, source_name: &str) -> String {
        let md = self.to_markdown();
        let line_count = md.lines().count();
        format!(
            "---\nformat: txt\nsource: \"{}\"\nlines: {}\n---\n\n{}",
            source_name.replace('"', "\\\""),
            line_count,
            md,
        )
    }
}

// ---------------------------------------------------------------------------
// Encoding detection + normalisation
// ---------------------------------------------------------------------------

/// Decode raw bytes to a UTF-8 string.
///
/// Strategy:
/// 1. Strip UTF-8 BOM if present.
/// 2. Try UTF-8.
/// 3. Fallback to EUC-KR (common in Korean `.txt` files).
fn decode_text(data: &[u8]) -> String {
    // Strip UTF-8 BOM.
    let data = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        &data[3..]
    } else {
        data
    };

    // Try UTF-8.
    if let Ok(s) = std::str::from_utf8(data) {
        return s.to_string();
    }

    // Fallback: EUC-KR.
    let (decoded, _, _) = encoding_rs::EUC_KR.decode(data);
    decoded.to_string()
}

/// Normalise line endings, trim trailing whitespace per line, collapse
/// excessive blank lines (more than 2 consecutive).
fn normalise(text: &str) -> String {
    // Normalise CRLF -> LF.
    let s = text.replace("\r\n", "\n").replace('\r', "\n");

    // Trim trailing whitespace per line.
    let lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();

    // Collapse runs of 3+ blank lines into 2.
    let mut result = String::with_capacity(s.len());
    let mut blank_count = 0u32;

    for line in &lines {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }

    // Trim leading/trailing whitespace of the whole document.
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalise_crlf() {
        let input = "line1\r\nline2\r\n";
        let out = normalise(input);
        assert!(!out.contains('\r'));
        assert!(out.contains("line1\nline2"));
    }

    #[test]
    fn test_normalise_collapse_blanks() {
        let input = "a\n\n\n\n\nb";
        let out = normalise(input);
        // Should have at most 2 blank lines between a and b.
        assert_eq!(out, "a\n\n\nb");
    }

    #[test]
    fn test_decode_utf8() {
        let data = "hello world".as_bytes();
        assert_eq!(decode_text(data), "hello world");
    }

    #[test]
    fn test_decode_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"text");
        assert_eq!(decode_text(&data), "text");
    }

    #[test]
    fn test_trailing_whitespace_trimmed() {
        let input = "line1   \nline2\t\n";
        let out = normalise(input);
        for line in out.lines() {
            assert_eq!(line, line.trim_end());
        }
    }
}
