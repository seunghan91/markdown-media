//! WASM bindings for the MDM core engine.
//!
//! Exposes document parsers (HWP, HWPX, PDF, DOCX) to JavaScript via
//! `wasm-bindgen`. All functions accept in-memory byte slices so no
//! file-system access is required.
//!
//! # Building
//!
//! ```sh
//! wasm-pack build core/ --target web --features wasm
//! ```

#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Panic hook initialisation
// ---------------------------------------------------------------------------

/// Initialise `console_error_panic_hook` so Rust panics produce readable
/// stack traces in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the crate version (from `Cargo.toml`).
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Detect the document format from the filename extension and/or magic bytes.
///
/// Returns one of `"hwp"`, `"hwpx"`, `"pdf"`, `"docx"`, or `"unknown"`.
#[wasm_bindgen]
pub fn detect_format(data: &[u8], filename: &str) -> String {
    detect_format_inner(data, filename).to_string()
}

/// Convert a document to Markdown.
///
/// The format is auto-detected from the filename extension and magic bytes.
/// On success the Markdown string is returned; on error a `JsValue`
/// containing the error message is thrown.
#[wasm_bindgen]
pub fn convert_to_markdown(data: &[u8], filename: &str) -> Result<String, JsValue> {
    let format = detect_format_inner(data, filename);
    match format {
        Format::Hwp => convert_hwp_to_markdown(data),
        Format::Hwpx => convert_hwpx_to_markdown(data),
        #[cfg(feature = "pdf")]
        Format::Pdf => convert_pdf_to_markdown(data),
        #[cfg(not(feature = "pdf"))]
        Format::Pdf => Err(JsValue::from_str("PDF support not available in this WASM build")),
        Format::Docx => convert_docx_to_markdown(data),
        Format::Unknown => Err(JsValue::from_str(
            "Unknown document format. Supported: .hwp, .hwpx, .pdf, .docx",
        )),
    }
}

/// Convert a document to a JSON string containing metadata and content.
///
/// The returned JSON has the shape:
/// ```json
/// {
///   "format": "hwp" | "hwpx" | "pdf" | "docx",
///   "version": "...",
///   "markdown": "...",
///   "metadata": { ... }
/// }
/// ```
#[wasm_bindgen]
pub fn convert_to_json(data: &[u8], filename: &str) -> Result<String, JsValue> {
    let format = detect_format_inner(data, filename);
    match format {
        Format::Hwp => convert_hwp_to_json(data),
        Format::Hwpx => convert_hwpx_to_json(data),
        #[cfg(feature = "pdf")]
        Format::Pdf => convert_pdf_to_json(data),
        #[cfg(not(feature = "pdf"))]
        Format::Pdf => Err(JsValue::from_str("PDF support not available in this WASM build")),
        Format::Docx => convert_docx_to_json(data),
        Format::Unknown => Err(JsValue::from_str(
            "Unknown document format. Supported: .hwp, .hwpx, .pdf, .docx",
        )),
    }
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Hwp,
    Hwpx,
    Pdf,
    Docx,
    Unknown,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Hwp => write!(f, "hwp"),
            Format::Hwpx => write!(f, "hwpx"),
            Format::Pdf => write!(f, "pdf"),
            Format::Docx => write!(f, "docx"),
            Format::Unknown => write!(f, "unknown"),
        }
    }
}

/// Magic-byte constants.
const OLE_MAGIC: [u8; 4] = [0xD0, 0xCF, 0x11, 0xE0];
const ZIP_MAGIC: [u8; 2] = [0x50, 0x4B];
const PDF_MAGIC: [u8; 4] = [0x25, 0x50, 0x44, 0x46]; // %PDF

fn detect_format_inner(data: &[u8], filename: &str) -> Format {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    // Extension-first for unambiguous cases.
    match ext.as_str() {
        "hwp" => return Format::Hwp,
        "hwpx" => return Format::Hwpx,
        "pdf" => return Format::Pdf,
        "docx" => return Format::Docx,
        _ => {}
    }

    // Fall back to magic bytes.
    if data.len() >= 4 && data[..4] == OLE_MAGIC {
        return Format::Hwp;
    }
    if data.len() >= 4 && data[..4] == PDF_MAGIC {
        return Format::Pdf;
    }
    if data.len() >= 2 && data[..2] == ZIP_MAGIC {
        // Distinguish HWPX from DOCX by peeking at ZIP entry names.
        return detect_zip_format(data);
    }

    Format::Unknown
}

/// Peek into ZIP entry names to distinguish HWPX from DOCX.
fn detect_zip_format(data: &[u8]) -> Format {
    use std::io::Cursor;

    let cursor = Cursor::new(data);
    if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index_raw(i) {
                let name = entry.name();
                if name.starts_with("Contents/") {
                    return Format::Hwpx;
                }
                if name.starts_with("word/") {
                    return Format::Docx;
                }
            }
        }
    }
    Format::Unknown
}

// ---------------------------------------------------------------------------
// Per-format conversion helpers
// ---------------------------------------------------------------------------

fn convert_hwp_to_markdown(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::hwp::HwpParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("HWP parse error: {}", e)))?;
    parser
        .extract_text()
        .map_err(|e| JsValue::from_str(&format!("HWP extraction error: {}", e)))
}

fn convert_hwpx_to_markdown(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::hwpx::HwpxParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("HWPX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| JsValue::from_str(&format!("HWPX extraction error: {}", e)))?;
    Ok(doc.sections.join("\n\n"))
}

#[cfg(feature = "pdf")]
fn convert_pdf_to_markdown(data: &[u8]) -> Result<String, JsValue> {
    let parser = crate::pdf::PdfParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("PDF parse error: {}", e)))?;
    let doc = parser
        .parse_from_memory()
        .map_err(|e| JsValue::from_str(&format!("PDF extraction error: {}", e)))?;
    Ok(doc.to_markdown_with_layout())
}

fn convert_docx_to_markdown(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::docx::DocxParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("DOCX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| JsValue::from_str(&format!("DOCX extraction error: {}", e)))?;
    Ok(doc.to_markdown())
}

// ---------------------------------------------------------------------------
// JSON conversion helpers
// ---------------------------------------------------------------------------

fn convert_hwp_to_json(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::hwp::HwpParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("HWP parse error: {}", e)))?;

    let markdown = parser
        .extract_text()
        .map_err(|e| JsValue::from_str(&format!("HWP extraction error: {}", e)))?;
    let metadata = parser
        .extract_metadata()
        .map_err(|e| JsValue::from_str(&format!("HWP metadata error: {}", e)))?;

    let json = serde_json::json!({
        "format": "hwp",
        "version": metadata.version,
        "markdown": markdown,
        "metadata": {
            "version": metadata.version,
            "encrypted": metadata.encrypted,
            "compressed": metadata.compressed,
            "section_count": metadata.section_count,
            "title": metadata.title,
            "author": metadata.author,
        }
    });

    serde_json::to_string(&json)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

fn convert_hwpx_to_json(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::hwpx::HwpxParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("HWPX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| JsValue::from_str(&format!("HWPX extraction error: {}", e)))?;

    let json = serde_json::json!({
        "format": "hwpx",
        "version": doc.version,
        "markdown": doc.sections.join("\n\n"),
        "metadata": {
            "version": doc.version,
            "section_count": doc.sections.len(),
            "image_count": doc.images.len(),
            "table_count": doc.tables.len(),
        }
    });

    serde_json::to_string(&json)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

#[cfg(feature = "pdf")]
fn convert_pdf_to_json(data: &[u8]) -> Result<String, JsValue> {
    let parser = crate::pdf::PdfParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("PDF parse error: {}", e)))?;
    let doc = parser
        .parse_from_memory()
        .map_err(|e| JsValue::from_str(&format!("PDF extraction error: {}", e)))?;

    let json = serde_json::json!({
        "format": "pdf",
        "version": doc.version,
        "markdown": doc.to_markdown_with_layout(),
        "metadata": {
            "version": doc.version,
            "page_count": doc.page_count,
            "title": doc.metadata.title,
            "author": doc.metadata.author,
            "image_count": doc.images.len(),
            "font_count": doc.fonts.len(),
            "table_count": doc.tables.len(),
        }
    });

    serde_json::to_string(&json)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

fn convert_docx_to_json(data: &[u8]) -> Result<String, JsValue> {
    let mut parser = crate::docx::DocxParser::from_bytes(data.to_vec())
        .map_err(|e| JsValue::from_str(&format!("DOCX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| JsValue::from_str(&format!("DOCX extraction error: {}", e)))?;

    let json = serde_json::json!({
        "format": "docx",
        "markdown": doc.to_markdown(),
        "metadata": {
            "title": doc.metadata.title,
            "author": doc.metadata.author,
            "created": doc.metadata.created,
            "modified": doc.metadata.modified,
            "paragraph_count": doc.paragraphs.len(),
            "image_count": doc.images.len(),
            "table_count": doc.tables.len(),
        }
    });

    serde_json::to_string(&json)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}
