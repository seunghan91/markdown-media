use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// PDF 파일 파서
pub struct PdfParser {
    data: Vec<u8>,
}

impl PdfParser {
    /// PDF 파일을 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        // Validate PDF magic bytes
        if data.len() < 5 || &data[0..5] != b"%PDF-" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a valid PDF file"));
        }
        
        Ok(PdfParser { data })
    }

    /// PDF 버전을 가져옵니다
    pub fn get_version(&self) -> String {
        // PDF version is in first line: %PDF-1.4
        if let Some(newline_pos) = self.data.iter().position(|&b| b == b'\n') {
            if let Ok(header) = String::from_utf8(self.data[0..newline_pos].to_vec()) {
                return header.replace("%PDF-", "");
            }
        }
        "Unknown".to_string()
    }

    /// 텍스트를 추출합니다 (기본 구현)
    pub fn extract_text(&self) -> io::Result<String> {
        let mut text = Vec::new();
        
        // Look for text streams between BT (Begin Text) and ET (End Text) operators
        let data_str = String::from_utf8_lossy(&self.data);
        
        for chunk in data_str.split("BT") {
            if let Some(text_chunk) = chunk.split("ET").next() {
                // Extract text from Tj and TJ operators
                for line in text_chunk.lines() {
                    if line.contains("Tj") || line.contains("TJ") {
                        // Simple extraction: get content between parentheses
                        if let Some(start) = line.find('(') {
                            if let Some(end) = line.rfind(')') {
                                let extracted = &line[start+1..end];
                                text.push(extracted.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        Ok(text.join("\n"))
    }

    /// 페이지 수를 가져옵니다
    pub fn get_page_count(&self) -> usize {
        let data_str = String::from_utf8_lossy(&self.data);
        
        // Look for /Type /Page entries
        data_str.matches("/Type /Page").count()
    }

    /// 메타데이터를 추출합니다
    pub fn extract_metadata(&self) -> Metadata {
        let data_str = String::from_utf8_lossy(&self.data);
        
        Metadata {
            version: self.get_version(),
            page_count: self.get_page_count(),
            producer: extract_metadata_field(&data_str, "Producer"),
            creator: extract_metadata_field(&data_str, "Creator"),
            title: extract_metadata_field(&data_str, "Title"),
        }
    }
}

/// 메타데이터 필드 추출
fn extract_metadata_field(data: &str, field: &str) -> String {
    let pattern = format!("/{}", field);
    if let Some(pos) = data.find(&pattern) {
        let after = &data[pos..];
        if let Some(start) = after.find('(') {
            if let Some(end) = after.find(')') {
                return after[start+1..end].to_string();
            }
        }
    }
    String::new()
}

/// PDF 메타데이터
#[derive(Debug)]
pub struct Metadata {
    pub version: String,
    pub page_count: usize,
    pub producer: String,
    pub creator: String,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_detection() {
        let pdf_data = b"%PDF-1.7\n".to_vec();
        let parser = PdfParser { data: pdf_data };
        assert_eq!(parser.get_version(), "1.7");
    }
}
