//! PDF parser implementation using pdf-extract
//!
//! Provides text extraction from PDF files with page-by-page support,
//! image extraction, metadata parsing, encryption detection, and layout preservation.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use flate2::read::ZlibDecoder;
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
    pub bold: bool,
    pub italic: bool,
}

/// Text element with position information
#[derive(Debug, Clone)]
pub struct PositionedText {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub page: usize,
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
    /// Open a PDF file
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        // Validate PDF magic bytes
        if data.len() < 5 || &data[0..5] != b"%PDF-" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid PDF file"));
        }

        Ok(PdfParser { path, data })
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
            // Read decrypted file
            let mut decrypted_data = Vec::new();
            let mut file = File::open(&temp_output)?;
            file.read_to_end(&mut decrypted_data)?;

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
                let element_type = if block.y > page_height * 0.9 {
                    LayoutElementType::Header
                } else if block.y < page_height * 0.1 {
                    LayoutElementType::Footer
                } else if block.content.starts_with('•') || block.content.starts_with('-')
                    || block.content.starts_with("* ") {
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

    /// Group positioned text into logical blocks
    fn group_text_into_blocks(&self, texts: &[PositionedText], _page_height: f64) -> Vec<TextBlock> {
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
            match &mut current_block {
                Some(block) if (block.y - text.y).abs() < Y_TOLERANCE => {
                    // Same line, append text
                    if !block.content.is_empty() && !text.text.is_empty() {
                        block.content.push(' ');
                    }
                    block.content.push_str(&text.text);
                    block.width = (text.x - block.x).max(block.width);
                }
                Some(block) => {
                    // New line, save current block and start new one
                    blocks.push(block.clone());
                    current_block = Some(TextBlock {
                        content: text.text.clone(),
                        x: text.x,
                        y: text.y,
                        width: 100.0, // Estimated
                        height: 12.0, // Default line height
                        font_size: None,
                        font_name: None,
                        is_bold: false,
                        is_italic: false,
                    });
                }
                None => {
                    current_block = Some(TextBlock {
                        content: text.text.clone(),
                        x: text.x,
                        y: text.y,
                        width: 100.0,
                        height: 12.0,
                        font_size: None,
                        font_name: None,
                        is_bold: false,
                        is_italic: false,
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

        Ok(PdfDocument {
            version,
            page_count,
            pages,
            metadata,
            images,
            fonts,
            tables,
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
                    is_bold: style.bold,
                    is_italic: style.italic,
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

            // Detect tables from positioned text
            if let Some(table) = detect_table_from_positions(&positioned_texts, page_num as usize) {
                tables.push(table);
            }
        }

        tables
    }

    /// Extract text with position from a PDF page
    fn extract_positioned_text(&self, doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<PositionedText> {
        let mut texts = Vec::new();

        // Get page content stream
        let content = match doc.get_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return texts,
        };

        // Decompress if needed
        let content_str = match String::from_utf8(content.clone()) {
            Ok(s) => s,
            Err(_) => {
                // Try to decompress
                match decompress_flate(&content) {
                    Ok(decompressed) => String::from_utf8_lossy(&decompressed).to_string(),
                    Err(_) => return texts,
                }
            }
        };

        // Parse content stream for text operators
        // This is a simplified parser for common text positioning patterns
        let mut current_x = 0.0;
        let mut current_y = 0.0;
        let mut in_text_block = false;

        for line in content_str.lines() {
            let line = line.trim();

            // Text block markers
            if line == "BT" {
                in_text_block = true;
                continue;
            }
            if line == "ET" {
                in_text_block = false;
                continue;
            }

            if !in_text_block {
                continue;
            }

            // Text matrix (Tm) - sets position directly
            if line.ends_with(" Tm") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    if let (Ok(x), Ok(y)) = (
                        parts[4].parse::<f64>(),
                        parts[5].parse::<f64>(),
                    ) {
                        current_x = x;
                        current_y = y;
                    }
                }
            }

            // Text positioning (Td) - relative move
            if line.ends_with(" Td") || line.ends_with(" TD") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(dx), Ok(dy)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<f64>(),
                    ) {
                        current_x += dx;
                        current_y += dy;
                    }
                }
            }

            // Show text (Tj) - simple string
            if line.ends_with(" Tj") {
                if let Some(text) = extract_pdf_string(line) {
                    if !text.trim().is_empty() {
                        texts.push(PositionedText {
                            text,
                            x: current_x,
                            y: current_y,
                            page: 0, // Will be set by caller
                        });
                    }
                }
            }

            // Show text array (TJ)
            if line.ends_with(" TJ") {
                if let Some(text) = extract_pdf_text_array(line) {
                    if !text.trim().is_empty() {
                        texts.push(PositionedText {
                            text,
                            x: current_x,
                            y: current_y,
                            page: 0,
                        });
                    }
                }
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

/// Decompress FlateDecode (zlib) data
fn decompress_flate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
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
        bold: is_bold,
        italic: is_italic,
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

/// Detect table structure from positioned text elements
fn detect_table_from_positions(texts: &[PositionedText], page: usize) -> Option<PdfTable> {
    if texts.len() < 4 {
        return None; // Need at least 2x2 cells
    }

    // Group texts by Y position (rows) with tolerance
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

    // Sort rows by Y position (descending for PDF coordinate system)
    rows.sort_by(|a, b| {
        let y_a = a.first().map(|t| t.y).unwrap_or(0.0);
        let y_b = b.first().map(|t| t.y).unwrap_or(0.0);
        y_b.partial_cmp(&y_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sort each row by X position
    for row in &mut rows {
        row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Check if we have consistent column structure (table heuristic)

    // Need at least 2 rows with 2+ columns
    let valid_rows: Vec<&Vec<&PositionedText>> = rows.iter()
        .filter(|r| r.len() >= 2)
        .collect();

    if valid_rows.len() < 2 {
        return None;
    }

    // Check column alignment (within tolerance)
    const X_TOLERANCE: f64 = 20.0;
    let first_row_x: Vec<f64> = valid_rows[0].iter().map(|t| t.x).collect();

    let mut aligned_count = 0;
    for row in &valid_rows[1..] {
        let row_x: Vec<f64> = row.iter().map(|t| t.x).collect();
        if row_x.len() == first_row_x.len() {
            let is_aligned = row_x.iter()
                .zip(first_row_x.iter())
                .all(|(x1, x2)| (x1 - x2).abs() < X_TOLERANCE);
            if is_aligned {
                aligned_count += 1;
            }
        }
    }

    // At least 50% of rows should be aligned
    if aligned_count * 2 < valid_rows.len() - 1 {
        return None;
    }

    // Build table structure
    let column_count = valid_rows[0].len();
    let table_rows: Vec<Vec<String>> = valid_rows.iter()
        .filter(|r| r.len() == column_count)
        .map(|r| r.iter().map(|t| t.text.clone()).collect())
        .collect();

    if table_rows.len() < 2 {
        return None;
    }

    Some(PdfTable {
        page,
        rows: table_rows,
        column_count,
    })
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

impl PdfDocument {
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

        // Content with page markers
        for page in &self.pages {
            if self.page_count > 1 {
                mdx.push_str(&format!("<!-- Page {} -->\n\n", page.page_number));
            }
            mdx.push_str(&page.text);
            mdx.push_str("\n\n");
        }

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
        };

        let mdx = doc.to_mdx();
        assert!(mdx.contains("images: 1"));
        assert!(mdx.contains("## Images"));
        assert!(mdx.contains("image_1.jpg (800x600, JPG)"));
    }

    #[test]
    fn test_font_style_detection_bold() {
        let style = detect_font_style("Arial-Bold");
        assert!(style.bold);
        assert!(!style.italic);

        let style = detect_font_style("TimesNewRoman-BoldMT");
        assert!(style.bold);
        assert!(!style.italic);

        let style = detect_font_style("Helvetica-Black");
        assert!(style.bold);
        assert!(!style.italic);
    }

    #[test]
    fn test_font_style_detection_italic() {
        let style = detect_font_style("Arial-Italic");
        assert!(!style.bold);
        assert!(style.italic);

        let style = detect_font_style("TimesNewRoman-ItalicMT");
        assert!(!style.bold);
        assert!(style.italic);

        let style = detect_font_style("Helvetica-Oblique");
        assert!(!style.bold);
        assert!(style.italic);
    }

    #[test]
    fn test_font_style_detection_bold_italic() {
        let style = detect_font_style("Arial-BoldItalic");
        assert!(style.bold);
        assert!(style.italic);

        let style = detect_font_style("TimesNewRoman-BoldItalicMT");
        assert!(style.bold);
        assert!(style.italic);
    }

    #[test]
    fn test_font_style_detection_regular() {
        let style = detect_font_style("Arial");
        assert!(!style.bold);
        assert!(!style.italic);

        let style = detect_font_style("TimesNewRomanPSMT");
        assert!(!style.bold);
        assert!(!style.italic);
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

    #[test]
    fn test_table_detection_from_positions() {
        let texts = vec![
            PositionedText { text: "Name".to_string(), x: 100.0, y: 700.0, page: 1 },
            PositionedText { text: "Age".to_string(), x: 200.0, y: 700.0, page: 1 },
            PositionedText { text: "Alice".to_string(), x: 100.0, y: 680.0, page: 1 },
            PositionedText { text: "30".to_string(), x: 200.0, y: 680.0, page: 1 },
            PositionedText { text: "Bob".to_string(), x: 100.0, y: 660.0, page: 1 },
            PositionedText { text: "25".to_string(), x: 200.0, y: 660.0, page: 1 },
        ];

        let table = detect_table_from_positions(&texts, 1);
        assert!(table.is_some());
        let table = table.unwrap();
        assert_eq!(table.column_count, 2);
        assert_eq!(table.rows.len(), 3);
        assert_eq!(table.rows[0], vec!["Name", "Age"]);
        assert_eq!(table.rows[1], vec!["Alice", "30"]);
        assert_eq!(table.rows[2], vec!["Bob", "25"]);
    }

    #[test]
    fn test_no_table_with_insufficient_data() {
        let texts = vec![
            PositionedText { text: "Hello".to_string(), x: 100.0, y: 700.0, page: 1 },
            PositionedText { text: "World".to_string(), x: 100.0, y: 680.0, page: 1 },
        ];

        let table = detect_table_from_positions(&texts, 1);
        assert!(table.is_none());
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
            PositionedText { text: "Hello".to_string(), x: 72.0, y: 720.0, page: 1 },
            PositionedText { text: "World".to_string(), x: 150.0, y: 720.0, page: 1 },
            PositionedText { text: "New line".to_string(), x: 72.0, y: 700.0, page: 1 },
        ];

        let blocks = parser.group_text_into_blocks(&texts, 792.0);

        // Should group "Hello World" on same line
        assert!(blocks.len() >= 2);
        assert!(blocks[0].content.contains("Hello"));
    }
}
