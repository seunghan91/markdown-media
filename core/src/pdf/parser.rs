//! PDF parser implementation using pdf-extract
//!
//! Provides text extraction from PDF files with page-by-page support,
//! image extraction, and metadata parsing.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use flate2::read::ZlibDecoder;

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

        Ok(PdfDocument {
            version,
            page_count,
            pages,
            metadata,
            images,
            fonts,
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
        };

        let mdx = doc.to_mdx();
        assert!(mdx.contains("fonts: 2"));
        assert!(mdx.contains("## Font Styles"));
        assert!(mdx.contains("Arial-Bold (Bold)"));
        assert!(mdx.contains("Arial-Italic (Italic)"));
    }
}
