//! Markdown → PDF generation module.
//!
//! Thin wrapper that converts Markdown to PDF by delegating to the `print`
//! module (IR → printpdf), which tracks the current `printpdf` API. Kept as a
//! stable entry point for the CLI; feature-gated behind `pdf-out`, which pulls
//! in `print-pdf` (see `core/Cargo.toml`).

use std::io;

#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
}

#[cfg(feature = "pdf-out")]
pub fn markdown_to_pdf(markdown: &str) -> io::Result<PdfOutput> {
    let blocks = crate::print::markdown_to_ir(markdown);
    let options = crate::print::RenderOptions::default();
    let bytes = crate::print::render_ir_to_pdf(&blocks, &options)?;
    Ok(PdfOutput { bytes })
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
