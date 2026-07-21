//! Markdown → DOCX generation module.
//!
//! Converts Markdown text to .docx (Office Open XML) using `docx-rs`.
//! Feature-gated behind `docx-out` (see `core/Cargo.toml`).

use std::io;

#[derive(Debug, Clone)]
pub struct DocxOutput {
    pub bytes: Vec<u8>,
}

#[cfg(feature = "docx-out")]
pub fn markdown_to_docx(markdown: &str) -> io::Result<DocxOutput> {
    use docx_rs::*;

    let mut document = Docx::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("# ") {
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(&trimmed[2..]).size(36).bold())
            );
        } else if trimmed.starts_with("## ") {
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(&trimmed[3..]).size(28).bold())
            );
        } else if trimmed.starts_with("### ") {
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(&trimmed[4..]).size(24).bold())
            );
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let text = &trimmed[2..];
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(&format!("  • {}", text)))
            );
        } else if trimmed.starts_with("> ") {
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(&trimmed[2..]).italic())
            );
        } else {
            document = document.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(trimmed))
            );
        }
    }

    let mut bytes = Vec::new();
    document
        .build()
        .pack(std::io::Cursor::new(&mut bytes))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    Ok(DocxOutput { bytes })
}

#[cfg(not(feature = "docx-out"))]
pub fn markdown_to_docx(_markdown: &str) -> io::Result<DocxOutput> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "DOCX output disabled. Build with `--features docx-out`.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "docx-out")]
    fn test_basic_docx() {
        let md = "# Title\n\nHello world\n\n- item 1\n- item 2";
        let result = markdown_to_docx(md).expect("generate");
        assert!(result.bytes.len() > 100);
        // Check ZIP magic
        assert_eq!(&result.bytes[..2], b"PK");
    }
}
