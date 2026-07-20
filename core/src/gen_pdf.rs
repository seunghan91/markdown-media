//! Markdown → PDF generation module.
//!
//! Converts Markdown text to A4 PDF using `printpdf`.
//! Feature-gated behind `pdf-out` (see `core/Cargo.toml`).

use std::io;

#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
}

#[cfg(feature = "pdf-out")]
pub fn markdown_to_pdf(markdown: &str) -> io::Result<PdfOutput> {
    use printpdf::*;
    use std::io::BufWriter;

    let (doc, page_idx, layer_idx) = PdfDocument::new(
        "MDM Generated Document",
        Mm(210.0), // A4
        Mm(297.0),
        "Layer 1",
    );

    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let current_layer = doc.get_page(page_idx).get_layer(layer_idx);
    let mut y = Mm(280.0); // Start near top
    let line_height = Mm(5.0);
    let margin = Mm(15.0);

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            y = y - line_height;
            continue;
        }
        if y < margin {
            break; // Stop at bottom margin
        }

        let (text, font_size) = if trimmed.starts_with("# ") {
            (&trimmed[2..], 18.0)
        } else if trimmed.starts_with("## ") {
            (&trimmed[3..], 14.0)
        } else if trimmed.starts_with("### ") {
            (&trimmed[4..], 12.0)
        } else {
            (trimmed, 10.0)
        };

        current_layer.use_text(text, font_size, margin, y, &font);
        y = y - if font_size > 12.0 { line_height * 3.0 } else { line_height * 2.0 };
    }

    let mut buf = Vec::new();
    {
        let mut writer = BufWriter::new(&mut buf);
        doc.save(&mut writer)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    }

    Ok(PdfOutput { bytes: buf })
}

#[cfg(not(feature = "pdf-out"))]
pub fn markdown_to_pdf(_markdown: &str) -> io::Result<PdfOutput> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "PDF output disabled. Build with `--features pdf-out`.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "pdf-out")]
    fn test_basic_pdf() {
        let md = "# Title\n\nHello world\n\n## Section\n\nContent here.";
        let result = markdown_to_pdf(md).expect("generate");
        assert!(result.bytes.len() > 100);
        // Check PDF magic
        assert_eq!(&result.bytes[..5], b"%PDF-");
    }
}
