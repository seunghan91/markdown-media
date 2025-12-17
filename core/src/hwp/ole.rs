use cfb::CompoundFile;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// OLE 파일 구조를 분석하고 스트림을 추출합니다
pub struct OleReader {
    compound_file: CompoundFile<File>,
}

impl OleReader {
    /// HWP 파일을 OLE 구조로 엽니다
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let compound_file = CompoundFile::open(file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        Ok(OleReader { compound_file })
    }

    /// 모든 스트림 이름 목록을 가져옵니다
    pub fn list_streams(&self) -> Vec<String> {
        self.compound_file
            .walk()
            .filter_map(|entry| {
                if entry.is_stream() {
                    Some(entry.path().to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// 특정 스트림의 내용을 읽습니다
    pub fn read_stream(&mut self, stream_name: &str) -> io::Result<Vec<u8>> {
        let mut stream = self.compound_file.open_stream(stream_name)
            .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;
        
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    /// FileHeader 스트림을 읽습니다 (HWP 메타데이터)
    pub fn read_file_header(&mut self) -> io::Result<Vec<u8>> {
        self.read_stream("FileHeader")
    }

    /// BodyText 섹션을 읽습니다
    pub fn read_body_text(&mut self, section: usize) -> io::Result<Vec<u8>> {
        let stream_name = format!("BodyText/Section{}", section);
        self.read_stream(&stream_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ole_structure() {
        // 실제 HWP 파일이 있어야 테스트 가능
        // TODO: 샘플 HWP 파일 추가
    }
}
