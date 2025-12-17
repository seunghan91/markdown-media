use super::ole::OleReader;
use std::io::{self, Read};
use std::path::Path;

/// HWP 파일 파서
pub struct HwpParser {
    ole_reader: OleReader,
}

impl HwpParser {
    /// HWP 파일을 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let ole_reader = OleReader::open(path)?;
        Ok(HwpParser { ole_reader })
    }

    /// HWP 파일 구조를 분석합니다
    pub fn analyze(&self) -> FileStructure {
        let streams = self.ole_reader.list_streams();
        
        FileStructure {
            total_streams: streams.len(),
            streams,
        }
    }

    /// 텍스트를 추출합니다
    pub fn extract_text(&mut self) -> io::Result<String> {
        let mut all_text = Vec::new();
        
        // Try to read multiple sections
        for section_num in 0..10 {
            match self.ole_reader.read_body_text(section_num) {
                Ok(data) => {
                    // HWP uses zlib compression for BodyText
                    // For now, we'll extract what we can from the raw data
                    let section_text = self.parse_section_data(&data);
                    if !section_text.is_empty() {
                        all_text.push(format!("=== Section {} ===\n{}", section_num, section_text));
                    }
                }
                Err(_) => break, // No more sections
            }
        }
        
        if all_text.is_empty() {
            Ok("No text extracted. File may be compressed or encrypted.".to_string())
        } else {
            Ok(all_text.join("\n\n"))
        }
    }

    /// 섹션 데이터에서 텍스트 파싱 (단순 ASCII 추출)
    fn parse_section_data(&self, data: &[u8]) -> String {
        // Simple text extraction: find printable ASCII/UTF-8 sequences
        let mut result = String::new();
        let mut current_word = Vec::new();
        
        for &byte in data {
            if byte >= 32 && byte < 127 {
                // Printable ASCII
                current_word.push(byte);
            } else if byte == 0x0A || byte == 0x0D {
                // Newline
                if !current_word.is_empty() {
                    if let Ok(s) = String::from_utf8(current_word.clone()) {
                        result.push_str(&s);
                        result.push('\n');
                    }
                    current_word.clear();
                }
            } else if !current_word.is_empty() && (byte == 0 || byte > 127) {
                // End of word
                if let Ok(s) = String::from_utf8(current_word.clone()) {
                    if s.len() > 2 {
                        result.push_str(&s);
                        result.push(' ');
                    }
                }
                current_word.clear();
            }
        }
        
        result.trim().to_string()
    }

    /// 이미지를 추출합니다
    pub fn extract_images(&mut self) -> io::Result<Vec<ImageData>> {
        let mut images = Vec::new();
        
        // Try to read BinData streams
        let streams = self.ole_reader.list_streams();
        for stream in streams {
            if stream.contains("BinData") {
                if let Ok(data) = self.ole_reader.read_stream(&stream) {
                    // Detect image format from magic bytes
                    let format = detect_image_format(&data);
                    if !format.is_empty() {
                        images.push(ImageData {
                            name: stream,
                            format,
                            data,
                        });
                    }
                }
            }
        }
        
        Ok(images)
    }

    /// 표 구조를 추출합니다
    pub fn extract_tables(&mut self) -> io::Result<Vec<TableData>> {
        // Basic table detection: look for structured data patterns
        // Real implementation would parse HWP table records
        let tables = Vec::new();
        
        // Placeholder: would need to parse table control records
        // from the BodyText sections
        
        Ok(tables)
    }

    /// 메타데이터를 추출합니다
    pub fn extract_metadata(&mut self) -> io::Result<Metadata> {
        match self.ole_reader.read_file_header() {
            Ok(header_data) => {
                // Parse basic metadata from FileHeader
                Ok(Metadata {
                    version: parse_version(&header_data),
                    author: parse_metadata_field(&header_data, "author"),
                    title: parse_metadata_field(&header_data, "title"),
                    created: parse_metadata_field(&header_data, "created"),
                })
            }
            Err(e) => Err(e),
        }
    }
}

/// 이미지 포맷 감지
fn detect_image_format(data: &[u8]) -> String {
    if data.len() < 4 {
        return String::new();
    }
    
    // Check magic bytes
    if data[0] == 0xFF && data[1] == 0xD8 {
        "jpeg".to_string()
    } else if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        "png".to_string()
    } else if data[0] == 0x47 && data[1] == 0x49 && data[2] == 0x46 {
        "gif".to_string()
    } else if data[0] == 0x42 && data[1] == 0x4D {
        "bmp".to_string()
    } else {
        String::new()
    }
}

/// 버전 파싱
fn parse_version(data: &[u8]) -> String {
    if data.len() >= 36 {
        // HWP version is typically at offset 0-31
        format!("HWP {}.{}", data[0], data[1])
    } else {
        "Unknown".to_string()
    }
}

/// 메타데이터 필드 파싱 (간단한 구현)
fn parse_metadata_field(_data: &[u8], _field: &str) -> String {
    // Simplified: would need proper parsing of HWP metadata structures
    String::new()
}

/// HWP 파일 구조 정보
#[derive(Debug)]
pub struct FileStructure {
    pub total_streams: usize,
    pub streams: Vec<String>,
}

/// 이미지 데이터
#[derive(Debug)]
pub struct ImageData {
    pub name: String,
    pub format: String,
    pub data: Vec<u8>,
}

/// 표 데이터
#[derive(Debug)]
pub struct TableData {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<String>>,
}

/// 메타데이터
#[derive(Debug)]
pub struct Metadata {
    pub version: String,
    pub author: String,
    pub title: String,
    pub created: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        // JPEG needs at least 2 bytes but more for proper detection
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_image_format(&jpeg), "jpeg");
        
        let png = vec![0x89, 0x50, 0x4E, 0x47];
        assert_eq!(detect_image_format(&png), "png");
        
        let gif = vec![0x47, 0x49, 0x46, 0x38];
        assert_eq!(detect_image_format(&gif), "gif");
        
        let bmp = vec![0x42, 0x4D, 0x00, 0x00];
        assert_eq!(detect_image_format(&bmp), "bmp");
        
        // Too short - should return empty
        let short = vec![0xFF];
        assert_eq!(detect_image_format(&short), "");
    }
}
