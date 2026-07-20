//! PDF parser implementation using pdf-extract
//!
//! Provides text extraction from PDF files with page-by-page support,
//! image extraction, metadata parsing, encryption detection, and layout preservation.

use crate::utils::bounded_io::{read_limited, MAX_PDF_FILE, MAX_PDF_STREAM};
use flate2::read::ZlibDecoder;
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use thiserror::Error;

/// PDF-specific errors
#[derive(Error, Debug)]
pub enum PdfError {
    #[error("PDF is encrypted and requires a password")]
    EncryptedNoPassword,

    #[error("Invalid password for encrypted PDF")]
    InvalidPassword,

    #[error("Unsupported encryption algorithm: {0}")]
    UnsupportedEncryption(String),

    #[error("PDF parsing error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

/// PDF encryption information
#[derive(Debug, Clone)]
pub struct EncryptionInfo {
    /// Encryption algorithm version (1-5)
    pub version: u32,
    /// Encryption revision (2-6)
    pub revision: u32,
    /// Key length in bits (40-256)
    pub key_length: u32,
    /// Whether user password is set
    pub user_password_set: bool,
    /// Whether owner password is set
    pub owner_password_set: bool,
    /// Permissions flags
    pub permissions: i32,
}

/// Layout element types for position-aware content
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutElementType {
    /// Text content
    Text,
    /// Image reference
    Image,
    /// Table structure
    Table,
    /// Horizontal line/rule
    HorizontalRule,
    /// Page break marker
    PageBreak,
    /// Header region
    Header,
    /// Footer region
    Footer,
    /// Paragraph break
    ParagraphBreak,
    /// List item
    ListItem,
}

/// A layout element with position and styling information
#[derive(Debug, Clone, serde::Serialize)]
pub struct LayoutElement {
    /// Type of layout element
    pub element_type: LayoutElementType,
    /// Content (for text elements)
    pub content: String,
    /// Page number (1-indexed)
    pub page: usize,
    /// X position in points from left
    pub x: f64,
    /// Y position in points from bottom
    pub y: f64,
    /// Width in points
    pub width: f64,
    /// Height in points
    pub height: f64,
    /// Font size in points (for text)
    pub font_size: Option<f64>,
    /// Font name (for text)
    pub font_name: Option<String>,
    /// Text alignment
    pub alignment: TextAlignment,
    /// Whether text is bold
    pub is_bold: bool,
    /// Whether text is italic
    pub is_italic: bool,
    /// Line spacing multiplier
    pub line_spacing: f64,
    /// Indent level (for nested content)
    pub indent_level: u32,
    /// Reference ID (for images, tables)
    pub ref_id: Option<String>,
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Internal text block for layout grouping
#[derive(Debug, Clone)]
struct TextBlock {
    content: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    font_size: Option<f64>,
    font_name: Option<String>,
    is_bold: bool,
    is_italic: bool,
}

/// PDF document parser
pub struct PdfParser {
    path: std::path::PathBuf,
    data: Vec<u8>,
}

/// Extracted PDF document
#[derive(Debug, Clone)]
pub struct PdfDocument {
    pub version: String,
    pub page_count: usize,
    pub pages: Vec<PageContent>,
    pub metadata: PdfMetadata,
    pub images: Vec<PdfImage>,
    pub fonts: Vec<PdfFont>,
    pub tables: Vec<PdfTable>,
    pub layout: Vec<LayoutElement>,
}

/// Extracted image from PDF
#[derive(Debug, Clone)]
pub struct PdfImage {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: Vec<u8>,
    pub page: Option<usize>,
}

/// Image format detected from PDF stream
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Raw,  // Uncompressed or unknown format
}

/// Font information extracted from PDF
#[derive(Debug, Clone)]
pub struct PdfFont {
    pub name: String,
    pub base_font: String,
    pub is_bold: bool,
    pub is_italic: bool,
}

/// Font style detected from font name analysis
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FontStyle {
    pub is_bold: bool,
    pub is_italic: bool,
}

/// Text element with position information
#[derive(Debug, Clone)]
pub struct PositionedText {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub page: usize,
    pub font_size: Option<f64>,
    pub font_name: Option<String>,
}

/// Detected table from PDF
#[derive(Debug, Clone)]
pub struct PdfTable {
    pub page: usize,
    pub rows: Vec<Vec<String>>,
    pub column_count: usize,
    /// Top Y coordinate (highest Y in PDF space) of the detected table region.
    /// Used by the renderer to deduplicate inline text vs. trailing `## Tables`.
    pub y_top: f64,
    /// Bottom Y coordinate (lowest Y in PDF space) of the detected table region.
    pub y_bottom: f64,
}

/// Content of a single PDF page
#[derive(Debug, Clone)]
pub struct PageContent {
    pub page_number: usize,
    pub text: String,
}

/// PDF metadata
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub creator: String,
    pub producer: String,
}

impl PdfParser {
    /// Open a PDF file.
    ///
    /// File size is capped at `MAX_PDF_FILE` (512 MB) so a pathological
    /// input cannot exhaust process memory before we even start parsing.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let data = read_limited(&mut file, MAX_PDF_FILE)?;

        // Validate PDF magic bytes
        if data.len() < 5 || &data[0..5] != b"%PDF-" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid PDF file"));
        }

        Ok(PdfParser { path, data })
    }

    /// Create a PdfParser from in-memory data (no file path required).
    ///
    /// This is the primary constructor for WASM environments where
    /// file system access is unavailable.
    pub fn from_bytes(data: Vec<u8>) -> io::Result<Self> {
        if data.len() < 5 || &data[0..5] != b"%PDF-" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Not a valid PDF file",
            ));
        }
        Ok(PdfParser {
            path: std::path::PathBuf::from("<memory>"),
            data,
        })
    }

    /// Parse the PDF document from in-memory data only (no file-path access).
    ///
    /// Unlike [`parse`], this method uses `pdf_extract::extract_text_from_mem`
    /// so it works in WASM and other sandboxed environments.
    pub fn parse_from_memory(&self) -> io::Result<PdfDocument> {
        let version = self.extract_version();

        // In-memory path cannot use external `pdftotext` fallback (it needs a
        // file path). Wrap pdf-extract in `catch_unwind` so CJK-font panics
        // become recoverable errors instead of aborting the process. Callers
        // that want CJK-safe extraction must use `parse()` with a file path.
        let data = self.data.clone();
        let full_text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            pdf_extract::extract_text_from_mem(&data)
        }))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "PDF extraction panicked (likely CJK CID font — use parse() for pdftotext fallback)",
            )
        })?
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("PDF extraction failed: {}", e)))?;

        let page_count = self.get_page_count().unwrap_or(1);
        let pages = self.split_into_pages(&full_text, page_count);
        let metadata = self.extract_metadata();
        let images = self.extract_images();
        let fonts = self.extract_fonts();
        let tables = self.detect_tables();
        let layout = self.extract_layout();

        Ok(PdfDocument {
            version,
            page_count,
            pages,
            metadata,
            images,
            fonts,
            tables,
            layout,
        })
    }

    /// Check if the PDF is encrypted
    /// Per-page triage: classify each page as TextNative / Scanned / Mixed
    /// before deciding whether to route to the external OCR bridge.
    ///
    /// Stage 1 only (see `plan/pdf-triage.md`). Returns an empty Vec if the
    /// document cannot be parsed as PDF.
    pub fn triage(&self) -> Vec<super::triage::PageTriage> {
        self.triage_with_config(&super::triage::TriageConfig::default())
    }

    /// Triage with a custom threshold configuration.
    pub fn triage_with_config(
        &self,
        cfg: &super::triage::TriageConfig,
    ) -> Vec<super::triage::PageTriage> {
        match lopdf::Document::load_mem(&self.data) {
            Ok(doc) => super::triage::classify_document(&doc, cfg),
            Err(_) => Vec::new(),
        }
    }

    pub fn is_encrypted(&self) -> bool {
        if let Ok(doc) = lopdf::Document::load_mem(&self.data) {
            // Check for Encrypt dictionary in trailer
            doc.trailer.get(b"Encrypt").is_ok()
        } else {
            // If we can't load the document, it might be encrypted
            // Check for encryption markers in raw data
            self.data.windows(8).any(|w| w == b"/Encrypt")
        }
    }

    /// Get encryption information if the PDF is encrypted
    pub fn get_encryption_info(&self) -> Option<EncryptionInfo> {
        let doc = lopdf::Document::load_mem(&self.data).ok()?;

        let encrypt_ref = doc.trailer.get(b"Encrypt").ok()?;
        let encrypt_id = encrypt_ref.as_reference().ok()?;
        let encrypt_dict = doc.get_dictionary(encrypt_id).ok()?;

        // Get encryption version (V)
        let version = encrypt_dict
            .get(b"V")
            .ok()
            .and_then(|v| v.as_i64().ok())
            .unwrap_or(0) as u32;

        // Get encryption revision (R)
        let revision = encrypt_dict
            .get(b"R")
            .ok()
            .and_then(|r| r.as_i64().ok())
            .unwrap_or(0) as u32;

        // Get key length (Length) - defaults based on version
        let key_length = encrypt_dict
            .get(b"Length")
            .ok()
            .and_then(|l| l.as_i64().ok())
            .unwrap_or(match version {
                1 => 40,
                2 | 3 => 128,
                4 | 5 => 256,
                _ => 40,
            }) as u32;

        // Get permissions (P)
        let permissions = encrypt_dict
            .get(b"P")
            .ok()
            .and_then(|p| p.as_i64().ok())
            .unwrap_or(0) as i32;

        // Check for user/owner password strings
        let user_password_set = encrypt_dict.get(b"U").is_ok();
        let owner_password_set = encrypt_dict.get(b"O").is_ok();

        Some(EncryptionInfo {
            version,
            revision,
            key_length,
            user_password_set,
            owner_password_set,
            permissions,
        })
    }

    /// Attempt to decrypt the PDF with the given password
    ///
    /// Returns a new PdfParser with the decrypted content if successful.
    /// Supports RC4 (40-128 bit) and AES (128-256 bit) encryption.
    pub fn decrypt(&self, password: &str) -> Result<Self, PdfError> {
        if !self.is_encrypted() {
            // Not encrypted, return a copy
            return Ok(PdfParser {
                path: self.path.clone(),
                data: self.data.clone(),
            });
        }

        let encryption_info = self.get_encryption_info()
            .ok_or(PdfError::ParseError("Cannot read encryption info".to_string()))?;

        // Check if encryption is supported
        // lopdf supports V=1,2 (RC4) and limited V=4,5 (AES)
        match encryption_info.version {
            1 | 2 | 4 => {
                // Try to load and decrypt with lopdf
                self.decrypt_with_lopdf(password, &encryption_info)
            }
            5 => {
                // AES-256 (PDF 2.0)
                self.decrypt_with_lopdf(password, &encryption_info)
            }
            v => Err(PdfError::UnsupportedEncryption(format!("version {}", v))),
        }
    }

    /// Decrypt using lopdf's built-in decryption
    fn decrypt_with_lopdf(&self, password: &str, info: &EncryptionInfo) -> Result<Self, PdfError> {
        // lopdf's Document::load_mem will attempt decryption with empty password
        // For password-protected PDFs, we need a different approach

        // First, try loading with empty password (for owner-restricted, user-accessible PDFs)
        if let Ok(doc) = lopdf::Document::load_mem(&self.data) {
            // Check if we can access content
            if doc.get_pages().len() > 0 {
                // Document loaded successfully, may already be decrypted
                // or accessible without password
                return Ok(PdfParser {
                    path: self.path.clone(),
                    data: self.data.clone(),
                });
            }
        }

        // Try with provided password
        // Note: lopdf doesn't have direct password support, so we need to check
        // if the document can be used with the computed decryption key

        // For now, attempt to use qpdf or similar tool if available
        // This is a fallback mechanism
        self.try_external_decryption(password, info)
    }

    /// Try external tools for decryption (fallback)
    fn try_external_decryption(&self, password: &str, _info: &EncryptionInfo) -> Result<Self, PdfError> {
        use std::process::Command;
        use std::io::Write;

        // Create temp files
        let temp_input = std::env::temp_dir().join(format!("mdm_pdf_in_{}.pdf", std::process::id()));
        let temp_output = std::env::temp_dir().join(format!("mdm_pdf_out_{}.pdf", std::process::id()));

        // Write encrypted PDF to temp file
        {
            let mut file = File::create(&temp_input)?;
            file.write_all(&self.data)?;
        }

        // Try qpdf first (most common on Unix systems)
        let qpdf_result = Command::new("qpdf")
            .args([
                "--password", password,
                "--decrypt",
                temp_input.to_str().unwrap(),
                temp_output.to_str().unwrap(),
            ])
            .output();

        let success = match qpdf_result {
            Ok(output) => output.status.success(),
            Err(_) => {
                // Try pdftk as fallback
                let pdftk_result = Command::new("pdftk")
                    .args([
                        temp_input.to_str().unwrap(),
                        "input_pw", password,
                        "output", temp_output.to_str().unwrap(),
                    ])
                    .output();

                match pdftk_result {
                    Ok(output) => output.status.success(),
                    Err(_) => false,
                }
            }
        };

        // Clean up input temp file
        let _ = std::fs::remove_file(&temp_input);

        if success {
            // Read decrypted file (same file-size ceiling as PdfParser::open)
            let mut file = File::open(&temp_output)?;
            let decrypted_data = read_limited(&mut file, MAX_PDF_FILE)?;

            // Clean up output temp file
            let _ = std::fs::remove_file(&temp_output);

            Ok(PdfParser {
                path: self.path.clone(),
                data: decrypted_data,
            })
        } else {
            // Clean up output temp file if it exists
            let _ = std::fs::remove_file(&temp_output);

            Err(PdfError::InvalidPassword)
        }
    }

    /// Check if decryption is needed and attempt with empty password
    pub fn try_auto_decrypt(&self) -> Result<Self, PdfError> {
        if !self.is_encrypted() {
            return Ok(PdfParser {
                path: self.path.clone(),
                data: self.data.clone(),
            });
        }

        // Try empty password first (common for owner-only restrictions)
        self.decrypt("")
    }

    /// Extract layout elements with position information
    ///
    /// Returns a vector of layout elements that preserve the visual structure
    /// of the PDF, including text positions, images, and detected regions.
    /// Pages are processed in parallel using Rayon for improved performance
    /// on multi-page documents.
    pub fn extract_layout(&self) -> Vec<LayoutElement> {
        let doc = match lopdf::Document::load_mem(&self.data) {
            Ok(d) => d,
            Err(_) => return vec![],
        };

        // Collect page info sequentially (fast, just reads the page tree)
        let pages: Vec<(u32, lopdf::ObjectId)> = doc.get_pages().into_iter().collect();

        // Extract images once upfront (avoids redundant work per page)
        let all_images = self.extract_images();

        let data = &self.data;

        // Process pages in parallel — each thread loads its own Document
        // because lopdf::Document is neither Send nor Sync.
        let mut all_elements: Vec<LayoutElement> = pages
            .par_iter()
            .flat_map(|(page_num, page_id)| {
                let thread_doc = match lopdf::Document::load_mem(data) {
                    Ok(d) => d,
                    Err(_) => return vec![],
                };
                self.extract_layout_for_page(&thread_doc, *page_num as usize, *page_id, &all_images)
            })
            .collect();

        // Stable sort by page only — the per-page extractor already emits
        // elements in correct reading order (column-by-column for 2-column
        // layouts). Re-sorting by Y here would flatten columns back into
        // an interleaved mess and defeat `detect_column_split`.
        all_elements.sort_by(|a, b| a.page.cmp(&b.page));

        all_elements
    }

    /// Extract layout elements for a single page.
    ///
    /// This is the per-page workhorse called from `extract_layout()`.
    /// It is designed to be called from parallel iterators — each invocation
    /// receives its own `doc` reference (loaded per-thread).
    fn extract_layout_for_page(
        &self,
        doc: &lopdf::Document,
        page_num: usize,
        page_id: lopdf::ObjectId,
        all_images: &[PdfImage],
    ) -> Vec<LayoutElement> {
        let mut elements = Vec::new();

        // Get page dimensions
        let (page_width, page_height) = self.get_page_dimensions(doc, page_id);

        // Add page break marker for pages after the first
        if page_num > 1 {
            elements.push(LayoutElement {
                element_type: LayoutElementType::PageBreak,
                content: String::new(),
                page: page_num,
                x: 0.0,
                y: page_height,
                width: page_width,
                height: 0.0,
                font_size: None,
                font_name: None,
                alignment: TextAlignment::Left,
                is_bold: false,
                is_italic: false,
                line_spacing: 1.0,
                indent_level: 0,
                ref_id: None,
            });
        }

        // Extract positioned text elements
        let positioned_texts = self.extract_positioned_text(doc, page_id);

        // Group text by visual blocks and detect formatting
        let text_blocks = self.group_text_into_blocks(&positioned_texts, page_height);

        for block in text_blocks {
            // Detect if this might be a header/footer
            // Use 95%/6% thresholds to avoid clipping heading text near page top
            let element_type = if block.y > page_height * 0.95 {
                LayoutElementType::Header
            } else if block.y < page_height * 0.06 {
                LayoutElementType::Footer
            } else if block.content.starts_with('•') || block.content.starts_with('-')
                || block.content.starts_with("* ")
                || block.content.starts_with("\u{2022}")  // bullet character
                || block.content.starts_with("\u{2013}")  // en-dash
            {
                LayoutElementType::ListItem
            } else {
                LayoutElementType::Text
            };

            // Detect alignment from position
            let alignment = if block.x < page_width * 0.2 {
                TextAlignment::Left
            } else if block.x > page_width * 0.6 {
                TextAlignment::Right
            } else {
                TextAlignment::Center
            };

            // Calculate indent level
            let indent_level = ((block.x / 36.0) as u32).min(10); // 36pt = 0.5 inch

            elements.push(LayoutElement {
                element_type,
                content: block.content,
                page: page_num,
                x: block.x,
                y: block.y,
                width: block.width,
                height: block.height,
                font_size: block.font_size,
                font_name: block.font_name,
                alignment,
                is_bold: block.is_bold,
                is_italic: block.is_italic,
                line_spacing: 1.2, // Default line spacing
                indent_level,
                ref_id: None,
            });
        }

        // Add image elements that belong to this page
        for image in all_images {
            if image.page == Some(page_num) || image.page.is_none() {
                elements.push(LayoutElement {
                    element_type: LayoutElementType::Image,
                    content: String::new(),
                    page: page_num,
                    x: 0.0, // Position not available from image extraction
                    y: 0.0,
                    width: image.width as f64,
                    height: image.height as f64,
                    font_size: None,
                    font_name: None,
                    alignment: TextAlignment::Center,
                    is_bold: false,
                    is_italic: false,
                    line_spacing: 1.0,
                    indent_level: 0,
                    ref_id: Some(image.id.clone()),
                });
            }
        }

        elements
    }

    /// Get page dimensions in points
    fn get_page_dimensions(&self, doc: &lopdf::Document, page_id: lopdf::ObjectId) -> (f64, f64) {
        // Default to US Letter size
        let default_width = 612.0;
        let default_height = 792.0;

        if let Ok(page_dict) = doc.get_dictionary(page_id) {
            // Try MediaBox first, then CropBox
            for key in &[b"MediaBox".as_slice(), b"CropBox".as_slice()] {
                if let Ok(box_obj) = page_dict.get(*key) {
                    if let Ok(arr) = box_obj.as_array() {
                        if arr.len() >= 4 {
                            let width = extract_number(&arr[2]).unwrap_or(default_width);
                            let height = extract_number(&arr[3]).unwrap_or(default_height);
                            return (width, height);
                        }
                    }
                }
            }
        }

        (default_width, default_height)
    }

    /// Group positioned text into logical blocks.
    ///
    /// Multi-column handling: before the normal Y-descending sort, we probe
    /// for a 2-column layout via [`detect_column_split`]. When a column
    /// gutter is detected the items are partitioned into left / right
    /// columns and each column is walked top-to-bottom independently,
    /// then concatenated (left first, then right). This is the kordoc v2.1
    /// fix for 2-column academic papers and reports where the old
    /// "sort by Y then X" ordering produced interleaved junk like
    /// `L1 R1 L2 R2 L3 R3 …`.
    fn group_text_into_blocks(&self, texts: &[PositionedText], page_height: f64) -> Vec<TextBlock> {
        if texts.is_empty() {
            return Vec::new();
        }

        // 2-column layout probe. When a split is detected, segment the page
        // by Y: any full-width content above or below the column region is
        // handled as a separate single-column block so titles / "After
        // Columns" trailers land at their natural position.
        if let Some(split_x) = detect_column_split(texts) {
            // Find the Y range where BOTH columns have runs — that's the
            // region where the 2-column layout is active. Outside that range,
            // runs are treated as full-width content.
            let is_left = |t: &PositionedText| t.x < split_x;
            let left_ys: Vec<f64> = texts.iter().filter(|t| is_left(t)).map(|t| t.y).collect();
            let right_ys: Vec<f64> = texts.iter().filter(|t| !is_left(t)).map(|t| t.y).collect();
            if left_ys.is_empty() || right_ys.is_empty() {
                // One side empty → not a real 2-column layout; fall through.
                return self.group_text_single_column(texts, page_height);
            }
            let col_y_max = left_ys.iter().chain(&right_ys).cloned().fold(f64::NEG_INFINITY, f64::max)
                .min(left_ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
                .min(right_ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
            let col_y_min = left_ys.iter().cloned().fold(f64::INFINITY, f64::min)
                .max(right_ys.iter().cloned().fold(f64::INFINITY, f64::min));

            // Partition into 4 regions in reading order:
            //   pre_column (above col_y_max) — full-width
            //   left column (within [col_y_min, col_y_max])
            //   right column (within [col_y_min, col_y_max])
            //   post_column (below col_y_min) — full-width
            let mut pre: Vec<PositionedText> = Vec::new();
            let mut left: Vec<PositionedText> = Vec::new();
            let mut right: Vec<PositionedText> = Vec::new();
            let mut post: Vec<PositionedText> = Vec::new();
            for t in texts.iter().cloned() {
                if t.y > col_y_max + 2.0 {
                    pre.push(t);
                } else if t.y < col_y_min - 2.0 {
                    post.push(t);
                } else if is_left(&t) {
                    left.push(t);
                } else {
                    right.push(t);
                }
            }

            let mut blocks = self.group_text_single_column(&pre, page_height);
            blocks.extend(self.group_text_single_column(&left, page_height));
            blocks.extend(self.group_text_single_column(&right, page_height));
            blocks.extend(self.group_text_single_column(&post, page_height));
            return blocks;
        }

        self.group_text_single_column(texts, page_height)
    }

    /// Single-column text-to-block walker. Sort by Y descending (PDF
    /// coordinates), then X ascending, coalesce lines within Y_TOLERANCE,
    /// and emit one `TextBlock` per line.
    fn group_text_single_column(
        &self,
        texts: &[PositionedText],
        _page_height: f64,
    ) -> Vec<TextBlock> {
        let mut blocks = Vec::new();

        if texts.is_empty() {
            return blocks;
        }

        // Group by Y position with tolerance
        const Y_TOLERANCE: f64 = 12.0; // About 1 line height
        let mut current_block: Option<TextBlock> = None;

        // Sort by Y (descending) with X tiebreaker, clustering nearby Y
        // values into the same bucket so two runs within a few points of
        // each other — e.g. a list marker "1" at y=415.9 and its sibling
        // text "First item" at y=416.9 — are treated as one visual line
        // and ordered by X.
        //
        // Pre-bucket Y by `floor(y / Y_CLUSTER_TOL)` so the comparison
        // is transitive and satisfies the total-order contract that
        // Rust's sort now panics on. A direct `abs(a.y - b.y) < tol`
        // check is NOT transitive and triggers that panic on dense
        // documents (IRS W-9 forms — hundreds of close-Y runs).
        const Y_CLUSTER_TOL: f64 = 2.0;
        let mut sorted_texts: Vec<PositionedText> = texts.to_vec();
        sorted_texts.sort_by(|a, b| {
            let ba = (a.y / Y_CLUSTER_TOL).floor() as i64;
            let bb = (b.y / Y_CLUSTER_TOL).floor() as i64;
            bb.cmp(&ba)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });

        for text in sorted_texts {
            // Detect bold/italic from font name
            let font_style = text.font_name.as_deref()
                .map(|n| detect_font_style(n))
                .unwrap_or(FontStyle { is_bold: false, is_italic: false });

            match &mut current_block {
                Some(block) if (block.y - text.y).abs() < Y_TOLERANCE => {
                    // Same line — but split block if font style changes
                    let style_changed = block.is_bold != font_style.is_bold
                        || block.is_italic != font_style.is_italic;

                    if style_changed && !block.content.is_empty() {
                        // Emit current block and start new one at same Y
                        blocks.push(block.clone());
                        *block = TextBlock {
                            content: text.text.clone(),
                            x: text.x,
                            y: text.y,
                            width: 100.0,
                            height: text.font_size.unwrap_or(12.0),
                            font_size: text.font_size,
                            font_name: text.font_name.clone(),
                            is_bold: font_style.is_bold,
                            is_italic: font_style.is_italic,
                        };
                    } else {
                        // Same style, append text
                        if !block.content.is_empty() && !text.text.is_empty() {
                            block.content.push(' ');
                        }
                        block.content.push_str(&text.text);
                        block.width = (text.x - block.x).max(block.width);
                        if block.font_size.is_none() && text.font_size.is_some() {
                            block.font_size = text.font_size;
                        }
                        if block.font_name.is_none() && text.font_name.is_some() {
                            block.font_name = text.font_name.clone();
                        }
                    }
                }
                Some(block) => {
                    // New line, save current block and start new one
                    blocks.push(block.clone());
                    current_block = Some(TextBlock {
                        content: text.text.clone(),
                        x: text.x,
                        y: text.y,
                        width: 100.0,
                        height: text.font_size.unwrap_or(12.0),
                        font_size: text.font_size,
                        font_name: text.font_name.clone(),
                        is_bold: font_style.is_bold,
                        is_italic: font_style.is_italic,
                    });
                }
                None => {
                    current_block = Some(TextBlock {
                        content: text.text.clone(),
                        x: text.x,
                        y: text.y,
                        width: 100.0,
                        height: text.font_size.unwrap_or(12.0),
                        font_size: text.font_size,
                        font_name: text.font_name.clone(),
                        is_bold: font_style.is_bold,
                        is_italic: font_style.is_italic,
                    });
                }
            }
        }

        // Don't forget the last block
        if let Some(block) = current_block {
            blocks.push(block);
        }

        blocks
    }

    /// Parse the PDF document
    pub fn parse(&self) -> io::Result<PdfDocument> {
        let version = self.extract_version();

        // Use pdf-extract for text extraction; catch panics from CJK CID
        // fonts (pdf-extract panics on Identity-V / UniKS-UTF16-H encodings
        // commonly used by Korean documents) and fall back to `pdftotext`
        // (Poppler) when available — it handles all CJK CMaps correctly.
        let full_text = extract_text_with_fallback(&self.path, &self.data)?;

        // Try to get page count from lopdf
        let page_count = self.get_page_count().unwrap_or(1);

        // Split text into pages (simple heuristic: form feed or page markers)
        let pages = self.split_into_pages(&full_text, page_count);

        // Extract metadata
        let metadata = self.extract_metadata();

        // Extract images
        let images = self.extract_images();

        // Extract fonts
        let fonts = self.extract_fonts();

        // Detect tables
        let tables = self.detect_tables();

        // Extract layout information for heading/bold/italic detection
        let layout = self.extract_layout();

        Ok(PdfDocument {
            version,
            page_count,
            pages,
            metadata,
            images,
            fonts,
            tables,
            layout,
        })
    }

    /// Extract all images from PDF
    pub fn extract_images(&self) -> Vec<PdfImage> {
        let mut images = Vec::new();

        let doc = match lopdf::Document::load_mem(&self.data) {
            Ok(d) => d,
            Err(_) => return images,
        };

        let mut image_count = 0;

        // Iterate through all objects looking for images
        for (_object_id, object) in doc.objects.iter() {
            if let Ok(stream) = object.as_stream() {
                let dict = &stream.dict;

                // Check if this is an image XObject
                let is_image = dict.get(b"Subtype")
                    .ok()
                    .and_then(|s| s.as_name().ok())
                    .map(|n| n == b"Image")
                    .unwrap_or(false);

                if !is_image {
                    continue;
                }

                // Get image dimensions
                let width = dict.get(b"Width")
                    .ok()
                    .and_then(|w| w.as_i64().ok())
                    .unwrap_or(0) as u32;
                let height = dict.get(b"Height")
                    .ok()
                    .and_then(|h| h.as_i64().ok())
                    .unwrap_or(0) as u32;

                if width == 0 || height == 0 {
                    continue;
                }

                // Determine format from filter
                let filter: Option<Vec<u8>> = dict.get(b"Filter")
                    .ok()
                    .and_then(|f| f.as_name().ok())
                    .map(|n| n.to_vec());

                let (format, data) = match filter.as_deref() {
                    Some(b"DCTDecode") => {
                        // JPEG - use raw stream content
                        (ImageFormat::Jpeg, stream.content.clone())
                    }
                    Some(b"FlateDecode") => {
                        // Compressed data - decompress
                        match decompress_flate(&stream.content) {
                            Ok(decompressed) => (ImageFormat::Raw, decompressed),
                            Err(_) => continue,
                        }
                    }
                    _ => {
                        // Raw or unsupported format
                        (ImageFormat::Raw, stream.content.clone())
                    }
                };

                image_count += 1;
                images.push(PdfImage {
                    id: format!("image_{}", image_count),
                    width,
                    height,
                    format,
                    data,
                    page: None, // Page association would require more complex tracking
                });
            }
        }

        images
    }

    /// Extract all fonts from PDF
    pub fn extract_fonts(&self) -> Vec<PdfFont> {
        let mut fonts = Vec::new();

        let doc = match lopdf::Document::load_mem(&self.data) {
            Ok(d) => d,
            Err(_) => return fonts,
        };

        // Iterate through all objects looking for Font dictionaries
        for (_object_id, object) in doc.objects.iter() {
            if let Ok(dict) = object.as_dict() {
                // Check if this is a Font dictionary
                let is_font = dict.get(b"Type")
                    .ok()
                    .and_then(|t| t.as_name().ok())
                    .map(|n| n == b"Font")
                    .unwrap_or(false);

                if !is_font {
                    continue;
                }

                // Get font name (key used in content streams)
                let name = dict.get(b"Name")
                    .ok()
                    .and_then(|n| n.as_name().ok())
                    .map(|n| String::from_utf8_lossy(n).to_string())
                    .unwrap_or_default();

                // Get BaseFont (actual font name with style info)
                let base_font = dict.get(b"BaseFont")
                    .ok()
                    .and_then(|bf| bf.as_name().ok())
                    .map(|n| String::from_utf8_lossy(n).to_string())
                    .unwrap_or_default();

                if base_font.is_empty() {
                    continue;
                }

                // Detect bold/italic from font name
                let style = detect_font_style(&base_font);

                fonts.push(PdfFont {
                    name,
                    base_font: base_font.clone(),
                    is_bold: style.is_bold,
                    is_italic: style.is_italic,
                });
            }
        }

        // Remove duplicates based on base_font
        fonts.sort_by(|a, b| a.base_font.cmp(&b.base_font));
        fonts.dedup_by(|a, b| a.base_font == b.base_font);

        fonts
    }

    /// Detect tables in PDF using text position heuristics
    pub fn detect_tables(&self) -> Vec<PdfTable> {
        let mut tables = Vec::new();

        let doc = match lopdf::Document::load_mem(&self.data) {
            Ok(d) => d,
            Err(_) => return tables,
        };

        // Dual-strategy detection per page: line-based (ruling-line grid) first,
        // then the text-cluster heuristic as the line-less fallback, merged so
        // line geometry wins where the two overlap.
        for (page_num, page_id) in doc.get_pages() {
            let positioned_texts = self.extract_positioned_text(&doc, page_id);

            if positioned_texts.is_empty() {
                continue;
            }

            let line_tables: Vec<PdfTable> =
                super::table_detect::detect_line_tables(&doc, page_id, &positioned_texts, page_num as usize)
                    .into_iter()
                    .map(|d| d.pdf)
                    .collect();
            let cluster_tables = detect_tables_from_positions(&positioned_texts, page_num as usize);
            tables.extend(super::table_detect::merge_line_and_cluster(line_tables, cluster_tables));
        }

        tables
    }

    /// Extract text with position from a PDF page.
    ///
    /// Uses lopdf's content-stream decoder to walk PDF text operators
    /// properly — the prior implementation line-split the stream and only
    /// matched `line == "BT"` / `line.ends_with(" Tm")`, which misses the
    /// majority of real-world PDFs where multiple operators share a line.
    /// That silent failure is what kept mdm's table detector from seeing
    /// positioned text on most corporate reports.
    fn extract_positioned_text(&self, doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<PositionedText> {
        use lopdf::content::Content;
        use lopdf::Object;

        let mut texts = Vec::new();

        let content_bytes = match doc.get_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return texts,
        };

        let content = match Content::decode(&content_bytes) {
            Ok(c) => c,
            Err(_) => return texts,
        };

        // Resolve page `/Resources` → `/Font` subdictionary so we can look up
        // the `/ToUnicode` CMap for each Tf-selected font. Without this,
        // CID-encoded Korean fonts come out as garbage (we were decoding
        // 2-byte CIDs as UTF-8) and the table detector sees empty rows.
        let font_cmaps: std::collections::HashMap<String, ToUnicodeCMap> =
            build_page_font_cmaps(doc, page_id);
        let font_names: std::collections::HashMap<String, String> =
            build_page_font_names(doc, page_id);
        let mut current_font: Option<String> = None;
        let mut current_font_size: Option<f64> = None;

        // Current transformation matrix (graphics state) tracked across the
        // whole content stream. PDF uses a 3×3 affine stored as (a b c d e f):
        //   [ a c e ]
        //   [ b d f ]
        //   [ 0 0 1 ]
        // Text positioning outside the text block is controlled by `cm`,
        // which LEFT-multiplies the CTM. Without this, Tm/Td operands look
        // like they're all at origin because the page translation lives in
        // the cm chain. `q` saves the full state; `Q` pops it.
        let mut ctm_a = 1.0_f64;
        let mut ctm_b = 0.0_f64;
        let mut ctm_c = 0.0_f64;
        let mut ctm_d = 1.0_f64;
        let mut ctm_e = 0.0_f64;
        let mut ctm_f = 0.0_f64;
        let mut gs_stack: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::new();

        // Apply the CTM to a point to get its page-space coordinates.
        let apply_ctm = |x: f64, y: f64,
                         a: f64, b: f64, c: f64, d: f64, e: f64, f: f64|
         -> (f64, f64) { (a * x + c * y + e, b * x + d * y + f) };

        // Text state: matrix-derived position + per-show offset from TJ arrays
        // and relative Td/TD/T* moves. We track the text line matrix origin
        // (x, y) and emit text at that origin — good enough for row/column
        // clustering, which is the only thing the table detector cares about.
        let mut tx = 0.0_f64;
        let mut ty = 0.0_f64;
        // Leading set by TD / TL, used by T* and single-quote operator.
        let mut leading = 0.0_f64;
        let mut in_text = false;

        let read_num = |obj: &Object| -> Option<f64> {
            match obj {
                Object::Integer(n) => Some(*n as f64),
                Object::Real(n) => Some(*n as f64),
                _ => None,
            }
        };

        // Helper: resolve current font's CMap (if any) and decode bytes.
        // Kept as a tiny closure so all show ops go through the same path.
        let decode_show = |obj: &Object, current_font: &Option<String>| -> Option<String> {
            let bytes = collect_show_bytes(obj)?;
            if bytes.is_empty() { return None; }
            let cmap = current_font.as_ref().and_then(|n| font_cmaps.get(n));
            let text = match cmap {
                Some(c) => c.decode(&bytes),
                None => String::from_utf8_lossy(&bytes).to_string(),
            };
            if text.trim().is_empty() { None } else { Some(text) }
        };

        for op in &content.operations {
            match op.operator.as_str() {
                // ── Graphics state outside text blocks ──
                "q" => {
                    gs_stack.push((ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f));
                }
                "Q" => {
                    if let Some((a, b, c, d, e, f)) = gs_stack.pop() {
                        ctm_a = a; ctm_b = b; ctm_c = c;
                        ctm_d = d; ctm_e = e; ctm_f = f;
                    }
                }
                // cm a b c d e f — left-multiply the CTM.
                //   CTM_new = [cm_matrix] × [CTM_old]
                // In the (a b c d e f) row representation, the product of
                //   M1 = (a1 b1 c1 d1 e1 f1) and M2 = (a2 b2 c2 d2 e2 f2)  is
                //   (a1*a2 + b1*c2,
                //    a1*b2 + b1*d2,
                //    c1*a2 + d1*c2,
                //    c1*b2 + d1*d2,
                //    e1*a2 + f1*c2 + e2,
                //    e1*b2 + f1*d2 + f2)
                "cm" => {
                    if op.operands.len() >= 6 {
                        let v: Vec<f64> = op.operands.iter()
                            .take(6)
                            .map(|o| read_num(o).unwrap_or(0.0))
                            .collect();
                        let (a1, b1, c1, d1, e1, f1) = (v[0], v[1], v[2], v[3], v[4], v[5]);
                        let (a2, b2, c2, d2, e2, f2) = (ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f);
                        ctm_a = a1 * a2 + b1 * c2;
                        ctm_b = a1 * b2 + b1 * d2;
                        ctm_c = c1 * a2 + d1 * c2;
                        ctm_d = c1 * b2 + d1 * d2;
                        ctm_e = e1 * a2 + f1 * c2 + e2;
                        ctm_f = e1 * b2 + f1 * d2 + f2;
                    }
                }
                "BT" => {
                    in_text = true;
                    tx = 0.0;
                    ty = 0.0;
                }
                "ET" => {
                    in_text = false;
                }
                // Tf /F1 12 — select font (name, size).
                "Tf" => {
                    if let Some(Object::Name(name_bytes)) = op.operands.first() {
                        current_font = Some(String::from_utf8_lossy(name_bytes).to_string());
                    }
                    if let Some(size_obj) = op.operands.get(1) {
                        if let Some(sz) = read_num(size_obj) {
                            current_font_size = Some(sz);
                        }
                    }
                }
                _ if !in_text => continue,
                // Tm a b c d e f — absolute text matrix; position = (e, f)
                "Tm" => {
                    if op.operands.len() >= 6 {
                        if let (Some(e), Some(f)) = (
                            read_num(&op.operands[4]),
                            read_num(&op.operands[5]),
                        ) {
                            tx = e;
                            ty = f;
                        }
                    }
                }
                // Td tx ty — relative move
                "Td" => {
                    if op.operands.len() >= 2 {
                        if let (Some(dx), Some(dy)) = (
                            read_num(&op.operands[0]),
                            read_num(&op.operands[1]),
                        ) {
                            tx += dx;
                            ty += dy;
                        }
                    }
                }
                // TD tx ty — relative move + sets leading to -ty
                "TD" => {
                    if op.operands.len() >= 2 {
                        if let (Some(dx), Some(dy)) = (
                            read_num(&op.operands[0]),
                            read_num(&op.operands[1]),
                        ) {
                            tx += dx;
                            ty += dy;
                            leading = -dy;
                        }
                    }
                }
                // TL leading — set leading
                "TL" => {
                    if let Some(l) = op.operands.first().and_then(read_num) {
                        leading = l;
                    }
                }
                // T* — move to next line (uses leading)
                "T*" => {
                    ty -= leading;
                }
                // Tj (string) — show text at current position
                "Tj" => {
                    if let Some(t) = op.operands.first().and_then(|o| decode_show(o, &current_font)) {
                        let (px, py) = apply_ctm(tx, ty, ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f);
                        texts.push(PositionedText { text: t, x: px, y: py, page: 0, font_size: current_font_size, font_name: current_font.as_ref().and_then(|alias| font_names.get(alias).cloned()).or_else(|| current_font.clone()) });
                    }
                }
                // TJ [ array ] — show with kerning
                "TJ" => {
                    if let Some(t) = op.operands.first().and_then(|o| decode_show(o, &current_font)) {
                        let (px, py) = apply_ctm(tx, ty, ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f);
                        texts.push(PositionedText { text: t, x: px, y: py, page: 0, font_size: current_font_size, font_name: current_font.as_ref().and_then(|alias| font_names.get(alias).cloned()).or_else(|| current_font.clone()) });
                    }
                }
                // ' (string) — next line + show
                "'" => {
                    ty -= leading;
                    if let Some(t) = op.operands.first().and_then(|o| decode_show(o, &current_font)) {
                        let (px, py) = apply_ctm(tx, ty, ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f);
                        texts.push(PositionedText { text: t, x: px, y: py, page: 0, font_size: current_font_size, font_name: current_font.as_ref().and_then(|alias| font_names.get(alias).cloned()).or_else(|| current_font.clone()) });
                    }
                }
                // " aw ac (string) — next line + show with spacing overrides
                "\"" => {
                    ty -= leading;
                    if let Some(t) = op.operands.get(2).and_then(|o| decode_show(o, &current_font)) {
                        let (px, py) = apply_ctm(tx, ty, ctm_a, ctm_b, ctm_c, ctm_d, ctm_e, ctm_f);
                        texts.push(PositionedText { text: t, x: px, y: py, page: 0, font_size: current_font_size, font_name: current_font.as_ref().and_then(|alias| font_names.get(alias).cloned()).or_else(|| current_font.clone()) });
                    }
                }
                _ => {}
            }
        }

        texts
    }

    /// Extract PDF version from header
    fn extract_version(&self) -> String {
        if let Some(newline_pos) = self.data.iter().position(|&b| b == b'\n' || b == b'\r') {
            if let Ok(header) = String::from_utf8(self.data[0..newline_pos].to_vec()) {
                return header.replace("%PDF-", "").trim().to_string();
            }
        }
        "Unknown".to_string()
    }

    /// Get page count using lopdf
    fn get_page_count(&self) -> Option<usize> {
        let doc = lopdf::Document::load_mem(&self.data).ok()?;
        Some(doc.get_pages().len())
    }

    /// Split extracted text into pages
    fn split_into_pages(&self, text: &str, page_count: usize) -> Vec<PageContent> {
        // Try to split by form feed character first
        let page_splits: Vec<&str> = text.split('\x0C').collect();
        
        if page_splits.len() > 1 {
            // Form feed split worked
            page_splits.iter()
                .enumerate()
                .map(|(i, content)| PageContent {
                    page_number: i + 1,
                    text: content.trim().to_string(),
                })
                .filter(|p| !p.text.is_empty())
                .collect()
        } else if page_count > 1 {
            // Try to split by approximate line count
            let lines: Vec<&str> = text.lines().collect();
            let lines_per_page = (lines.len() / page_count).max(1);
            
            lines.chunks(lines_per_page)
                .enumerate()
                .map(|(i, chunk)| PageContent {
                    page_number: i + 1,
                    text: chunk.join("\n").trim().to_string(),
                })
                .filter(|p| !p.text.is_empty())
                .collect()
        } else {
            // Single page
            vec![PageContent {
                page_number: 1,
                text: text.trim().to_string(),
            }]
        }
    }

    /// Extract metadata using lopdf
    fn extract_metadata(&self) -> PdfMetadata {
        let mut metadata = PdfMetadata::default();
        
        if let Ok(doc) = lopdf::Document::load_mem(&self.data) {
            if let Ok(info) = doc.trailer.get(b"Info") {
                if let Ok(info_ref) = info.as_reference() {
                    if let Ok(info_dict) = doc.get_dictionary(info_ref) {
                        metadata.title = get_pdf_string(&doc, info_dict, b"Title");
                        metadata.author = get_pdf_string(&doc, info_dict, b"Author");
                        metadata.subject = get_pdf_string(&doc, info_dict, b"Subject");
                        metadata.creator = get_pdf_string(&doc, info_dict, b"Creator");
                        metadata.producer = get_pdf_string(&doc, info_dict, b"Producer");
                    }
                }
            }
        }
        
        metadata
    }
}

/// Helper to get string from PDF dictionary
fn get_pdf_string(_doc: &lopdf::Document, dict: &lopdf::Dictionary, key: &[u8]) -> String {
    if let Ok(obj) = dict.get(key) {
        match obj {
            lopdf::Object::String(bytes, _) => {
                // Try UTF-8 first, then Latin-1
                String::from_utf8(bytes.clone())
                    .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect())
            }
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

/// Decompress FlateDecode (zlib) data with a hard output ceiling
/// (`MAX_PDF_STREAM` = 128 MB). Guards against PDF decompression bombs.
fn decompress_flate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    read_limited(&mut decoder, MAX_PDF_STREAM)
}

/// Extract numeric value from PDF object (handles both Integer and Real types)
fn extract_number(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Detect font style (bold/italic) from font name
fn detect_font_style(font_name: &str) -> FontStyle {
    let name_lower = font_name.to_lowercase();

    // Common bold indicators in font names
    let is_bold = name_lower.contains("bold")
        || name_lower.contains("-bd")
        || name_lower.contains("_bd")
        || name_lower.contains("-b,")
        || name_lower.ends_with("-b")
        || name_lower.contains("black")
        || name_lower.contains("heavy")
        || name_lower.contains("semibold")
        || name_lower.contains("demibold")
        || name_lower.contains("extrabold")
        || name_lower.contains("ultrabold");

    // Common italic/oblique indicators in font names
    let is_italic = name_lower.contains("italic")
        || name_lower.contains("oblique")
        || name_lower.contains("-it")
        || name_lower.contains("_it")
        || name_lower.contains("-i,")
        || name_lower.ends_with("-i")
        || name_lower.contains("slanted");

    FontStyle {
        is_bold,
        is_italic,
    }
}

/// Extract text from Tj operator (simple string)
fn extract_pdf_string(line: &str) -> Option<String> {
    // Format: (text) Tj or <hex> Tj
    let line = line.trim();

    if line.starts_with('(') {
        // Literal string
        if let Some(end) = line.rfind(") Tj") {
            let content = &line[1..end];
            // Handle escape sequences
            return Some(unescape_pdf_string(content));
        }
    } else if line.starts_with('<') {
        // Hex string
        if let Some(end) = line.rfind("> Tj") {
            let hex = &line[1..end];
            return decode_hex_string(hex);
        }
    }

    None
}

/// Extract text from TJ operator (text array)
fn extract_pdf_text_array(line: &str) -> Option<String> {
    // Format: [(text) -kern (text2)] TJ
    let line = line.trim();

    if !line.starts_with('[') {
        return None;
    }

    let mut result = String::new();
    let mut in_string = false;
    let mut current_string = String::new();
    let mut is_hex = false;

    for ch in line.chars() {
        if !in_string {
            if ch == '(' {
                in_string = true;
                is_hex = false;
                current_string.clear();
            } else if ch == '<' {
                in_string = true;
                is_hex = true;
                current_string.clear();
            }
        } else {
            if !is_hex && ch == ')' {
                result.push_str(&unescape_pdf_string(&current_string));
                in_string = false;
            } else if is_hex && ch == '>' {
                if let Some(decoded) = decode_hex_string(&current_string) {
                    result.push_str(&decoded);
                }
                in_string = false;
            } else if !is_hex && ch == '\\' {
                // Handle escape - simplified
                current_string.push(ch);
            } else {
                current_string.push(ch);
            }
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Unescape PDF literal string escape sequences
fn unescape_pdf_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('(') => result.push('('),
                Some(')') => result.push(')'),
                Some('\\') => result.push('\\'),
                Some(c) if c.is_ascii_digit() => {
                    // Octal escape
                    let mut octal = String::new();
                    octal.push(c);
                    for _ in 0..2 {
                        if let Some(&next) = chars.peek() {
                            if next.is_ascii_digit() {
                                octal.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                    if let Ok(val) = u8::from_str_radix(&octal, 8) {
                        result.push(val as char);
                    }
                }
                Some(c) => result.push(c),
                None => {}
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Decode hex string from PDF
fn decode_hex_string(hex: &str) -> Option<String> {
    let hex = hex.replace(' ', "");
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            let end = (i + 2).min(hex.len());
            u8::from_str_radix(&hex[i..end], 16).ok()
        })
        .collect();

    Some(String::from_utf8_lossy(&bytes).to_string())
}

// ──────────────────────────────────────────────────────────────────────────────
// ToUnicode CMap support
// ──────────────────────────────────────────────────────────────────────────────
//
// Korean / CJK corporate PDFs almost always embed Type0 fonts with CIDFont
// descendants, where Tj/TJ operand bytes are 2-byte CIDs (not UTF-8). The
// font dict carries a `/ToUnicode` stream: a PostScript-like CMap that maps
// CID byte sequences to Unicode codepoints. Without applying it, our text
// extractor emits garbage for these PDFs and the table detector sees nothing.
//
// We only implement the subset of the CMap spec used by ~99% of real fonts:
// `beginbfchar`/`endbfchar` and `beginbfrange`/`endbfrange`. Multi-char targets
// (`<0041 0042>` on the RHS) are supported. `beginrearrangedrange` and PS-level
// logic are not.

/// Parsed ToUnicode CMap for one font.
#[derive(Debug, Default, Clone)]
pub struct ToUnicodeCMap {
    /// Exact-match mappings: src byte sequence → UTF-8 string
    bfchar: std::collections::HashMap<Vec<u8>, String>,
    /// Range mappings: (start_bytes, end_bytes, base_codepoint).
    /// CID X in [start, end] maps to base + (X - start) as a single char.
    bfrange: Vec<(Vec<u8>, Vec<u8>, u32)>,
    /// Range mappings with per-entry array: (start_bytes, Vec<target>).
    /// Covers `<0000> <0002> [<0041> <0042> <0043>]` form.
    bfrange_array: Vec<(Vec<u8>, Vec<String>)>,
}

impl ToUnicodeCMap {
    /// Decode a byte sequence using this CMap. Tries the longest match first
    /// (4-byte, then 2-byte, then 1-byte) to handle variable-width codes.
    pub fn decode(&self, bytes: &[u8]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            // Try 2-byte first (dominant CID width), then 1-byte fallback
            let mut consumed = 0;
            for width in [2usize, 1] {
                if i + width > bytes.len() { continue; }
                let seg = &bytes[i..i + width];
                if let Some(s) = self.lookup(seg) {
                    out.push_str(&s);
                    consumed = width;
                    break;
                }
            }
            if consumed == 0 {
                // Unknown CID — skip one byte to avoid infinite loop.
                // We deliberately do NOT push the raw byte, because that
                // would reintroduce garbage into the output.
                i += 1;
            } else {
                i += consumed;
            }
        }
        out
    }

    fn lookup(&self, seg: &[u8]) -> Option<String> {
        if let Some(s) = self.bfchar.get(seg) {
            return Some(s.clone());
        }
        let cid = bytes_to_cid(seg);
        for (start, end, base) in &self.bfrange {
            if seg.len() == start.len()
                && seg >= start.as_slice()
                && seg <= end.as_slice()
            {
                let offset = cid - bytes_to_cid(start);
                if let Some(ch) = char::from_u32(base + offset) {
                    return Some(ch.to_string());
                }
            }
        }
        for (start, arr) in &self.bfrange_array {
            if seg.len() == start.len() && seg >= start.as_slice() {
                let offset = (cid - bytes_to_cid(start)) as usize;
                if offset < arr.len() {
                    return Some(arr[offset].clone());
                }
            }
        }
        None
    }
}

fn bytes_to_cid(bytes: &[u8]) -> u32 {
    let mut v = 0u32;
    for b in bytes {
        v = (v << 8) | (*b as u32);
    }
    v
}

/// Decode a hex string like "0041" / "00 41" / "0041 0042" to bytes.
fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    let cleaned: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() % 2 != 0 || cleaned.is_empty() { return None; }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    for i in (0..cleaned.len()).step_by(2) {
        out.push(u8::from_str_radix(&cleaned[i..i + 2], 16).ok()?);
    }
    Some(out)
}

/// Decode a hex string as a UTF-16BE sequence → UTF-8 string.
/// PDF ToUnicode targets are always UTF-16BE per the spec.
fn hex_to_utf16_string(s: &str) -> Option<String> {
    let bytes = hex_to_bytes(s)?;
    if bytes.is_empty() || bytes.len() % 2 != 0 { return None; }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| ((c[0] as u16) << 8) | (c[1] as u16))
        .collect();
    String::from_utf16(&units).ok()
}

/// Tokenize a CMap stream by stripping comments and splitting on whitespace,
/// keeping `<...>` hex literals and `[...]` array brackets intact.
fn tokenize_cmap(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                // comment until newline
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc == '\n' { break; }
                }
            }
            '<' => {
                let mut buf = String::from("<");
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    buf.push(nc);
                    if nc == '>' { break; }
                }
                tokens.push(buf);
            }
            '[' => tokens.push("[".to_string()),
            ']' => tokens.push("]".to_string()),
            c if c.is_whitespace() => {}
            c => {
                let mut buf = String::from(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_whitespace() || nc == '<' || nc == '[' || nc == ']' { break; }
                    chars.next();
                    buf.push(nc);
                }
                tokens.push(buf);
            }
        }
    }
    tokens
}

/// Parse a CMap stream (`/ToUnicode` content) into a ToUnicodeCMap.
pub fn parse_tounicode_cmap(stream_bytes: &[u8]) -> ToUnicodeCMap {
    let text = String::from_utf8_lossy(stream_bytes);
    let tokens = tokenize_cmap(&text);
    let mut cmap = ToUnicodeCMap::default();

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "beginbfchar" => {
                i += 1;
                while i < tokens.len() && tokens[i] != "endbfchar" {
                    // <src> <dst>
                    if i + 1 < tokens.len()
                        && tokens[i].starts_with('<')
                        && tokens[i + 1].starts_with('<')
                    {
                        let src = tokens[i].trim_matches(|c| c == '<' || c == '>');
                        let dst = tokens[i + 1].trim_matches(|c| c == '<' || c == '>');
                        if let (Some(sb), Some(ds)) =
                            (hex_to_bytes(src), hex_to_utf16_string(dst))
                        {
                            cmap.bfchar.insert(sb, ds);
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            "beginbfrange" => {
                i += 1;
                while i < tokens.len() && tokens[i] != "endbfrange" {
                    // <start> <end> <base>  OR  <start> <end> [ <t0> <t1> ... ]
                    if i + 2 < tokens.len()
                        && tokens[i].starts_with('<')
                        && tokens[i + 1].starts_with('<')
                    {
                        let start_hex = tokens[i].trim_matches(|c| c == '<' || c == '>');
                        let end_hex = tokens[i + 1].trim_matches(|c| c == '<' || c == '>');
                        let start_bytes = hex_to_bytes(start_hex);
                        let end_bytes = hex_to_bytes(end_hex);

                        if tokens[i + 2] == "[" {
                            // Array form
                            let mut j = i + 3;
                            let mut arr = Vec::new();
                            while j < tokens.len() && tokens[j] != "]" {
                                if tokens[j].starts_with('<') {
                                    let h = tokens[j].trim_matches(|c| c == '<' || c == '>');
                                    if let Some(s) = hex_to_utf16_string(h) {
                                        arr.push(s);
                                    } else {
                                        arr.push(String::new());
                                    }
                                }
                                j += 1;
                            }
                            if let Some(sb) = start_bytes {
                                cmap.bfrange_array.push((sb, arr));
                            }
                            i = j + 1;
                        } else if tokens[i + 2].starts_with('<') {
                            // Base codepoint form
                            let base_hex =
                                tokens[i + 2].trim_matches(|c| c == '<' || c == '>');
                            if let (Some(sb), Some(eb)) = (start_bytes, end_bytes) {
                                // base_hex is UTF-16BE; grab the first code unit
                                if let Some(base_bytes) = hex_to_bytes(base_hex) {
                                    let base = bytes_to_cid(&base_bytes);
                                    cmap.bfrange.push((sb, eb, base));
                                }
                            }
                            i += 3;
                        } else {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => { i += 1; }
        }
    }

    cmap
}

/// Walk a page's fonts (including inherited from `/Parent` Pages nodes) and
/// build a CMap for every font that carries a `/ToUnicode` stream. Fonts
/// without ToUnicode are silently skipped — the caller falls back to UTF-8
/// lossy decoding for those.
fn build_page_font_cmaps(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> std::collections::HashMap<String, ToUnicodeCMap> {
    let mut out = std::collections::HashMap::new();

    // `get_page_fonts` already walks the Resources chain up through parent
    // Pages nodes and resolves every font reference to its dict. Reinventing
    // that walk (as the first attempt did) silently missed PDFs that put
    // fonts on the parent Pages node instead of the individual Page dict.
    let fonts = match doc.get_page_fonts(page_id) {
        Ok(f) => f,
        Err(_) => return out,
    };

    for (name_bytes, font_dict) in fonts.iter() {
        let font_name = String::from_utf8_lossy(name_bytes).to_string();

        // Resolve /ToUnicode — always a stream (direct or by reference)
        let tu = match font_dict.get(b"ToUnicode") {
            Ok(o) => o,
            Err(_) => continue,
        };
        let stream = match tu {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(o) => o,
                Err(_) => continue,
            },
            o => o,
        };
        let stream = match stream.as_stream() {
            Ok(s) => s,
            Err(_) => continue,
        };
        // `decompressed_content` handles FlateDecode etc.
        let bytes = match stream.decompressed_content() {
            Ok(b) => b,
            Err(_) => stream.content.clone(),
        };
        let cmap = parse_tounicode_cmap(&bytes);
        if !cmap.bfchar.is_empty()
            || !cmap.bfrange.is_empty()
            || !cmap.bfrange_array.is_empty()
        {
            out.insert(font_name, cmap);
        }
    }

    out
}

/// Build a mapping from font alias (F1, F2) to actual BaseFont name (Helvetica-Bold, etc.)
fn build_page_font_names(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();

    let fonts = match doc.get_page_fonts(page_id) {
        Ok(f) => f,
        Err(_) => return out,
    };

    for (name_bytes, font_dict) in fonts.iter() {
        let alias = String::from_utf8_lossy(name_bytes).to_string();

        // Try BaseFont first, then Name
        let base_font = font_dict.get(b"BaseFont")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).to_string());

        if let Some(bf) = base_font {
            out.insert(alias, bf);
        }
    }

    out
}

/// Pull the raw byte payload out of a `Tj`/`TJ`/`'`/`"` operand.
/// For TJ arrays, concatenates every string element (skipping kerning nums).
fn collect_show_bytes(obj: &lopdf::Object) -> Option<Vec<u8>> {
    match obj {
        lopdf::Object::String(bytes, _) => Some(bytes.clone()),
        lopdf::Object::Array(arr) => {
            let mut out = Vec::new();
            for el in arr {
                if let lopdf::Object::String(bytes, _) = el {
                    out.extend_from_slice(bytes);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

/// Detect multiple table structures on a single page from positioned text.
///
/// Phase-1 improvement over the old single-table heuristic:
///   1. Segments the page into contiguous row regions by Y-gap (median × 2.5)
///      so a page with 3 stacked tables is no longer forced into one group.
///   2. Picks each region's "mode column count" from the row length distribution
///      and projects every row onto the mode row's X anchors — tolerates merged
///      cells and short rows instead of filter-dropping them.
///   3. Derives X tolerance dynamically from the minimum column spacing of the
///      anchor row instead of a hard-coded 20pt (robust across font sizes).
///   4. Accepts any region with ≥2 rows × ≥2 mode columns — no 50%-alignment gate.

/// Approximate right-edge X coordinate of a positioned text run.
///
/// `PositionedText` only stores the start `x`, not the run width, so we
/// estimate the right edge as `x + char_count * 5pt`. The 5pt/char figure
/// is a reasonable default for 10pt body fonts and is only used for the
/// multi-column layout heuristic — false positives here just mean we
/// process a single-column page as single-column.
fn approx_right_edge(t: &PositionedText) -> f64 {
    const PT_PER_CHAR: f64 = 5.0;
    t.x + (t.text.chars().count() as f64) * PT_PER_CHAR
}

/// Detect a 2-column page layout from positioned text and return the split X.
///
/// Algorithm: bin run start-X coordinates into a histogram, find the two
/// tallest bins, and require them to (a) sit in the left/right halves of
/// the content span, (b) be separated by an empty trough, and (c) each
/// anchor at least `min_runs_per_side` runs.
///
/// This replaces an earlier "right-edge gap" heuristic that failed on
/// short pages (<30 runs) and on dense columns where the approximate
/// right edge of each run overshot the true column boundary. The
/// histogram version only uses each run's start-X — a robust signal
/// regardless of per-run text length.
pub(crate) fn detect_column_split(texts: &[PositionedText]) -> Option<f64> {
    const MIN_TOTAL_RUNS: usize = 10;
    const MIN_RUNS_PER_SIDE: usize = 3;
    const BIN_WIDTH_PT: f64 = 10.0;

    if texts.len() < MIN_TOTAL_RUNS {
        return None;
    }

    let min_x = texts.iter().map(|t| t.x).fold(f64::INFINITY, f64::min);
    let max_x = texts.iter().map(|t| t.x).fold(f64::NEG_INFINITY, f64::max);
    let span = max_x - min_x;
    if span < 150.0 {
        return None;
    }

    // Histogram over start-X.
    let bin_count = (span / BIN_WIDTH_PT).ceil() as usize + 1;
    let mut hist = vec![0usize; bin_count];
    for t in texts {
        let b = ((t.x - min_x) / BIN_WIDTH_PT) as usize;
        if b < bin_count {
            hist[b] += 1;
        }
    }

    // Find the two most populated bins.
    let (i1, c1) = hist
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .map(|(i, c)| (i, *c))?;
    let (i2, c2) = hist
        .iter()
        .enumerate()
        .filter(|(i, _)| i.abs_diff(i1) >= 3) // must be separated
        .max_by_key(|(_, c)| **c)
        .map(|(i, c)| (i, *c))?;

    if c1 < MIN_RUNS_PER_SIDE || c2 < MIN_RUNS_PER_SIDE {
        return None;
    }

    // Order bins: left = smaller index, right = larger.
    let (left_bin, right_bin, _left_count, _right_count) = if i1 < i2 {
        (i1, i2, c1, c2)
    } else {
        (i2, i1, c2, c1)
    };
    let left_x = min_x + (left_bin as f64) * BIN_WIDTH_PT;
    let right_x = min_x + (right_bin as f64) * BIN_WIDTH_PT;

    // Left peak must sit in the left half, right peak in the right half.
    let center = min_x + span / 2.0;
    if left_x > center - 20.0 || right_x < center + 20.0 {
        return None;
    }

    // Trough check: the narrow band around the geometric gutter (middle
    // 40% of the span) should be nearly empty. Checking the full range
    // between peaks would reject pages with titles or centered headings
    // that happen to start at x-positions between the columns' anchors
    // but are actually full-width content, not column-gutter content.
    let gutter_lo = min_x + span * 0.3;
    let gutter_hi = min_x + span * 0.7;
    let lo_bin = ((gutter_lo - min_x) / BIN_WIDTH_PT).max(0.0) as usize;
    let hi_bin = (((gutter_hi - min_x) / BIN_WIDTH_PT) as usize).min(bin_count);
    let trough_max = ((c1.min(c2) as f64) * 0.25).round().max(1.0) as usize;
    for b in lo_bin..hi_bin {
        if hist[b] > trough_max {
            return None;
        }
    }

    // Count runs whose start-X anchors are on each side, using the midpoint
    // between the two peak bins as the divider. Runs well outside both
    // peaks (e.g. centered full-width headings) are excluded from the
    // per-side balance check to avoid counting them against the layout.
    let split = (left_x + right_x) / 2.0;
    let anchor_tol = (right_x - left_x) * 0.35; // allow some drift per column
    let left_runs = texts
        .iter()
        .filter(|t| (t.x - left_x).abs() <= anchor_tol)
        .count();
    let right_runs = texts
        .iter()
        .filter(|t| (t.x - right_x).abs() <= anchor_tol)
        .count();
    if left_runs < MIN_RUNS_PER_SIDE || right_runs < MIN_RUNS_PER_SIDE {
        return None;
    }
    // Balance: the lighter column must have ≥ 35% of the heavier.
    let (lo, hi) = if left_runs < right_runs {
        (left_runs, right_runs)
    } else {
        (right_runs, left_runs)
    };
    if (lo as f64) / (hi as f64) < 0.35 {
        return None;
    }

    Some(split)
}

fn detect_tables_from_positions(texts: &[PositionedText], page: usize) -> Vec<PdfTable> {
    if texts.len() < 4 {
        return vec![]; // Need at least 2x2 cells
    }

    // ── Step 1: group texts into rows by Y proximity ──────────────────────
    const Y_TOLERANCE: f64 = 5.0;
    let mut rows: Vec<Vec<&PositionedText>> = Vec::new();
    for text in texts {
        let mut found_row = false;
        for row in &mut rows {
            if let Some(first) = row.first() {
                if (first.y - text.y).abs() < Y_TOLERANCE {
                    row.push(text);
                    found_row = true;
                    break;
                }
            }
        }
        if !found_row {
            rows.push(vec![text]);
        }
    }

    // Sort rows top→bottom (PDF Y axis grows upward)
    rows.sort_by(|a, b| {
        let y_a = a.first().map(|t| t.y).unwrap_or(0.0);
        let y_b = b.first().map(|t| t.y).unwrap_or(0.0);
        y_b.partial_cmp(&y_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    for row in &mut rows {
        row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }

    if rows.len() < 2 {
        return vec![];
    }

    // ── Step 2: segment rows into contiguous regions by Y-gap ─────────────
    let gaps: Vec<f64> = rows
        .windows(2)
        .map(|w| {
            let y0 = w[0].first().map(|t| t.y).unwrap_or(0.0);
            let y1 = w[1].first().map(|t| t.y).unwrap_or(0.0);
            (y0 - y1).abs()
        })
        .collect();

    let median_gap = {
        let mut g = gaps.clone();
        g.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if g.is_empty() { Y_TOLERANCE } else { g[g.len() / 2] }
    };
    let gap_threshold = (median_gap * 2.5_f64).max(15.0);

    let mut segments: Vec<Vec<&Vec<&PositionedText>>> = Vec::new();
    let mut current: Vec<&Vec<&PositionedText>> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 && gaps[i - 1] > gap_threshold {
            if current.len() >= 2 {
                segments.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
        current.push(row);
    }
    if current.len() >= 2 {
        segments.push(current);
    }

    // ── Step 3: detect a table per segment ────────────────────────────────
    let mut out = Vec::new();
    for seg in &segments {
        // Need at least 2 rows carrying 2+ texts
        let multi_col_rows: Vec<&&Vec<&PositionedText>> =
            seg.iter().filter(|r| r.len() >= 2).collect();
        if multi_col_rows.len() < 2 {
            continue;
        }

        // Mode column count (dominant row width)
        let mut len_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for r in &multi_col_rows {
            *len_counts.entry(r.len()).or_insert(0) += 1;
        }
        let mode_cols = len_counts
            .iter()
            .max_by_key(|(len, count)| (*count, *len))
            .map(|(len, _)| *len)
            .unwrap_or(0);
        if mode_cols < 2 {
            continue;
        }

        // Require mode to represent a non-trivial share of the segment.
        // Without this gate, text-heavy legal / guide PDFs fabricate dozens
        // of "tables" out of numbered-list alignment. Floor at 3 hits OR
        // 25% of multi-column rows (whichever is stricter in sparse segments).
        let mode_hits = *len_counts.get(&mode_cols).unwrap_or(&0);
        if mode_hits < 3 || mode_hits * 5 < multi_col_rows.len() {
            continue;
        }

        // Anchor column X positions from the first mode-width row
        let anchor_row = match multi_col_rows.iter().find(|r| r.len() == mode_cols) {
            Some(r) => *r,
            None => continue,
        };
        let anchor_xs: Vec<f64> = anchor_row.iter().map(|t| t.x).collect();

        // Dynamic X tolerance = 0.45 × min inter-column gap, clamped [10, 40]
        let min_col_spacing = anchor_xs
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(f64::INFINITY, f64::min);
        let x_tol = if min_col_spacing.is_finite() {
            (min_col_spacing * 0.45).clamp(10.0, 40.0)
        } else {
            20.0
        };

        // Project every row onto anchor columns (merged cells absorbed into
        // the nearest anchor). Track Y positions of rows that end up in
        // the table so the renderer can map the table back to its page
        // region precisely (no leaking into surrounding prose).
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut row_ys: Vec<f64> = Vec::new();
        for r in seg.iter() {
            let mut cells: Vec<String> = vec![String::new(); mode_cols];
            for t in r.iter() {
                let mut best = 0usize;
                let mut best_d = f64::INFINITY;
                for (i, ax) in anchor_xs.iter().enumerate() {
                    let d = (t.x - ax).abs();
                    if d < best_d {
                        best_d = d;
                        best = i;
                    }
                }
                // Accept text if it snaps to an anchor within 2× tolerance
                if best_d <= x_tol * 2.0 {
                    if !cells[best].is_empty() {
                        cells[best].push(' ');
                    }
                    cells[best].push_str(&t.text);
                }
            }
            // Keep row only if ≥2 non-empty cells (filter single-line headers/noise)
            if cells.iter().filter(|c| !c.is_empty()).count() >= 2 {
                let row_y = r.first().map(|t| t.y).unwrap_or(0.0);
                table_rows.push(cells);
                row_ys.push(row_y);
            }
        }

        // Require at least 3 projected rows — a 2-row "table" is almost
        // always a header + single line of body text that the heuristic
        // grabbed from a bullet list.
        if table_rows.len() < 3 {
            continue;
        }

        // Prose-vs-table discriminator: real table cells are typically short
        // (labels, numbers, keywords). If the median non-empty cell is long,
        // we're almost certainly looking at paragraph text that happens to
        // align on column boundaries (common in legal/guide PDFs and any
        // multi-column page layout).
        let mut cell_lens: Vec<usize> = table_rows
            .iter()
            .flat_map(|r| r.iter().filter(|c| !c.is_empty()).map(|c| c.chars().count()))
            .collect();
        if cell_lens.len() >= 4 {
            cell_lens.sort_unstable();
            let median = cell_lens[cell_lens.len() / 2];
            // Tightened from 100 → 50. Real-world tables hold labels, numbers,
            // or short phrases; a median cell >50 chars across the table is a
            // strong signal that we're seeing prose aligned on a column grid.
            if median > 50 {
                continue;
            }
        }

        // Column-width discriminator: genuine tables use narrow columns.
        // If column anchors are spaced >180pt apart (wider than most table
        // cells on letter-size pages), it's almost certainly a page-level
        // 2-column layout being mistaken for a table.
        if anchor_xs.len() >= 2 {
            let max_col_spacing = anchor_xs
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0_f64, f64::max);
            if max_col_spacing > 180.0 {
                continue;
            }
        }

        let (y_top, y_bottom) = row_ys
            .iter()
            .fold((f64::NEG_INFINITY, f64::INFINITY), |(top, bot), y| {
                (top.max(*y), bot.min(*y))
            });

        out.push(PdfTable {
            page,
            rows: table_rows,
            column_count: mode_cols,
            y_top,
            y_bottom,
        });
    }

    out
}

impl ImageFormat {
    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Raw => "raw",
        }
    }
}

impl PdfImage {
    /// Get suggested filename for this image
    pub fn filename(&self) -> String {
        format!("{}.{}", self.id, self.format.extension())
    }
}

impl PdfTable {
    /// Convert table to Markdown format
    pub fn to_markdown(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let mut md = String::new();

        // Header row
        if let Some(header) = self.rows.first() {
            md.push_str("| ");
            md.push_str(&header.join(" | "));
            md.push_str(" |\n");

            // Separator row
            md.push_str("|");
            for _ in 0..self.column_count {
                md.push_str(" --- |");
            }
            md.push('\n');
        }

        // Data rows
        for row in self.rows.iter().skip(1) {
            md.push_str("| ");
            md.push_str(&row.join(" | "));
            md.push_str(" |\n");
        }

        md
    }
}

/// Check if content looks like a list item (bullet or numbered).
fn is_list_item(content: &str) -> bool {
    let trimmed = content.trim_start();
    if trimmed.starts_with("• ")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("– ")
        || trimmed.starts_with("— ")
    {
        return true;
    }
    // Numbered list: e.g. "1." or "1)" or "12. "
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if bytes[0].is_ascii_digit() {
        for (i, &b) in bytes.iter().enumerate().skip(1) {
            if b == b'.' || b == b')' {
                // Must be followed by space or be end of string
                return i + 1 >= bytes.len() || bytes[i + 1] == b' ';
            }
            // Bare digit followed by a space and a letter is also treated
            // as a numbered list item. This handles PDFs where the
            // trailing "." was drawn as a separate graphic rather than a
            // glyph (common in Office-style numbered lists) and so the
            // extracted text is just "1 First item" instead of "1. First".
            if b == b' ' && i >= 1 {
                // Require the first non-digit char after the space to be
                // alphabetic to avoid treating things like "2024 year" as
                // a list. Also cap digit count ≤ 3 so years don't qualify.
                if i > 3 {
                    return false;
                }
                let rest = &bytes[i + 1..];
                return rest.first().map_or(false, |c| c.is_ascii_alphabetic());
            }
            if !b.is_ascii_digit() {
                return false;
            }
        }
    }
    false
}

/// Normalize a list item to standard markdown bullet or numbered format.
fn normalize_list_item(content: &str) -> String {
    let trimmed = content.trim_start();
    // Bullet markers → "- "
    if let Some(rest) = trimmed.strip_prefix("• ")
        .or_else(|| trimmed.strip_prefix("– "))
        .or_else(|| trimmed.strip_prefix("— "))
    {
        return format!("- {}", rest);
    }
    // Already standard bullet
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return trimmed.to_string();
    }
    // Numbered list with just a space: "1 First item" → "1. First item".
    // Same digit-cap heuristic as is_list_item so years aren't touched.
    if let Some(space_pos) = trimmed.find(' ') {
        let prefix = &trimmed[..space_pos];
        if !prefix.is_empty()
            && prefix.len() <= 3
            && prefix.bytes().all(|b| b.is_ascii_digit())
        {
            let rest = trimmed[space_pos + 1..].trim_start();
            if rest.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
                return format!("{}. {}", prefix, rest);
            }
        }
    }
    // Numbered list: keep as-is but normalize "1) " → "1. "
    if let Some(paren_pos) = trimmed.find(')') {
        let prefix = &trimmed[..paren_pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) && !prefix.is_empty() {
            let rest = &trimmed[paren_pos + 1..].trim_start();
            return format!("{}. {}", prefix, rest);
        }
    }
    trimmed.to_string()
}

/// Collapse punctuation-adjacent whitespace that positioned-text joining
/// introduces for Korean / CJK documents. PDF text extractors emit each
/// glyph run as a separate `Tj` and the block-joiner puts a space between
/// them, so "(온라인)" becomes "( 온라인 )" and "4.12." becomes "4. 12.".
///
/// Rules applied in order:
///   1. No space inside paired brackets/parens: `( x )` → `(x)`
///   2. No space before closing punctuation: `x .` → `x.`
///   3. No space after opening punctuation: `. x` stays (needs context)
///      — handled via rule 1 for brackets; other leading punct kept as-is
///   4. Collapse runs of 2+ internal spaces to one
fn normalize_cjk_spacing(s: &str) -> String {
    // Fast path: pure-ASCII prose is unchanged. Korean / punctuation
    // cleanup only matters when there's some CJK or bracket noise.
    let needs_work = s.chars().any(|c| {
        let cp = c as u32;
        // CJK Unified Ideographs, Hangul, Hiragana, Katakana, full-width brackets
        (0x3000..=0x303F).contains(&cp)
            || (0x3040..=0x30FF).contains(&cp)
            || (0x4E00..=0x9FFF).contains(&cp)
            || (0xAC00..=0xD7AF).contains(&cp)
            || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '「' | '」' | '『' | '』')
    });
    if !needs_work {
        return s.to_string();
    }

    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let is_open = |c: char| matches!(c, '(' | '[' | '{' | '「' | '『' | '【' | '〔' | '（' | '［');
    let is_close = |c: char| matches!(c, ')' | ']' | '}' | '」' | '』' | '】' | '〕' | '）' | '］');
    let is_trailing_punct =
        |c: char| matches!(c, ',' | '.' | ';' | ':' | '?' | '!' | '、' | '。' | '，' | '．');

    for i in 0..chars.len() {
        let c = chars[i];
        if c == ' ' {
            // Skip space directly after an opener
            if out.chars().last().map_or(false, |p| is_open(p)) {
                continue;
            }
            // Skip space directly before a closer or trailing punctuation
            if let Some(&next) = chars.get(i + 1) {
                if is_close(next) || is_trailing_punct(next) {
                    continue;
                }
            }
            // Collapse consecutive spaces
            if out.ends_with(' ') {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Korean / CJK outline markers that begin a fresh bullet or paragraph.
/// Common in government documents and technical reports: `□` (L1),
/// `○` (L2), `●` (L3 emphasized), `▪`/`■`/`·` for subordinate bullets.
/// ASCII `-` is already recognized elsewhere via `is_list_item`.
fn starts_new_cjk_item(c: char) -> bool {
    matches!(
        c,
        '□' | '○' | '●' | '▪' | '■' | '▫' | '◦' | '◎' | '·'
          | '⦁' | '☐' | '☑' | '❑' | '❏' | '❒' | '➢'
    )
}

/// Apply bold/italic markdown formatting to inline text.
/// Does not wrap if the text is being used as a heading.
fn apply_inline_formatting(content: &str, is_bold: bool, is_italic: bool) -> String {
    match (is_bold, is_italic) {
        (true, true) => format!("***{}***", content),
        (true, false) => format!("**{}**", content),
        (false, true) => format!("*{}*", content),
        (false, false) => content.to_string(),
    }
}

impl PdfDocument {
    /// Convert layout elements to markdown with heading detection, bold/italic formatting,
    /// and list item normalization.
    ///
    /// Heading detection uses font size ratios relative to the median body font size:
    /// - `>= median * 1.8` AND bold -> H1
    /// - `>= median * 1.4` AND bold -> H2
    /// - `>= median * 1.15` AND bold -> H3
    /// - bold AND `> median` -> H4
    ///
    /// Header/Footer regions (top/bottom 10%) are stripped.
    pub fn to_markdown_with_layout(&self) -> String {
        if self.layout.is_empty() {
            // Fallback: no layout data, return raw page text
            return self.pages.iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        // Step 1: Compute median body font size from Text elements
        let median_font_size = self.compute_median_font_size();

        // Step 1b: Compute body-baseline X. Paragraphs sit at this X; list
        // items are indented further right. We use the minimum X among
        // Text/Heading blocks with body-ish font size — that's the left
        // margin of prose. Anything ≥ threshold pt right of it is a
        // candidate list item.
        let body_x = {
            let mut xs: Vec<f64> = self.layout.iter()
                .filter(|e| matches!(e.element_type,
                                     LayoutElementType::Text | LayoutElementType::ListItem))
                .filter(|e| e.font_size.map_or(true, |s| (s - median_font_size).abs() < 2.0))
                .map(|e| e.x)
                .collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if xs.is_empty() { 0.0 } else { xs[0] }
        };
        const LIST_INDENT_MIN_PT: f64 = 12.0;

        // Pre-pass: detect runs of ≥3 consecutive indented blocks with uniform
        // X, stable font size, and short content → a bullet list the PDF
        // drew without an explicit bullet glyph (common in style-sheet-
        // generated test PDFs). We require:
        //   - run length ≥ 3 (so genuine prose paragraphs don't get listed)
        //   - each item ≤ 120 chars (genuine list items are short)
        //   - consistent Y-spacing within the run (< 2.5 × font_size)
        //   - no enormous prose block on the same indent (protects column
        //     layouts where columns get flagged as "indented prose")
        let mut list_indices: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        const LIST_ITEM_MAX_CHARS: usize = 120;
        {
            // Walk in the layout order the renderer will visit. Group into
            // candidate runs then post-filter by content-length + spacing.
            let mut i = 0;
            while i < self.layout.len() {
                let start = i;
                let anchor = &self.layout[i];
                if !matches!(anchor.element_type,
                    LayoutElementType::Text | LayoutElementType::ListItem)
                    || anchor.x < body_x + LIST_INDENT_MIN_PT
                    || anchor.content.trim().is_empty()
                {
                    i += 1;
                    continue;
                }
                let mut run_end = i + 1;
                while run_end < self.layout.len() {
                    let e = &self.layout[run_end];
                    let matches = matches!(e.element_type,
                        LayoutElementType::Text | LayoutElementType::ListItem)
                        && (e.x - anchor.x).abs() < 2.0
                        && e.font_size == anchor.font_size
                        && e.page == anchor.page;
                    if !matches {
                        break;
                    }
                    run_end += 1;
                }
                if run_end - start >= 3 {
                    // Validate content length + spacing uniformity. The
                    // average-length gate is the key prose-vs-list signal:
                    // bullet lists average ~10-30 chars per item; prose
                    // paragraphs broken across lines average 40+. Gates
                    // intentionally conservative to avoid rewriting
                    // multi-column body text as a list.
                    let items: Vec<&LayoutElement> = (start..run_end)
                        .map(|k| &self.layout[k]).collect();
                    let short_enough = items.iter()
                        .all(|e| e.content.chars().count() <= LIST_ITEM_MAX_CHARS);
                    let avg_len: f64 = items.iter()
                        .map(|e| e.content.chars().count() as f64)
                        .sum::<f64>() / items.len() as f64;
                    let fs = anchor.font_size.unwrap_or(median_font_size).max(1.0);
                    let spacing_ok = items.windows(2).all(|w| {
                        let dy = (w[0].y - w[1].y).abs();
                        dy > fs * 0.5 && dy < fs * 3.0
                    });
                    if short_enough && spacing_ok && avg_len <= 35.0 {
                        for k in start..run_end {
                            list_indices.insert(k);
                        }
                    }
                }
                i = run_end.max(start + 1);
            }
        }

        let mut output = String::new();
        let mut last_page: usize = 0;
        let mut last_y: f64 = f64::MAX; // Track Y position for inline segments
        // Set after emitting a heading / list item so the NEXT text block
        // starts a new paragraph even when it's on the same Y (style split).
        // Further blocks on that same Y can then be soft-joined normally.
        let mut force_new_paragraph = false;
        // Which detected tables have already been emitted inline — renderer
        // emits each table exactly once, at the Y position of its top row,
        // and suppresses the raw text blocks that live inside the region.
        let mut emitted_tables: Vec<bool> = vec![false; self.tables.len()];

        for (elem_idx, elem) in self.layout.iter().enumerate() {
            // Skip header/footer regions
            if elem.element_type == LayoutElementType::Header
                || elem.element_type == LayoutElementType::Footer
            {
                continue;
            }

            // Page break marker
            if elem.element_type == LayoutElementType::PageBreak {
                if !output.is_empty() && !output.ends_with("\n\n") {
                    output.push_str("\n\n");
                }
                continue;
            }

            // Page markers for multi-page documents
            if self.page_count > 1 && elem.page != last_page {
                if last_page > 0 && !output.ends_with("\n\n") {
                    output.push_str("\n\n");
                }
                last_page = elem.page;
            }

            // Suppress inline text blocks that fall inside any detected
            // table's Y-region — they'd be a flat duplicate of the table
            // rendered later. Also emit the table markdown once when we
            // first cross into its region.
            let mut covered_by_table: Option<usize> = None;
            if matches!(elem.element_type, LayoutElementType::Text | LayoutElementType::ListItem) {
                for (i, t) in self.tables.iter().enumerate() {
                    if t.page != elem.page {
                        continue;
                    }
                    // Allow a small tolerance so the first row of the table
                    // (whose Y exactly equals y_top) is counted as inside.
                    if elem.y <= t.y_top + 0.5 && elem.y >= t.y_bottom - 0.5 {
                        covered_by_table = Some(i);
                        break;
                    }
                }
            }
            if let Some(ti) = covered_by_table {
                if !emitted_tables[ti] {
                    if !output.is_empty() && !output.ends_with("\n\n") {
                        output.push_str("\n\n");
                    }
                    output.push_str(&self.tables[ti].to_markdown());
                    output.push_str("\n\n");
                    emitted_tables[ti] = true;
                    force_new_paragraph = true;
                    last_y = self.tables[ti].y_bottom;
                }
                continue;
            }

            // Image elements
            if elem.element_type == LayoutElementType::Image {
                if let Some(ref id) = elem.ref_id {
                    output.push_str(&format!("![{}]({})\n\n", id, id));
                }
                continue;
            }

            let normalized = normalize_cjk_spacing(elem.content.trim());
            let content = normalized.as_str();
            if content.is_empty() {
                continue;
            }

            let font_size = elem.font_size.unwrap_or(median_font_size);

            // Heading classification based on font size ratio to median.
            // In PDF, font names are often aliases (F1, F2) so we rely primarily
            // on size. Bold is a bonus signal but not required.
            let heading_level = if median_font_size > 0.0 {
                let ratio = font_size / median_font_size;
                if ratio >= 1.8 {
                    Some(1) // H1: title-size text (e.g. 24pt vs 11pt body)
                } else if ratio >= 1.4 {
                    Some(2) // H2: chapter-size (e.g. 18pt)
                } else if ratio >= 1.15 {
                    Some(3) // H3: section-size (e.g. 14pt)
                } else if ratio > 1.02 && elem.is_bold {
                    Some(4) // H4: slightly larger + bold
                } else if ratio >= 0.98 && elem.is_bold && content.len() < 80
                    && content.len() > 3
                    && !content.starts_with(',')
                    && !content.starts_with('.')
                    && !content.ends_with(',')
                    && content.chars().next().map_or(false, |c| c.is_uppercase() || !c.is_ascii())
                {
                    // Bold text at ~body size: heading if starts with uppercase
                    // and doesn't look like a mid-sentence fragment
                    Some(4)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(level) = heading_level {
                // Ensure paragraph break before heading
                if !output.is_empty() && !output.ends_with("\n\n") {
                    output.push_str("\n\n");
                }
                let prefix = "#".repeat(level);
                output.push_str(&format!("{} {}\n\n", prefix, content));
                force_new_paragraph = true;
                last_y = elem.y;
                continue;
            } else if elem.element_type == LayoutElementType::ListItem
                || is_list_item(content)
            {
                // Normalize list items
                let normalized = normalize_list_item(content);
                output.push_str(&format!("{}\n", normalized));
                force_new_paragraph = true;
                last_y = elem.y;
                continue;
            } else if list_indices.contains(&elem_idx) {
                // Heuristically detected list: bulletless indented block
                // that appears in a run of ≥2 siblings at the same indent.
                // Emit as "- ..." so the markdown reader renders a list.
                output.push_str(&format!("- {}\n", content));
                force_new_paragraph = true;
                last_y = elem.y;
                continue;
            } else {
                // Regular text with bold/italic formatting
                let formatted = apply_inline_formatting(content, elem.is_bold, elem.is_italic);

                // Three-tier Y-delta classification for line spacing.
                //   same_line     : two blocks stacked at the same baseline
                //                   (style split on one visual line)
                //   same_paragraph: the normal PDF line pitch (≈font_size × 1.2);
                //                   in MDM terms these are soft-wrapped lines
                //                   inside one paragraph → join with a space
                //   new_paragraph : everything above ~1.8× the font size
                let y_delta = (last_y - elem.y).abs();
                let fs = elem.font_size.unwrap_or(median_font_size).max(1.0);
                let first_block = last_y == f64::MAX;
                // Korean gov-doc bullet markers (□ ○ ● ▪ ■ ·) start new
                // outline items — never soft-join onto a preceding line
                // even when Y-delta is within the normal line pitch.
                // Without this, multi-level Korean bullet lists get
                // concatenated into one long paragraph.
                let starts_with_cjk_bullet = content
                    .chars()
                    .next()
                    .map_or(false, starts_new_cjk_item);
                // `force_new_paragraph` makes this block start a fresh
                // paragraph even if Y-wise it would have been soft-joined
                // (e.g., the text right after a heading that shares the
                // heading's baseline due to a style split).
                let same_line = !first_block && !force_new_paragraph
                    && !starts_with_cjk_bullet
                    && y_delta < fs * 0.5;
                let same_paragraph = !first_block && !force_new_paragraph
                    && !starts_with_cjk_bullet
                    && !same_line && y_delta < fs * 1.8;

                if same_line {
                    // Strip any trailing paragraph break the previous
                    // `force_new_paragraph` pass may have emitted — this
                    // block shares the baseline, so it's still part of
                    // the same visual line.
                    while output.ends_with('\n') {
                        output.pop();
                    }
                    if !output.ends_with(' ') {
                        output.push(' ');
                    }
                    output.push_str(&formatted);
                } else if same_paragraph {
                    // Soft-wrap within the same paragraph: strip trailing
                    // paragraph break if one was just emitted, join with space.
                    while output.ends_with('\n') {
                        output.pop();
                    }
                    if !output.ends_with(' ') {
                        output.push(' ');
                    }
                    output.push_str(&formatted);
                } else {
                    if !output.is_empty() && !output.ends_with("\n\n") {
                        output.push_str("\n\n");
                    }
                    output.push_str(&formatted);
                    output.push_str("\n\n");
                }
                last_y = elem.y;
                force_new_paragraph = false;
            }
        }

        output.trim_end().to_string()
    }

    /// Compute the median font size from all Text layout elements.
    /// This represents the "body" font size used as baseline for heading detection.
    fn compute_median_font_size(&self) -> f64 {
        let mut sizes: Vec<f64> = self.layout.iter()
            .filter(|e| e.element_type == LayoutElementType::Text
                || e.element_type == LayoutElementType::ListItem)
            .filter_map(|e| e.font_size)
            .filter(|&s| s > 0.0)
            .collect();

        if sizes.is_empty() {
            return 12.0; // sensible default
        }

        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sizes.len() / 2;
        if sizes.len() % 2 == 0 {
            (sizes[mid - 1] + sizes[mid]) / 2.0
        } else {
            sizes[mid]
        }
    }

    /// Convert to MDX format
    pub fn to_mdx(&self) -> String {
        let mut mdx = String::new();

        // Frontmatter
        mdx.push_str("---\n");
        mdx.push_str("format: pdf\n");
        mdx.push_str(&format!("version: \"{}\"\n", self.version));
        mdx.push_str(&format!("pages: {}\n", self.page_count));
        mdx.push_str(&format!("images: {}\n", self.images.len()));
        mdx.push_str(&format!("fonts: {}\n", self.fonts.len()));
        mdx.push_str(&format!("tables: {}\n", self.tables.len()));
        if !self.metadata.title.is_empty() {
            mdx.push_str(&format!("title: \"{}\"\n", self.metadata.title.replace('"', "\\\"")));
        }
        if !self.metadata.author.is_empty() {
            mdx.push_str(&format!("author: \"{}\"\n", self.metadata.author.replace('"', "\\\"")));
        }
        mdx.push_str("---\n\n");

        // Content: use layout-aware conversion if layout data is available
        let content = self.to_markdown_with_layout();
        let content = merge_partial_numbering(&content);
        mdx.push_str(&content);
        mdx.push_str("\n\n");

        // Image references (if any)
        if !self.images.is_empty() {
            mdx.push_str("## Images\n\n");
            for image in &self.images {
                mdx.push_str(&format!(
                    "- {} ({}x{}, {})\n",
                    image.filename(),
                    image.width,
                    image.height,
                    image.format.extension().to_uppercase()
                ));
            }
            mdx.push('\n');
        }

        // Font information (if any have styling)
        let styled_fonts: Vec<_> = self.fonts.iter()
            .filter(|f| f.is_bold || f.is_italic)
            .collect();
        if !styled_fonts.is_empty() {
            mdx.push_str("## Font Styles\n\n");
            for font in styled_fonts {
                let style = match (font.is_bold, font.is_italic) {
                    (true, true) => "Bold Italic",
                    (true, false) => "Bold",
                    (false, true) => "Italic",
                    (false, false) => "Regular",
                };
                mdx.push_str(&format!("- {} ({})\n", font.base_font, style));
            }
            mdx.push('\n');
        }

        // Tables: rendered inline by `to_markdown_with_layout` at their
        // natural position in the document. A trailing `## Tables` section
        // is appended only for tables that have no usable Y coordinates
        // (e.g., hand-constructed in tests) so inline emission can't place
        // them.
        let orphan_tables: Vec<&PdfTable> = self.tables
            .iter()
            .filter(|t| !(t.y_top.is_finite() && t.y_bottom.is_finite()
                          && t.y_top > t.y_bottom))
            .collect();
        if !orphan_tables.is_empty() {
            mdx.push_str("## Tables\n\n");
            for (i, table) in orphan_tables.iter().enumerate() {
                if orphan_tables.len() > 1 {
                    mdx.push_str(&format!("### Table {} (Page {})\n\n", i + 1, table.page));
                }
                mdx.push_str(&table.to_markdown());
                mdx.push('\n');
            }
        }

        mdx
    }

    /// Get full text content
    pub fn full_text(&self) -> String {
        self.pages.iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Merge MasterFormat-style partial numbering lines with the following text.
///
/// Some construction / specification documents use numbering like:
///   .1 The intent of this Request for Proposal...
///   .2 Available information...
/// PDF text extractors often split these across two lines. This post-processor
/// detects a line containing only `.\d+` and fuses it with the next non-blank
/// line. If there is no follower, the line is preserved as-is.
fn merge_partial_numbering(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let stripped = line.trim();
        if is_partial_numbering(stripped) {
            // Find next non-blank line.
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < lines.len() {
                out.push(format!("{} {}", stripped, lines[j].trim()));
                i = j + 1;
                continue;
            } else {
                // No follower — preserve as-is.
                out.push(line.to_string());
                i += 1;
                continue;
            }
        }
        out.push(line.to_string());
        i += 1;
    }
    out.join("\n")
}

/// Returns true if `s` is exactly `.N` where N is one or more digits.
fn is_partial_numbering(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'.' {
        return false;
    }
    bytes[1..].iter().all(|b| b.is_ascii_digit())
}

/// Extract PDF text with a two-tier strategy: primary parser (pdf-extract)
/// guarded by `catch_unwind`, then `pdftotext` (Poppler) fallback on panic
/// or empty output.
///
/// **Why this exists**: `pdf-extract` panics on CJK CID fonts that use
/// encodings other than `Identity-H` (e.g. `Identity-V`, `UniKS-UTF16-H`).
/// Korean exam PDFs and most older Korean government documents trigger this
/// consistently. Rather than re-implementing a full PDF text extractor, we
/// shell out to Poppler's `pdftotext`, which has been the reference CJK
/// implementation for 20+ years.
///
/// Fallback is skipped silently when `pdftotext` is not on PATH — callers
/// get the original pdf-extract error / panic conversion. The fallback path
/// requires a file path (not in-memory bytes); in-memory callers use
/// `parse_from_memory` which has its own panic catch.
fn extract_text_with_fallback(
    path: &std::path::Path,
    _data: &[u8],
) -> io::Result<String> {
    // Tier 1: pdf-extract with panic guard.
    let path_owned = path.to_path_buf();
    let primary = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        pdf_extract::extract_text(&path_owned)
    }));

    match primary {
        Ok(Ok(text)) if !text.trim().is_empty() => return Ok(text),
        Ok(Ok(_)) => { /* empty — try fallback */ }
        Ok(Err(e)) => {
            // Real parser error, not panic. Try fallback before giving up.
            if let Some(text) = pdftotext_fallback(path) {
                return Ok(text);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("PDF extraction failed: {}", e),
            ));
        }
        Err(_) => { /* panic — try fallback */ }
    }

    // Tier 2: pdftotext fallback — if it ran and produced any bytes (even
    // just a form-feed for image-only PDFs), treat as success with empty text
    // so downstream image extraction still runs. Only error when pdftotext
    // is unavailable/broken.
    if !pdftotext_available() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "PDF extraction failed: pdf-extract panicked (likely CJK CID font \
             such as Identity-V or UniKS-UTF16-H). Install Poppler to enable \
             the pdftotext fallback:\n  \
             macOS:   brew install poppler\n  \
             Ubuntu:  apt install poppler-utils\n  \
             Windows: https://github.com/oschwartz10612/poppler-windows",
        ));
    }
    match pdftotext_invoke(path) {
        Some(text) => Ok(text), // may be empty for image-only PDFs
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PDF extraction failed: both pdf-extract and pdftotext (Poppler) \
             failed to read this file — may be corrupt or an unsupported variant",
        )),
    }
}

/// Cheap runtime check: is `pdftotext` on PATH and executable?
/// Caches the result for the process lifetime so repeated calls don't fork.
fn pdftotext_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("pdftotext")
            .arg("-v")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Invoke `pdftotext -layout -enc UTF-8 <path> -` and return stdout.
/// Returns None when the binary is missing or the invocation fails with a
/// non-zero status. When pdftotext runs successfully but produces only a
/// form-feed (image-only PDF), returns `Some("")` — this is semantically
/// "successfully processed, no extractable text" rather than a failure.
fn pdftotext_fallback(path: &std::path::Path) -> Option<String> {
    let text = pdftotext_invoke(path)?;
    if text.trim().is_empty() {
        return None; // treat as failure at the tier-1 level so tier 2 doesn't re-run
    }
    Some(text)
}

fn pdftotext_invoke(path: &std::path::Path) -> Option<String> {
    use std::process::Command;
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-enc")
        .arg("UTF-8")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_detection() {
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.7\n".to_vec(),
        };
        assert_eq!(parser.extract_version(), "1.7");
    }

    #[test]
    fn test_page_split() {
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.4\n".to_vec(),
        };

        // Test form feed split
        let text = "Page 1 content\x0CPage 2 content\x0CPage 3 content";
        let pages = parser.split_into_pages(text, 3);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[0].text, "Page 1 content");
    }

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Raw.extension(), "raw");
    }

    #[test]
    fn test_pdf_image_filename() {
        let image = PdfImage {
            id: "image_1".to_string(),
            width: 100,
            height: 200,
            format: ImageFormat::Jpeg,
            data: vec![],
            page: None,
        };
        assert_eq!(image.filename(), "image_1.jpg");
    }

    #[test]
    fn test_mdx_with_images() {
        let doc = PdfDocument {
            version: "1.7".to_string(),
            page_count: 1,
            pages: vec![PageContent {
                page_number: 1,
                text: "Hello".to_string(),
            }],
            metadata: PdfMetadata::default(),
            images: vec![PdfImage {
                id: "image_1".to_string(),
                width: 800,
                height: 600,
                format: ImageFormat::Jpeg,
                data: vec![],
                page: None,
            }],
            fonts: vec![],
            tables: vec![],
            layout: vec![],
        };

        let mdx = doc.to_mdx();
        assert!(mdx.contains("images: 1"));
        assert!(mdx.contains("## Images"));
        assert!(mdx.contains("image_1.jpg (800x600, JPG)"));
    }

    #[test]
    fn test_font_style_detection_bold() {
        let style = detect_font_style("Arial-Bold");
        assert!(style.is_bold);
        assert!(!style.is_italic);

        let style = detect_font_style("TimesNewRoman-BoldMT");
        assert!(style.is_bold);
        assert!(!style.is_italic);

        let style = detect_font_style("Helvetica-Black");
        assert!(style.is_bold);
        assert!(!style.is_italic);
    }

    #[test]
    fn test_font_style_detection_italic() {
        let style = detect_font_style("Arial-Italic");
        assert!(!style.is_bold);
        assert!(style.is_italic);

        let style = detect_font_style("TimesNewRoman-ItalicMT");
        assert!(!style.is_bold);
        assert!(style.is_italic);

        let style = detect_font_style("Helvetica-Oblique");
        assert!(!style.is_bold);
        assert!(style.is_italic);
    }

    #[test]
    fn test_font_style_detection_bold_italic() {
        let style = detect_font_style("Arial-BoldItalic");
        assert!(style.is_bold);
        assert!(style.is_italic);

        let style = detect_font_style("TimesNewRoman-BoldItalicMT");
        assert!(style.is_bold);
        assert!(style.is_italic);
    }

    #[test]
    fn test_font_style_detection_regular() {
        let style = detect_font_style("Arial");
        assert!(!style.is_bold);
        assert!(!style.is_italic);

        let style = detect_font_style("TimesNewRomanPSMT");
        assert!(!style.is_bold);
        assert!(!style.is_italic);
    }

    #[test]
    fn test_mdx_with_fonts() {
        let doc = PdfDocument {
            version: "1.7".to_string(),
            page_count: 1,
            pages: vec![PageContent {
                page_number: 1,
                text: "Hello".to_string(),
            }],
            metadata: PdfMetadata::default(),
            images: vec![],
            fonts: vec![
                PdfFont {
                    name: "F1".to_string(),
                    base_font: "Arial-Bold".to_string(),
                    is_bold: true,
                    is_italic: false,
                },
                PdfFont {
                    name: "F2".to_string(),
                    base_font: "Arial-Italic".to_string(),
                    is_bold: false,
                    is_italic: true,
                },
            ],
            tables: vec![],
            layout: vec![],
        };

        let mdx = doc.to_mdx();
        assert!(mdx.contains("fonts: 2"));
        assert!(mdx.contains("## Font Styles"));
        assert!(mdx.contains("Arial-Bold (Bold)"));
        assert!(mdx.contains("Arial-Italic (Italic)"));
    }

    #[test]
    fn test_table_to_markdown() {
        let table = PdfTable {
            page: 1,
            rows: vec![
                vec!["Name".to_string(), "Age".to_string(), "City".to_string()],
                vec!["Alice".to_string(), "30".to_string(), "NYC".to_string()],
                vec!["Bob".to_string(), "25".to_string(), "LA".to_string()],
            ],
            column_count: 3,
            y_top: 0.0,
            y_bottom: 0.0,
        };

        let md = table.to_markdown();
        assert!(md.contains("| Name | Age | City |"));
        assert!(md.contains("| --- | --- | --- |"));
        assert!(md.contains("| Alice | 30 | NYC |"));
        assert!(md.contains("| Bob | 25 | LA |"));
    }

    // Stale tests referencing the old `detect_table_from_positions` (Option return).
    // The function was refactored to `detect_tables_from_positions` -> Vec<PdfTable>
    // in commit de7090e but these assertions were never updated. Rewritten to
    // call the current Vec-returning API so `cargo test --lib` isn't blocked
    // at compile time. Semantics preserved: expect one detected table.
    #[test]
    fn test_table_detection_from_positions() {
        let texts = vec![
            PositionedText { text: "Name".to_string(), x: 100.0, y: 700.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "Age".to_string(), x: 200.0, y: 700.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "Alice".to_string(), x: 100.0, y: 680.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "30".to_string(), x: 200.0, y: 680.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "Bob".to_string(), x: 100.0, y: 660.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "25".to_string(), x: 200.0, y: 660.0, page: 1, font_size: None, font_name: None },
        ];

        let tables = detect_tables_from_positions(&texts, 1);
        assert!(!tables.is_empty(), "expected at least one detected table");
        let table = &tables[0];
        assert_eq!(table.column_count, 2);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[0], vec!["Name", "Age"]);
        assert_eq!(table.rows[1], vec!["Alice", "30"]);
        assert_eq!(table.rows[2], vec!["Bob", "25"]);
    }

    #[test]
    fn test_no_table_with_insufficient_data() {
        let texts = vec![
            PositionedText { text: "Hello".to_string(), x: 100.0, y: 700.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "World".to_string(), x: 100.0, y: 680.0, page: 1, font_size: None, font_name: None },
        ];

        let tables = detect_tables_from_positions(&texts, 1);
        assert!(tables.is_empty());
    }

    #[test]
    fn detects_two_column_layout() {
        // Synthesize a 2-column page: 20 lines of ~short text on the left
        // around x=50, another 20 lines on the right around x=320.
        let mut texts = Vec::new();
        for i in 0..20 {
            let y = 700.0 - (i as f64) * 15.0;
            texts.push(PositionedText {
                text: "LeftCol".to_string(),
                x: 50.0,
                y,
                page: 1, font_size: None, font_name: None,
            });
            texts.push(PositionedText {
                text: "RightCol".to_string(),
                x: 320.0,
                y,
                page: 1, font_size: None, font_name: None,
            });
        }

        let split = detect_column_split(&texts).expect("expected column split");
        // Split should land between ~85 (left right edge) and 320 (right start),
        // i.e. around 200 — well within the 35-65% band of a 612pt page.
        assert!(
            (150.0..250.0).contains(&split),
            "split {} outside expected range",
            split
        );
    }

    #[test]
    fn single_column_returns_no_split() {
        // One dense left column, nothing on the right.
        let mut texts = Vec::new();
        for i in 0..40 {
            texts.push(PositionedText {
                text: "Only column text".to_string(),
                x: 72.0,
                y: 700.0 - (i as f64) * 14.0,
                page: 1, font_size: None, font_name: None,
            });
        }
        assert!(detect_column_split(&texts).is_none());
    }

    #[test]
    fn too_few_items_returns_no_split() {
        // Below the 10-item floor (new histogram-based detector) — never a
        // column candidate. Two runs per side × 2 sides = 4 total < 10.
        let texts: Vec<PositionedText> = (0..2)
            .flat_map(|i| {
                vec![
                    PositionedText {
                        text: "L".to_string(),
                        x: 50.0,
                        y: 700.0 - (i as f64) * 15.0,
                        page: 1, font_size: None, font_name: None,
                    },
                    PositionedText {
                        text: "R".to_string(),
                        x: 320.0,
                        y: 700.0 - (i as f64) * 15.0,
                        page: 1, font_size: None, font_name: None,
                    },
                ]
            })
            .collect();
        assert!(detect_column_split(&texts).is_none());
    }

    #[test]
    fn short_two_column_page_is_detected() {
        // Regression: MDM failed test_twocolumn.pdf because it has only
        // ~15 positioned-text runs. The new detector should succeed at
        // this scale so 2-column reading order is preserved.
        let mut texts = Vec::new();
        // title, centered
        texts.push(PositionedText {
            text: "Title".to_string(),
            x: 250.0, y: 742.0, page: 1, font_size: None, font_name: None,
        });
        // 6 left-column runs at x=56
        for i in 0..6 {
            texts.push(PositionedText {
                text: "Left text content".to_string(),
                x: 56.0,
                y: 700.0 - (i as f64) * 14.0,
                page: 1, font_size: None, font_name: None,
            });
        }
        // 6 right-column runs at x=306
        for i in 0..6 {
            texts.push(PositionedText {
                text: "Right text content".to_string(),
                x: 306.0,
                y: 700.0 - (i as f64) * 14.0,
                page: 1, font_size: None, font_name: None,
            });
        }
        let split = detect_column_split(&texts).expect("expected a column split");
        assert!(
            (150.0..=230.0).contains(&split),
            "split {} should land between the two columns", split
        );
    }

    #[test]
    fn cjk_spacing_collapses_paren_whitespace() {
        assert_eq!(normalize_cjk_spacing("보도자료 ( 온라인 )"), "보도자료 (온라인)");
        assert_eq!(normalize_cjk_spacing("[ 한국 ]"), "[한국]");
    }

    #[test]
    fn cjk_spacing_collapses_trailing_punct() {
        assert_eq!(normalize_cjk_spacing("말했다 ."), "말했다.");
        assert_eq!(normalize_cjk_spacing("한 , 두 , 세"), "한, 두, 세");
    }

    #[test]
    fn cjk_spacing_collapses_runs_of_spaces() {
        assert_eq!(normalize_cjk_spacing("한국   교육"), "한국 교육");
    }

    #[test]
    fn cjk_spacing_fast_path_for_plain_ascii() {
        // Fast path: no brackets, no CJK → untouched.
        let original = "Hello world this is plain prose.";
        assert_eq!(normalize_cjk_spacing(original), original);
    }

    #[test]
    fn cjk_spacing_cleans_ascii_with_brackets() {
        // Brackets trigger the slow path even for ASCII, which is desired:
        // same whitespace noise happens in Latin documents too.
        assert_eq!(
            normalize_cjk_spacing("Hello (world) , three"),
            "Hello (world), three"
        );
    }

    #[test]
    fn test_pdf_string_extraction() {
        assert_eq!(extract_pdf_string("(Hello) Tj"), Some("Hello".to_string()));
        assert_eq!(extract_pdf_string("(Hello\\nWorld) Tj"), Some("Hello\nWorld".to_string()));
        assert_eq!(extract_pdf_string("<48656C6C6F> Tj"), Some("Hello".to_string()));
    }

    #[test]
    fn test_pdf_text_array_extraction() {
        assert_eq!(extract_pdf_text_array("[(Hello) -10 (World)] TJ"), Some("HelloWorld".to_string()));
        assert_eq!(extract_pdf_text_array("[(Test)] TJ"), Some("Test".to_string()));
    }

    #[test]
    fn test_mdx_with_tables() {
        let doc = PdfDocument {
            version: "1.7".to_string(),
            page_count: 1,
            pages: vec![PageContent {
                page_number: 1,
                text: "Hello".to_string(),
            }],
            metadata: PdfMetadata::default(),
            images: vec![],
            fonts: vec![],
            tables: vec![PdfTable {
                page: 1,
                rows: vec![
                    vec!["A".to_string(), "B".to_string()],
                    vec!["1".to_string(), "2".to_string()],
                ],
                column_count: 2,
                y_top: 0.0,
                y_bottom: 0.0,
            }],
            layout: vec![],
        };

        let mdx = doc.to_mdx();
        assert!(mdx.contains("tables: 1"));
        assert!(mdx.contains("## Tables"));
        assert!(mdx.contains("| A | B |"));
        assert!(mdx.contains("| 1 | 2 |"));
    }

    #[test]
    fn test_is_encrypted_unencrypted_pdf() {
        // Create minimal valid PDF without encryption
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF".to_vec(),
        };
        assert!(!parser.is_encrypted());
    }

    #[test]
    fn test_encryption_info_none_for_unencrypted() {
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.4\n".to_vec(),
        };
        assert!(parser.get_encryption_info().is_none());
    }

    #[test]
    fn test_layout_element_types() {
        assert_eq!(LayoutElementType::Text, LayoutElementType::Text);
        assert_ne!(LayoutElementType::Text, LayoutElementType::Image);
        assert_ne!(LayoutElementType::Header, LayoutElementType::Footer);
    }

    #[test]
    fn test_text_alignment_default() {
        let alignment: TextAlignment = Default::default();
        assert_eq!(alignment, TextAlignment::Left);
    }

    #[test]
    fn test_layout_element_creation() {
        let element = LayoutElement {
            element_type: LayoutElementType::Text,
            content: "Hello World".to_string(),
            page: 1,
            x: 72.0,
            y: 720.0,
            width: 200.0,
            height: 12.0,
            font_size: Some(12.0),
            font_name: Some("Arial".to_string()),
            alignment: TextAlignment::Left,
            is_bold: false,
            is_italic: false,
            line_spacing: 1.2,
            indent_level: 0,
            ref_id: None,
        };

        assert_eq!(element.content, "Hello World");
        assert_eq!(element.page, 1);
        assert_eq!(element.font_size, Some(12.0));
    }

    #[test]
    fn test_encryption_info_struct() {
        let info = EncryptionInfo {
            version: 4,
            revision: 4,
            key_length: 128,
            user_password_set: true,
            owner_password_set: true,
            permissions: -3904,
        };

        assert_eq!(info.version, 4);
        assert_eq!(info.key_length, 128);
        assert!(info.user_password_set);
    }

    #[test]
    fn test_pdf_error_display() {
        let err = PdfError::EncryptedNoPassword;
        assert!(format!("{}", err).contains("encrypted"));

        let err = PdfError::InvalidPassword;
        assert!(format!("{}", err).contains("Invalid password"));

        let err = PdfError::UnsupportedEncryption("AES-512".to_string());
        assert!(format!("{}", err).contains("AES-512"));
    }

    #[test]
    fn test_try_auto_decrypt_unencrypted() {
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.4\n".to_vec(),
        };

        // Should succeed for unencrypted PDF
        let result = parser.try_auto_decrypt();
        assert!(result.is_ok());
    }

    #[test]
    fn test_text_block_grouping() {
        let parser = PdfParser {
            path: std::path::PathBuf::new(),
            data: b"%PDF-1.4\n".to_vec(),
        };

        let texts = vec![
            PositionedText { text: "Hello".to_string(), x: 72.0, y: 720.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "World".to_string(), x: 150.0, y: 720.0, page: 1, font_size: None, font_name: None },
            PositionedText { text: "New line".to_string(), x: 72.0, y: 700.0, page: 1, font_size: None, font_name: None },
        ];

        let blocks = parser.group_text_into_blocks(&texts, 792.0);

        // Should group "Hello World" on same line
        assert!(blocks.len() >= 2);
        assert!(blocks[0].content.contains("Hello"));
    }

    #[test]
    fn test_merge_partial_numbering_basic() {
        let input = ".1\nThe intent of this Request for Proposal...\n.2\nAvailable information relative to...";
        let out = merge_partial_numbering(input);
        assert!(out.contains(".1 The intent"), "got: {:?}", out);
        assert!(out.contains(".2 Available"));
    }

    #[test]
    fn test_merge_partial_numbering_skip_blanks() {
        let input = ".3\n\n\nactual content here";
        let out = merge_partial_numbering(input);
        assert!(out.contains(".3 actual content here"), "got: {:?}", out);
    }

    #[test]
    fn test_merge_partial_numbering_no_false_positive() {
        // A decimal number like ".5" alone SHOULD merge, but normal prose
        // and valid sentence-initial markers shouldn't be disturbed.
        let input = "Regular line.\nNext line.\n.1\nItem text";
        let out = merge_partial_numbering(input);
        assert!(out.contains("Regular line."));
        assert!(out.contains(".1 Item text"));
        // The merge should not duplicate text.
        assert_eq!(out.matches("Item text").count(), 1);
    }

    #[test]
    fn test_merge_partial_numbering_trailing() {
        // Partial numbering at EOF with no follower: keep as-is.
        let input = "Paragraph\n.9";
        let out = merge_partial_numbering(input);
        assert!(out.ends_with(".9"));
    }
}
