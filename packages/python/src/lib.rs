//! Python bindings for MDM Core Engine via PyO3

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use std::path::Path;

/// Convert a file to Markdown by path.
#[pyfunction]
fn convert_file(path: &str) -> PyResult<String> {
    let ext = Path::new(path).extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "hwp" => {
            let mut parser = mdm_core::hwp::HwpParser::open(path)
                .map_err(|e| PyValueError::new_err(format!("HWP error: {}", e)))?;
            let md = parser.extract_text()
                .map_err(|e| PyValueError::new_err(format!("HWP parse error: {}", e)))?;
            Ok(md)
        }
        "hwpx" => {
            let mut parser = mdm_core::hwpx::HwpxParser::open(path)
                .map_err(|e| PyValueError::new_err(format!("HWPX error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("HWPX parse error: {}", e)))?;
            Ok(doc.sections.join("\n\n"))
        }
        "pdf" => {
            let parser = mdm_core::pdf::PdfParser::open(path)
                .map_err(|e| PyValueError::new_err(format!("PDF error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("PDF parse error: {}", e)))?;
            Ok(doc.to_markdown_with_layout())
        }
        "docx" => {
            let mut parser = mdm_core::docx::DocxParser::open(path)
                .map_err(|e| PyValueError::new_err(format!("DOCX error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("DOCX parse error: {}", e)))?;
            Ok(doc.to_markdown())
        }
        _ => Err(PyValueError::new_err(format!("Unsupported: .{}", ext))),
    }
}

/// Convert raw bytes to Markdown.
#[pyfunction]
fn convert_bytes(data: &[u8], filename: &str) -> PyResult<String> {
    let ext = Path::new(filename).extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "hwp" => {
            let mut parser = mdm_core::hwp::HwpParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("HWP error: {}", e)))?;
            let md = parser.extract_text()
                .map_err(|e| PyValueError::new_err(format!("HWP parse error: {}", e)))?;
            Ok(md)
        }
        "hwpx" => {
            let mut parser = mdm_core::hwpx::HwpxParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("HWPX error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("HWPX parse error: {}", e)))?;
            Ok(doc.sections.join("\n\n"))
        }
        "pdf" => {
            let parser = mdm_core::pdf::PdfParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("PDF error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("PDF parse error: {}", e)))?;
            Ok(doc.to_markdown_with_layout())
        }
        "docx" => {
            let mut parser = mdm_core::docx::DocxParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("DOCX error: {}", e)))?;
            let doc = parser.parse()
                .map_err(|e| PyValueError::new_err(format!("DOCX parse error: {}", e)))?;
            Ok(doc.to_markdown())
        }
        _ => Err(PyValueError::new_err(format!("Unsupported: .{}", ext))),
    }
}

/// Convert a file and return JSON with metadata + content.
#[pyfunction]
fn convert_file_to_json(path: &str) -> PyResult<String> {
    let md = convert_file(path)?;
    let ext = Path::new(path).extension()
        .and_then(|e| e.to_str()).unwrap_or("unknown");
    let json = serde_json::json!({
        "format": ext,
        "source": path,
        "markdown": md,
    });
    Ok(serde_json::to_string_pretty(&json).unwrap_or_default())
}

/// Detect document format from filename/magic bytes.
#[pyfunction]
fn detect_format(data: &[u8], filename: &str) -> String {
    let ext = Path::new(filename).extension()
        .and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "hwp" | "hwpx" | "pdf" | "docx" => return ext,
        _ => {}
    }
    if data.len() >= 4 {
        if &data[0..4] == b"%PDF" { return "pdf".into(); }
        if data[0..4] == [0xD0, 0xCF, 0x11, 0xE0] { return "hwp".into(); }
        if &data[0..2] == b"PK" { return "docx".into(); } // simplified
    }
    "unknown".into()
}

/// Get MDM core version.
#[pyfunction]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Python module
#[pymodule]
fn _mdm_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(convert_file, m)?)?;
    m.add_function(wrap_pyfunction!(convert_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(convert_file_to_json, m)?)?;
    m.add_function(wrap_pyfunction!(detect_format, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
