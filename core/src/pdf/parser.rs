//! PDF parser implementation using pdf-extract
//!
//! Provides text extraction from PDF files with page-by-page support,
//! image extraction, metadata parsing, encryption detection, and layout preservation.

use crate::utils::bounded_io::{read_limited, MAX_PDF_FILE, MAX_PDF_STREAM};
use flate2::read::ZlibDecoder;
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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

        let full_text = pdf_extract::extract_text_from_mem(&self.data)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("PDF extraction failed: {}", e),
                )
            })?;

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
    pub fn extract_layout(&self) -> Vec<LayoutElement> {
        let mut elements = Vec::new();

        let doc = match lopdf::Document::load_mem(&self.data) {
            Ok(d) => d,
            Err(_) => return elements,
        };

        // Get page dimensions for relative positioning
        for (page_num, page_id) in doc.get_pages() {
            let page_num = page_num as usize;

            // Get page dimensions
            let (page_width, page_height) = self.get_page_dimensions(&doc, page_id);

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
            let positioned_texts = self.extract_positioned_text(&doc, page_id);

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

            // Add image elements
            for image in &self.extract_images() {
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
        }

        // Sort elements by page and Y position (top to bottom)
        elements.sort_by(|a, b| {
            match a.page.cmp(&b.page) {
                std::cmp::Ordering::Equal => {
                    // Within same page, sort by Y (descending for PDF coords) then X
                    match b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal) {
                        std::cmp::Ordering::Equal => {
                            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        other => other,
                    }
                }
                other => other,
            }
        });

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

        // 2-column layout probe. When a split is detected, process each
        // column independently and concatenate — otherwise fall through to
        // the single-column walker below.
        if let Some(split_x) = detect_column_split(texts) {
            let (left, right): (Vec<PositionedText>, Vec<PositionedText>) = texts
                .iter()
                .cloned()
                .partition(|t| approx_right_edge(t) / 2.0 + t.x / 2.0 < split_x);
            let mut blocks = self.group_text_single_column(&left, page_height);
            blocks.extend(self.group_text_single_column(&right, page_height));
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

        let mut sorted_texts = texts.to_vec();
        sorted_texts.sort_by(|a, b| {
            b.y.partial_cmp(&a.y)
                .unwrap_or(std::cmp::Ordering::Equal)
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

        // Use pdf-extract for text extraction
        let full_text = pdf_extract::extract_text(&self.path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("PDF extraction failed: {}", e)))?;

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

        // Extract positioned text from each page
        for (page_num, page_id) in doc.get_pages() {
            let positioned_texts = self.extract_positioned_text(&doc, page_id);

            if positioned_texts.is_empty() {
                continue;
            }

            // Detect tables from positioned text (multiple tables per page)
            tables.extend(detect_tables_from_positions(&positioned_texts, page_num as usize));
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

/// Detect a 2-column page layout and return the split X coordinate if
/// present. Ported from kordoc pdf/parser.ts:676-711 `hasMultiColumnLayout`
/// but returns the split itself so callers can partition text.
///
/// Heuristics:
/// 1. At least 30 text runs (pages with less are headers/covers)
/// 2. Page content width ≥ 200 pt
/// 3. Biggest X-gap between consecutive runs (sorted by start X) ≥ 20 pt
/// 4. Gap center lies in the 35%–65% band of the page width
/// 5. Both sides have ≥ 15 runs
/// 6. Item-count ratio min/max ≥ 0.35 (balanced columns)
///
/// Returns `None` when any check fails.
pub(crate) fn detect_column_split(texts: &[PositionedText]) -> Option<f64> {
    if texts.len() < 30 {
        return None;
    }

    // Sort by start X so we can scan consecutive gaps.
    let mut sorted: Vec<&PositionedText> = texts.iter().collect();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let min_x = sorted[0].x;
    let max_x = sorted
        .iter()
        .map(|t| approx_right_edge(t))
        .fold(f64::NEG_INFINITY, f64::max);
    let page_width = max_x - min_x;
    if page_width < 200.0 {
        return None;
    }

    // Biggest X gap = largest (next.x) - (prev.x + prev_width_approx)
    let mut best_gap = 0.0_f64;
    let mut best_split = 0.0_f64;
    for j in 1..sorted.len() {
        let prev_right = approx_right_edge(sorted[j - 1]);
        let gap = sorted[j].x - prev_right;
        if gap > best_gap {
            best_gap = gap;
            best_split = (prev_right + sorted[j].x) / 2.0;
        }
    }
    if best_gap < 20.0 {
        return None;
    }

    // Gap center must be near page center (35–65%).
    let split_ratio = (best_split - min_x) / page_width;
    if !(0.35..=0.65).contains(&split_ratio) {
        return None;
    }

    // Both sides need enough items and balanced counts.
    let left_count = texts
        .iter()
        .filter(|t| (t.x + approx_right_edge(t)) / 2.0 < best_split)
        .count();
    let right_count = texts.len() - left_count;
    if left_count < 15 || right_count < 15 {
        return None;
    }
    let (lo, hi) = if left_count < right_count {
        (left_count, right_count)
    } else {
        (right_count, left_count)
    };
    if (lo as f64) / (hi as f64) < 0.35 {
        return None;
    }

    Some(best_split)
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
        // the nearest anchor)
        let mut table_rows: Vec<Vec<String>> = Vec::new();
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
                table_rows.push(cells);
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
        // align on column boundaries (common in legal/guide PDFs).
        let mut cell_lens: Vec<usize> = table_rows
            .iter()
            .flat_map(|r| r.iter().filter(|c| !c.is_empty()).map(|c| c.chars().count()))
            .collect();
        if cell_lens.len() >= 4 {
            cell_lens.sort_unstable();
            let median = cell_lens[cell_lens.len() / 2];
            if median > 100 {
                continue;
            }
        }

        out.push(PdfTable {
            page,
            rows: table_rows,
            column_count: mode_cols,
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

        let mut output = String::new();
        let mut last_page: usize = 0;
        let mut last_y: f64 = f64::MAX; // Track Y position for inline segments

        for elem in &self.layout {
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

            // Image elements
            if elem.element_type == LayoutElementType::Image {
                if let Some(ref id) = elem.ref_id {
                    output.push_str(&format!("![{}]({})\n\n", id, id));
                }
                continue;
            }

            let content = elem.content.trim();
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
            } else if elem.element_type == LayoutElementType::ListItem
                || is_list_item(content)
            {
                // Normalize list items
                let normalized = normalize_list_item(content);
                output.push_str(&format!("{}\n", normalized));
            } else {
                // Regular text with bold/italic formatting
                let formatted = apply_inline_formatting(content, elem.is_bold, elem.is_italic);

                // If this block is on the same line as the previous one (same Y),
                // append inline without paragraph break
                let same_line = (last_y - elem.y).abs() < 12.0 && last_y != f64::MAX;
                if same_line {
                    // Add space before inline segment if needed
                    if !output.ends_with(' ') && !output.ends_with('\n') {
                        output.push(' ');
                    }
                    output.push_str(&formatted);
                } else {
                    output.push_str(&formatted);
                    output.push_str("\n\n");
                }
            }

            last_y = elem.y;
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

        // Tables (if any detected)
        if !self.tables.is_empty() {
            mdx.push_str("## Tables\n\n");
            for (i, table) in self.tables.iter().enumerate() {
                if self.tables.len() > 1 {
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
        // Below the 30-item floor — never a column candidate.
        let texts: Vec<PositionedText> = (0..10)
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
}
