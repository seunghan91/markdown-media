//! weknora RAG 서비스용 내보내기 모듈
//!
//! 법률 청크를 다양한 형식으로 내보내기

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::legal::types::LegalChunk;

/// weknora RAG 서비스용 내보내기 클래스
pub struct WeKnoraExporter {
    /// 컨텍스트를 내용에 포함할지 여부
    pub include_context_in_content: bool,
}

impl Default for WeKnoraExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl WeKnoraExporter {
    /// 새 익스포터 생성
    pub fn new() -> Self {
        Self {
            include_context_in_content: true,
        }
    }

    /// 옵션을 지정하여 익스포터 생성
    pub fn with_options(include_context_in_content: bool) -> Self {
        Self {
            include_context_in_content,
        }
    }

    /// 임베딩용 데이터 내보내기
    pub fn export_for_embedding(&self, chunks: &[LegalChunk]) -> Vec<serde_json::Value> {
        chunks
            .iter()
            .map(|chunk| {
                // 컨텍스트를 내용에 포함 (RAG 검색 품질 향상)
                let enhanced_content = if self.include_context_in_content && !chunk.context_path.is_empty() {
                    format!("[{}]\n\n{}", chunk.context_path, chunk.content)
                } else {
                    chunk.content.clone()
                };

                json!({
                    "id": chunk.id,
                    "content": enhanced_content,
                    "raw_content": chunk.content,
                    "metadata": {
                        "law_name": chunk.metadata.law_name,
                        "law_id": chunk.metadata.law_id,
                        "category": chunk.metadata.category,
                        "revision_date": chunk.metadata.revision_date,
                        "effective_date": chunk.metadata.effective_date,
                        "hierarchy": {
                            "part": chunk.metadata.part,
                            "chapter": chunk.metadata.chapter,
                            "section": chunk.metadata.section,
                            "subsection": chunk.metadata.subsection,
                        },
                        "article": {
                            "number": chunk.metadata.article_number,
                            "title": chunk.metadata.article_title,
                            "paragraph": chunk.metadata.paragraph_number,
                        },
                        "references": chunk.metadata.references,
                        "source_file": chunk.metadata.source_file,
                        "chunk_type": chunk.chunk_type,
                        "token_count": chunk.token_count,
                        "context_path": chunk.context_path,
                    }
                })
            })
            .collect()
    }

    /// JSONL 형식으로 내보내기 (weknora 인제스트용)
    pub fn export_to_jsonl<P: AsRef<Path>>(
        &self,
        chunks: &[LegalChunk],
        output_path: P,
    ) -> Result<usize, std::io::Error> {
        let data = self.export_for_embedding(chunks);
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        for item in &data {
            serde_json::to_writer(&mut writer, item)?;
            writeln!(writer)?;
        }

        writer.flush()?;
        Ok(data.len())
    }

    /// JSON 배열로 내보내기
    pub fn export_to_json<P: AsRef<Path>>(
        &self,
        chunks: &[LegalChunk],
        output_path: P,
    ) -> Result<usize, std::io::Error> {
        let data = self.export_for_embedding(chunks);
        let file = File::create(output_path)?;
        let writer = BufWriter::new(file);

        serde_json::to_writer_pretty(writer, &data)?;
        Ok(data.len())
    }
}

/// 처리 통계
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessingStats {
    pub total_files: usize,
    pub total_chunks: usize,
    pub total_tokens: usize,
    pub files_processed: Vec<FileStats>,
    pub errors: Vec<FileError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub file: String,
    pub chunks: usize,
    pub tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileError {
    pub file: String,
    pub error: String,
}

impl ProcessingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_success(&mut self, file: String, chunks: usize, tokens: usize) {
        self.total_files += 1;
        self.total_chunks += chunks;
        self.total_tokens += tokens;
        self.files_processed.push(FileStats { file, chunks, tokens });
    }

    pub fn add_error(&mut self, file: String, error: String) {
        self.errors.push(FileError { file, error });
    }

    /// 통계를 JSON 파일로 저장
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legal::types::{ChunkType, LegalMetadata};

    fn create_test_chunk() -> LegalChunk {
        LegalChunk {
            id: "test123".to_string(),
            content: "제1조(목적) 이 규정은 유가증권시장의 상장에 관한 사항을 정한다.".to_string(),
            metadata: LegalMetadata {
                law_name: "유가증권시장 상장규정".to_string(),
                law_id: "12345".to_string(),
                category: "시장규정".to_string(),
                article_number: Some("1".to_string()),
                article_title: Some("목적".to_string()),
                ..Default::default()
            },
            chunk_type: ChunkType::Article,
            token_count: 25,
            context_path: "제1편 총칙 > 제1조(목적)".to_string(),
            parent_chunk_id: None,
        }
    }

    #[test]
    fn test_export_for_embedding() {
        let exporter = WeKnoraExporter::new();
        let chunks = vec![create_test_chunk()];
        
        let data = exporter.export_for_embedding(&chunks);
        
        assert_eq!(data.len(), 1);
        assert!(data[0]["content"].as_str().unwrap().contains("[제1편 총칙"));
        assert_eq!(data[0]["metadata"]["law_name"], "유가증권시장 상장규정");
    }

    #[test]
    fn test_export_for_embedding_without_context() {
        let exporter = WeKnoraExporter::with_options(false);
        let chunks = vec![create_test_chunk()];
        
        let data = exporter.export_for_embedding(&chunks);
        
        assert!(!data[0]["content"].as_str().unwrap().starts_with('['));
    }

    #[test]
    fn test_processing_stats() {
        let mut stats = ProcessingStats::new();
        
        stats.add_success("test1.md".to_string(), 10, 500);
        stats.add_success("test2.md".to_string(), 15, 750);
        stats.add_error("test3.md".to_string(), "Parse error".to_string());
        
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_chunks, 25);
        assert_eq!(stats.total_tokens, 1250);
        assert_eq!(stats.errors.len(), 1);
    }
}
