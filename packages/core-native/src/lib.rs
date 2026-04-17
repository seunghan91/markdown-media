mod types;

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;
use std::path::Path;
use types::*;

// ============ Unified Document API (parity with WASM / Python) ============

/// Return the mdm-core crate version.
#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Hwp,
    Hwpx,
    Pdf,
    Docx,
    Unknown,
}

impl Format {
    fn as_str(&self) -> &'static str {
        match self {
            Format::Hwp => "hwp",
            Format::Hwpx => "hwpx",
            Format::Pdf => "pdf",
            Format::Docx => "docx",
            Format::Unknown => "unknown",
        }
    }
}

const OLE_MAGIC: [u8; 4] = [0xD0, 0xCF, 0x11, 0xE0];
const ZIP_MAGIC: [u8; 2] = [0x50, 0x4B];
const PDF_MAGIC: [u8; 4] = [0x25, 0x50, 0x44, 0x46];

fn detect_format_inner(data: &[u8], filename: &str) -> Format {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "hwp" => return Format::Hwp,
        "hwpx" => return Format::Hwpx,
        "pdf" => return Format::Pdf,
        "docx" => return Format::Docx,
        _ => {}
    }

    if data.len() >= 4 && data[..4] == OLE_MAGIC {
        return Format::Hwp;
    }
    if data.len() >= 4 && data[..4] == PDF_MAGIC {
        return Format::Pdf;
    }
    if data.len() >= 2 && data[..2] == ZIP_MAGIC {
        return detect_zip_format(data);
    }
    Format::Unknown
}

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

/// Detect document format from filename extension and/or magic bytes.
/// Returns one of `"hwp"`, `"hwpx"`, `"pdf"`, `"docx"`, `"unknown"`.
#[napi]
pub fn detect_format(data: Buffer, filename: String) -> String {
    detect_format_inner(data.as_ref(), &filename)
        .as_str()
        .to_string()
}

fn hwp_to_markdown(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::hwp::HwpParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("HWP parse error: {}", e)))?;
    parser
        .extract_text()
        .map_err(|e| napi::Error::from_reason(format!("HWP extraction error: {}", e)))
}

fn hwpx_to_markdown(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::hwpx::HwpxParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("HWPX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("HWPX extraction error: {}", e)))?;
    Ok(doc.sections.join("\n\n"))
}

fn pdf_to_markdown(data: &[u8]) -> napi::Result<String> {
    let parser = mdm_core::pdf::PdfParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("PDF parse error: {}", e)))?;
    let doc = parser
        .parse_from_memory()
        .map_err(|e| napi::Error::from_reason(format!("PDF extraction error: {}", e)))?;
    Ok(doc.to_markdown_with_layout())
}

fn docx_to_markdown(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::docx::DocxParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("DOCX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("DOCX extraction error: {}", e)))?;
    Ok(doc.to_markdown())
}

fn convert_bytes_inner(data: &[u8], filename: &str) -> napi::Result<String> {
    match detect_format_inner(data, filename) {
        Format::Hwp => hwp_to_markdown(data),
        Format::Hwpx => hwpx_to_markdown(data),
        Format::Pdf => pdf_to_markdown(data),
        Format::Docx => docx_to_markdown(data),
        Format::Unknown => Err(napi::Error::from_reason(
            "Unknown document format. Supported: .hwp, .hwpx, .pdf, .docx",
        )),
    }
}

/// Convert a document (raw bytes) to Markdown. Format auto-detected from filename + magic bytes.
#[napi]
pub fn convert_bytes(data: Buffer, filename: String) -> napi::Result<String> {
    convert_bytes_inner(data.as_ref(), &filename)
}

/// Convert a document on disk to Markdown.
#[napi]
pub fn convert_file(path: String) -> napi::Result<String> {
    let data = std::fs::read(&path)
        .map_err(|e| napi::Error::from_reason(format!("Read error '{}': {}", path, e)))?;
    convert_bytes_inner(&data, &path)
}

fn hwp_to_json(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::hwp::HwpParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("HWP parse error: {}", e)))?;
    let markdown = parser
        .extract_text()
        .map_err(|e| napi::Error::from_reason(format!("HWP extraction error: {}", e)))?;
    let metadata = parser
        .extract_metadata()
        .map_err(|e| napi::Error::from_reason(format!("HWP metadata error: {}", e)))?;
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
        .map_err(|e| napi::Error::from_reason(format!("JSON error: {}", e)))
}

fn hwpx_to_json(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::hwpx::HwpxParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("HWPX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("HWPX extraction error: {}", e)))?;
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
        .map_err(|e| napi::Error::from_reason(format!("JSON error: {}", e)))
}

fn pdf_to_json(data: &[u8]) -> napi::Result<String> {
    let parser = mdm_core::pdf::PdfParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("PDF parse error: {}", e)))?;
    let doc = parser
        .parse_from_memory()
        .map_err(|e| napi::Error::from_reason(format!("PDF extraction error: {}", e)))?;
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
        .map_err(|e| napi::Error::from_reason(format!("JSON error: {}", e)))
}

fn docx_to_json(data: &[u8]) -> napi::Result<String> {
    let mut parser = mdm_core::docx::DocxParser::from_bytes(data.to_vec())
        .map_err(|e| napi::Error::from_reason(format!("DOCX parse error: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("DOCX extraction error: {}", e)))?;
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
        .map_err(|e| napi::Error::from_reason(format!("JSON error: {}", e)))
}

/// Convert a document (bytes) to a JSON string with format/version/markdown/metadata.
#[napi]
pub fn convert_to_json(data: Buffer, filename: String) -> napi::Result<String> {
    let bytes = data.as_ref();
    match detect_format_inner(bytes, &filename) {
        Format::Hwp => hwp_to_json(bytes),
        Format::Hwpx => hwpx_to_json(bytes),
        Format::Pdf => pdf_to_json(bytes),
        Format::Docx => docx_to_json(bytes),
        Format::Unknown => Err(napi::Error::from_reason(
            "Unknown document format. Supported: .hwp, .hwpx, .pdf, .docx",
        )),
    }
}

// ============ Legacy / low-level API (unchanged) ============

// ============ Annex Parser ============

/// Parse annex/form markers from Korean legal text
#[napi]
pub fn parse_annex_text(text: String) -> Vec<NapiAnnexInfo> {
    mdm_core::legal::AnnexParser::extract_from_text(&text)
        .into_iter()
        .map(NapiAnnexInfo::from)
        .collect()
}

/// Parse annexes from an HWP file path
#[napi]
pub fn parse_annex_hwp(path: String) -> napi::Result<Vec<NapiAnnexInfo>> {
    mdm_core::legal::AnnexParser::from_hwp_file(&path)
        .map(|v| v.into_iter().map(NapiAnnexInfo::from).collect())
        .map_err(|e| napi::Error::from_reason(e))
}

/// Parse annexes from an HWPX file path
#[napi]
pub fn parse_annex_hwpx(path: String) -> napi::Result<Vec<NapiAnnexInfo>> {
    mdm_core::legal::AnnexParser::from_hwpx_file(&path)
        .map(|v| v.into_iter().map(NapiAnnexInfo::from).collect())
        .map_err(|e| napi::Error::from_reason(e))
}

// ============ Date Parser ============

/// Parse Korean date expression with today as reference
#[napi]
pub fn parse_date(text: String) -> Option<NapiDateResult> {
    mdm_core::utils::date_parser::KoreanDateParser::today()
        .parse(&text)
        .map(NapiDateResult::from)
}

/// Parse Korean date expression with custom reference date (YYYYMMDD)
#[napi]
pub fn parse_date_with_reference(
    text: String,
    reference_date: String,
) -> napi::Result<Option<NapiDateResult>> {
    let ref_date = chrono::NaiveDate::parse_from_str(&reference_date, "%Y%m%d").map_err(|e| {
        napi::Error::from_reason(format!(
            "Invalid reference date '{}': {}",
            reference_date, e
        ))
    })?;
    Ok(mdm_core::utils::date_parser::KoreanDateParser::new(ref_date)
        .parse(&text)
        .map(NapiDateResult::from))
}

// ============ Chain Planner ============

/// Create a chain execution plan
#[napi]
pub fn create_chain_plan(chain_type: String, query: String) -> napi::Result<NapiChainPlan> {
    let ct =
        mdm_core::legal::ChainType::from_str(&chain_type).map_err(napi::Error::from_reason)?;
    let plan = mdm_core::legal::ChainPlan::from_query(ct, &query);
    Ok(NapiChainPlan::from(plan))
}

/// Aggregate chain step results into markdown
#[napi]
pub fn aggregate_chain_results(chain_type: String, results: Vec<String>) -> napi::Result<String> {
    let ct =
        mdm_core::legal::ChainType::from_str(&chain_type).map_err(napi::Error::from_reason)?;
    Ok(mdm_core::legal::ChainPlan::aggregate_results(&ct, &results))
}

// ============ HWP/HWPX Parser ============

/// Extract text and tables from HWP file
#[napi]
pub fn parse_hwp_file(path: String) -> napi::Result<NapiHwpResult> {
    let mut parser = mdm_core::hwp::HwpParser::open(&path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to open HWP: {}", e)))?;
    let text = parser
        .extract_text()
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract text: {}", e)))?;
    let tables = parser
        .extract_tables()
        .map_err(|e| napi::Error::from_reason(format!("Failed to extract tables: {}", e)))?;
    Ok(NapiHwpResult {
        text,
        tables: tables.into_iter().map(NapiTableData::from).collect(),
    })
}

/// Extract text from HWPX file
#[napi]
pub fn parse_hwpx_file(path: String) -> napi::Result<String> {
    let mut parser = mdm_core::hwpx::HwpxParser::open(&path)
        .map_err(|e| napi::Error::from_reason(format!("Failed to open HWPX: {}", e)))?;
    let doc = parser
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("Failed to parse HWPX: {}", e)))?;
    Ok(doc.sections.join("\n"))
}
