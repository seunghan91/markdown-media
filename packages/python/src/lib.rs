//! Python bindings for MDM Core Engine via PyO3

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::{PyBytes, PyDict};
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

/// Collapse duplicate image extensions like "BIN0001.jpg.jpeg" → "BIN0001.jpg".
/// mdm-core HWP parser appends img.format ("jpeg") to a name that may already
/// carry an alias extension (".jpg"), producing the double-extension form.
fn normalize_hwp_image_name(name: &str, format: &str) -> String {
    const IMAGE_EXTS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "emf", "wmf",
    ];
    let is_image_ext = |e: &str| IMAGE_EXTS.contains(&e);

    if name.is_empty() {
        let ext = if format.is_empty() { "bin" } else { format };
        return format!("image.{}", ext.to_lowercase());
    }

    // Strip trailing ".{format}" if the remaining name still ends in an image extension.
    // Handles "BIN0001.jpg.jpeg" + format="jpeg" → "BIN0001.jpg".
    let format_norm = format.to_lowercase();
    if !format_norm.is_empty() {
        let suffix = format!(".{}", format_norm);
        if name.to_lowercase().ends_with(&suffix) {
            let stripped_len = name.len() - suffix.len();
            let stripped = &name[..stripped_len];
            if let Some(inner_ext) = Path::new(stripped).extension().and_then(|e| e.to_str()) {
                if is_image_ext(&inner_ext.to_lowercase()) {
                    return stripped.to_string();
                }
            }
            // Original name had no inner extension — keep the appended format.
            return name.to_string();
        }
    }

    // Name already has extension → use as-is.
    if Path::new(name).extension().is_some() {
        return name.to_string();
    }

    // No extension anywhere → attach format if available.
    if !format_norm.is_empty() {
        format!("{}.{}", name, format_norm)
    } else {
        name.to_string()
    }
}

/// Extract images from a document (bytes).
///
/// Returns a dict mapping filename (with extension) -> raw image bytes.
/// Supports HWP / HWPX / DOCX / PDF. Filename collisions are resolved with an index suffix.
#[pyfunction]
fn extract_images<'py>(py: Python<'py>, data: &[u8], filename: &str) -> PyResult<Bound<'py, PyDict>> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let result = PyDict::new(py);
    let mut used = std::collections::HashSet::<String>::new();

    let mut insert = |name: String, bytes: Vec<u8>| -> PyResult<()> {
        let mut final_name = name.clone();
        let mut n: usize = 1;
        while used.contains(&final_name) {
            let stem = Path::new(&name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&name);
            let ext_part = Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            final_name = format!("{}_{}{}", stem, n, ext_part);
            n += 1;
        }
        used.insert(final_name.clone());
        result.set_item(final_name, PyBytes::new(py, &bytes))?;
        Ok(())
    };

    match ext.as_str() {
        "hwp" => {
            let mut parser = mdm_core::hwp::HwpParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("HWP error: {}", e)))?;
            let images = parser
                .extract_images()
                .map_err(|e| PyValueError::new_err(format!("HWP image error: {}", e)))?;
            for img in images {
                // Normalize filename: mdm-core may produce "BIN0001.jpg.jpeg"
                // (name already has .jpg, format is "jpeg"). Collapse to one extension.
                let fname = normalize_hwp_image_name(&img.name, &img.format);
                insert(fname, img.data)?;
            }
        }
        "hwpx" => {
            let mut parser = mdm_core::hwpx::HwpxParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("HWPX error: {}", e)))?;
            let doc = parser
                .parse()
                .map_err(|e| PyValueError::new_err(format!("HWPX parse error: {}", e)))?;
            for img in doc.image_info {
                let fname = Path::new(&img.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("{}.bin", img.id));
                insert(fname, img.data)?;
            }
        }
        "docx" => {
            let mut parser = mdm_core::docx::DocxParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("DOCX error: {}", e)))?;
            let images = parser
                .extract_images()
                .map_err(|e| PyValueError::new_err(format!("DOCX image error: {}", e)))?;
            for img in images {
                if let Some(bytes) = img.data {
                    let fname = if !img.filename.is_empty() {
                        img.filename
                    } else if !img.path.is_empty() {
                        Path::new(&img.path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(String::from)
                            .unwrap_or_else(|| format!("{}.bin", img.id))
                    } else {
                        format!("{}.bin", img.id)
                    };
                    insert(fname, bytes)?;
                }
            }
        }
        "pdf" => {
            let parser = mdm_core::pdf::PdfParser::from_bytes(data.to_vec())
                .map_err(|e| PyValueError::new_err(format!("PDF error: {}", e)))?;
            for img in parser.extract_images() {
                let ext_guess = match img.format {
                    mdm_core::pdf::ImageFormat::Jpeg => "jpg",
                    mdm_core::pdf::ImageFormat::Png => "png",
                    _ => "bin",
                };
                insert(format!("{}.{}", img.id, ext_guess), img.data)?;
            }
        }
        _ => return Err(PyValueError::new_err(format!("Unsupported: .{}", ext))),
    }

    Ok(result)
}

/// Configure the number of threads for parallel PDF processing.
///
/// Must be called before any PDF conversion. Pass 0 for auto-detect (CPU cores).
#[pyfunction]
fn set_threads(n: usize) -> PyResult<()> {
    let threads = if n == 0 { num_cpus::get() } else { n };
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .map_err(|e| PyValueError::new_err(format!("Thread pool error: {}", e)))
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
    m.add_function(wrap_pyfunction!(extract_images, m)?)?;
    m.add_function(wrap_pyfunction!(set_threads, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
