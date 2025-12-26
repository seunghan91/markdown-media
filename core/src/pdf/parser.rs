//! PDF parser implementation using pdf-extract
//!
//! Provides text extraction from PDF files with page-by-page support.

use std::fs::File;
use std::io::{self, Read, BufReader};
use std::path::Path;

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
        
        Ok(PdfDocument {
            version,
            page_count,
            pages,
            metadata,
        })
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
fn get_pdf_string(doc: &lopdf::Document, dict: &lopdf::Dictionary, key: &[u8]) -> String {
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

impl PdfDocument {
    /// Convert to MDX format
    pub fn to_mdx(&self) -> String {
        let mut mdx = String::new();
        
        // Frontmatter
        mdx.push_str("---\n");
        mdx.push_str("format: pdf\n");
        mdx.push_str(&format!("version: \"{}\"\n", self.version));
        mdx.push_str(&format!("pages: {}\n", self.page_count));
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
}
