use std::fs::File;
use std::io::{self, Read, BufReader};
use std::path::Path;
use std::collections::HashMap;

/// DOCX 파일 파서
/// DOCX는 ZIP 압축된 XML 파일입니다
pub struct DocxParser {
    content: String,
    relationships: HashMap<String, String>,
}

impl DocxParser {
    /// DOCX 파일을 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        
        // DOCX is a ZIP file
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        // Read document.xml (main content)
        let content = Self::read_document_xml(&mut archive)?;
        
        // Read relationships
        let relationships = Self::read_relationships(&mut archive)?;
        
        Ok(DocxParser { content, relationships })
    }

    fn read_document_xml(archive: &mut zip::ZipArchive<BufReader<File>>) -> io::Result<String> {
        let mut doc_file = archive.by_name("word/document.xml")
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
        
        let mut content = String::new();
        doc_file.read_to_string(&mut content)?;
        Ok(content)
    }

    fn read_relationships(archive: &mut zip::ZipArchive<BufReader<File>>) -> io::Result<HashMap<String, String>> {
        let mut rels = HashMap::new();
        
        if let Ok(mut rels_file) = archive.by_name("word/_rels/document.xml.rels") {
            let mut content = String::new();
            rels_file.read_to_string(&mut content)?;
            
            // Simple parsing of relationships (would use XML parser in production)
            for line in content.lines() {
                if line.contains("Id=") && line.contains("Target=") {
                    // Extract Id and Target
                    if let (Some(id_start), Some(target_start)) = 
                        (line.find("Id=\""), line.find("Target=\"")) {
                        let id_end = line[id_start+4..].find('"').map(|i| id_start + 4 + i);
                        let target_end = line[target_start+8..].find('"').map(|i| target_start + 8 + i);
                        
                        if let (Some(ie), Some(te)) = (id_end, target_end) {
                            let id = &line[id_start+4..ie];
                            let target = &line[target_start+8..te];
                            rels.insert(id.to_string(), target.to_string());
                        }
                    }
                }
            }
        }
        
        Ok(rels)
    }

    /// 텍스트를 추출합니다
    pub fn extract_text(&self) -> String {
        let mut text = Vec::new();
        let mut in_text = false;
        let mut current_text = String::new();
        
        // Simple XML text extraction
        let mut chars = self.content.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '<' {
                // Start of tag
                let mut tag = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '>' {
                        chars.next();
                        break;
                    }
                    tag.push(chars.next().unwrap());
                }
                
                if tag.starts_with("w:t") && !tag.starts_with("w:t/") {
                    in_text = true;
                } else if tag == "/w:t" {
                    if !current_text.is_empty() {
                        text.push(current_text.clone());
                    }
                    current_text.clear();
                    in_text = false;
                } else if tag.starts_with("w:p") && tag.ends_with("/") {
                    // Self-closing paragraph
                    text.push("\n".to_string());
                } else if tag == "/w:p" {
                    // End of paragraph
                    text.push("\n".to_string());
                }
            } else if in_text {
                current_text.push(ch);
            }
        }
        
        text.join("")
    }

    /// 메타데이터를 추출합니다
    pub fn extract_metadata(&self) -> Metadata {
        Metadata {
            author: String::new(), // Would parse core.xml
            title: String::new(),
            created: String::new(),
        }
    }

    /// 이미지 목록을 가져옵니다
    pub fn list_images(&self) -> Vec<String> {
        self.relationships
            .values()
            .filter(|v| v.ends_with(".png") || v.ends_with(".jpg") || v.ends_with(".jpeg"))
            .cloned()
            .collect()
    }
}

/// DOCX 메타데이터
#[derive(Debug)]
pub struct Metadata {
    pub author: String,
    pub title: String,
    pub created: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_extraction() {
        // Would need a test DOCX file
    }
}
