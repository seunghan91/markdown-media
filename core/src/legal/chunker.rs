//! Korean Legal Document Chunker
//!
//! 한국 법령 문서를 조(Article) 단위로 파싱하여 청크를 생성합니다.

use std::fs;
use std::path::Path;
use sha2::{Sha256, Digest};

use crate::legal::patterns::*;
use crate::legal::types::*;

/// 한국 법률 문서 청킹 클래스
pub struct KoreanLegalChunker {
    /// 조(Article) 단위로 청킹할지 여부
    pub chunk_by_article: bool,
    /// 상위 계층 컨텍스트 포함 여부
    pub include_context: bool,
    /// 최대 청크 토큰 수
    pub max_chunk_tokens: usize,
    /// 청크 간 오버랩 토큰 수
    pub overlap_tokens: usize,
    /// 현재 파싱 상태
    current_state: ParsingState,
}

impl Default for KoreanLegalChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl KoreanLegalChunker {
    /// 새 청커 생성
    pub fn new() -> Self {
        Self {
            chunk_by_article: true,
            include_context: true,
            max_chunk_tokens: 512,
            overlap_tokens: 50,
            current_state: ParsingState::new(),
        }
    }

    /// 옵션을 지정하여 청커 생성
    pub fn with_options(
        chunk_by_article: bool,
        include_context: bool,
        max_chunk_tokens: usize,
        overlap_tokens: usize,
    ) -> Self {
        Self {
            chunk_by_article,
            include_context,
            max_chunk_tokens,
            overlap_tokens,
            current_state: ParsingState::new(),
        }
    }

    /// 토큰 수 추정 (한글은 대략 1.5자당 1토큰)
    pub fn estimate_tokens(&self, text: &str) -> usize {
        let korean_count = RE_KOREAN.find_iter(text).count();
        let alphanum_count = RE_ALPHANUMERIC.find_iter(text).count();
        let space_count = RE_WHITESPACE.find_iter(text).count();

        (korean_count as f64 / 1.5 + alphanum_count as f64 / 4.0 + space_count as f64 / 4.0) as usize
    }

    /// 청크 고유 ID 생성 (SHA256 해시)
    pub fn generate_chunk_id(&self, content: &str, metadata: &LegalMetadata) -> String {
        let unique_str = format!(
            "{}:{}:{}",
            metadata.law_name,
            metadata.article_number.as_deref().unwrap_or(""),
            &content.chars().take(100).collect::<String>()
        );
        
        let mut hasher = Sha256::new();
        hasher.update(unique_str.as_bytes());
        let result = hasher.finalize();
        
        // 첫 16바이트를 hex로 변환
        hex::encode(&result[..8])
    }

    /// 마크다운 헤더에서 메타데이터 추출
    fn parse_metadata_header(&self, lines: &[&str]) -> (LegalMetadata, usize) {
        let mut metadata = LegalMetadata::default();
        let mut start_idx = 0;

        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();

            // 제목 (# 법령명)
            if line.starts_with("# ") {
                metadata.law_name = line[2..].trim().to_string();
                continue;
            }

            // 규정 ID
            if line.starts_with("- **규정 ID**:") {
                if let Some(id) = line.split(':').last() {
                    metadata.law_id = id.trim().to_string();
                }
                continue;
            }

            // 분류
            if line.starts_with("- **분류**:") {
                if let Some(cat) = line.split(':').last() {
                    metadata.category = cat.trim().to_string();
                }
                continue;
            }

            // 개정 정보
            if let Some(caps) = RE_REVISION.captures(line) {
                let year = &caps[1];
                let month = &caps[2];
                let day = &caps[3];
                metadata.revision_date = Some(format!(
                    "{}-{:0>2}-{:0>2}",
                    year,
                    month,
                    day
                ));
                if let Some(eff) = caps.get(4) {
                    metadata.effective_date = Some(eff.as_str().to_string());
                }
            }

            // 개정 차수
            if let Some(caps) = RE_REVISION_NUMBER.captures(line) {
                metadata.revision_number = Some(caps[1].to_string());
            }

            // 본문 시작 감지 (제X편, 제X장, 제X조)
            if RE_PART.is_match(line) || RE_CHAPTER.is_match(line) || RE_ARTICLE.is_match(line) {
                start_idx = i;
                break;
            }
        }

        (metadata, start_idx)
    }

    /// 법조문 참조 관계 추출
    fn extract_references(&self, text: &str) -> Vec<LegalReference> {
        let mut references = Vec::new();
        let mut external_raw_texts: Vec<String> = Vec::new();

        // 외부 법률 참조
        for caps in RE_LAW_REFERENCE.captures_iter(text) {
            let target_law = caps[1].to_string();
            let target_article = caps.get(2).map(|m| format!("제{}조", m.as_str()));
            let raw_text = caps[0].to_string();
            
            external_raw_texts.push(raw_text.clone());
            references.push(LegalReference::external(target_law, target_article, raw_text));
        }

        // 내부 참조 (같은 법령 내)
        for caps in RE_INTERNAL_REFERENCE.captures_iter(text) {
            let raw_text = caps[0].to_string();
            
            // 이미 외부 참조로 처리된 것 제외
            if external_raw_texts.iter().any(|ext| ext.contains(&raw_text)) {
                continue;
            }

            let mut article = format!("제{}조", &caps[1]);
            if let Some(branch) = caps.get(2) {
                article.push_str(&format!("의{}", branch.as_str()));
            }

            references.push(LegalReference::internal(article, raw_text));
        }

        references
    }

    /// 현재 계층 경로 문자열 생성
    fn build_context_path(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref p) = self.current_state.part {
            parts.push(p.clone());
        }
        if let Some(ref c) = self.current_state.chapter {
            parts.push(c.clone());
        }
        if let Some(ref s) = self.current_state.section {
            parts.push(s.clone());
        }
        if let Some(ref ss) = self.current_state.subsection {
            parts.push(ss.clone());
        }
        if let Some(ref a) = self.current_state.article {
            parts.push(a.clone());
        }

        parts.join(" > ")
    }

    /// 조(Article) 블록 파싱
    fn parse_article_block(
        &mut self,
        lines: &[&str],
        start_idx: usize,
        base_metadata: &LegalMetadata,
    ) -> (LegalChunk, usize) {
        let mut article_lines = Vec::new();
        let mut current_idx = start_idx;
        let mut article_num = String::new();
        let mut article_branch: Option<String> = None;
        let mut article_title: Option<String> = None;

        // 첫 줄에서 조 정보 추출
        let first_line = lines[start_idx].trim();
        if let Some(caps) = RE_ARTICLE.captures(first_line) {
            article_num = caps[1].to_string();
            article_branch = caps.get(2).map(|m| m.as_str().to_string());
            article_title = caps.get(3).map(|m| m.as_str().to_string());

            let formatted = format_article_with_title(
                &article_num,
                article_branch.as_deref(),
                article_title.as_deref(),
            );
            self.current_state.set_article(formatted);
        }

        // 다음 조가 나올 때까지 수집
        while current_idx < lines.len() {
            let line = lines[current_idx].trim();

            // 다음 조 시작 감지
            if current_idx > start_idx && RE_ARTICLE.is_match(line) {
                break;
            }

            // 편/장/절/관 감지 및 상태 업데이트
            if let Some(caps) = RE_PART.captures(line) {
                let part = format!("제{}편 {}", &caps[1], caps[2].trim());
                self.current_state.set_part(part);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_CHAPTER.captures(line) {
                let chapter = format!("제{}장 {}", &caps[1], caps[2].trim());
                self.current_state.set_chapter(chapter);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SECTION.captures(line) {
                let section = format!("제{}절 {}", &caps[1], caps[2].trim());
                self.current_state.set_section(section);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SUBSECTION.captures(line) {
                let subsection = format!("제{}관 {}", &caps[1], caps[2].trim());
                self.current_state.set_subsection(subsection);
                current_idx += 1;
                continue;
            }

            article_lines.push(line);
            current_idx += 1;
        }

        // 청크 생성
        let content = article_lines.join("\n").trim().to_string();
        let references = self.extract_references(&content);

        let chunk_metadata = LegalMetadata {
            law_name: base_metadata.law_name.clone(),
            law_id: base_metadata.law_id.clone(),
            category: base_metadata.category.clone(),
            revision_date: base_metadata.revision_date.clone(),
            revision_number: base_metadata.revision_number.clone(),
            effective_date: base_metadata.effective_date.clone(),
            part: self.current_state.part.clone(),
            chapter: self.current_state.chapter.clone(),
            section: self.current_state.section.clone(),
            subsection: self.current_state.subsection.clone(),
            article_number: Some(article_num),
            article_title,
            paragraph_number: None,
            references,
            source_file: base_metadata.source_file.clone(),
            line_start: start_idx,
            line_end: current_idx.saturating_sub(1),
        };

        let chunk_id = self.generate_chunk_id(&content, &chunk_metadata);
        let token_count = self.estimate_tokens(&content);
        let context_path = self.build_context_path();

        let chunk = LegalChunk {
            id: chunk_id,
            content,
            metadata: chunk_metadata,
            chunk_type: ChunkType::Article,
            token_count,
            context_path,
            parent_chunk_id: None,
        };

        (chunk, current_idx)
    }

    /// 마크다운 파일 파싱
    pub fn parse_markdown<P: AsRef<Path>>(&mut self, filepath: P) -> Result<Vec<LegalChunk>, std::io::Error> {
        let content = fs::read_to_string(filepath.as_ref())?;
        let lines: Vec<&str> = content.lines().collect();

        // 메타데이터 헤더 파싱
        let (mut base_metadata, body_start) = self.parse_metadata_header(&lines);
        base_metadata.source_file = filepath
            .as_ref()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // 상태 초기화
        self.current_state.reset();

        let mut chunks = Vec::new();
        let mut current_idx = body_start;

        // 조 단위로 파싱
        while current_idx < lines.len() {
            let line = lines[current_idx].trim();

            // 빈 줄 스킵
            if line.is_empty() {
                current_idx += 1;
                continue;
            }

            // 편/장/절/관 업데이트
            if let Some(caps) = RE_PART.captures(line) {
                let part = format!("제{}편 {}", &caps[1], caps[2].trim());
                self.current_state.set_part(part);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_CHAPTER.captures(line) {
                let chapter = format!("제{}장 {}", &caps[1], caps[2].trim());
                self.current_state.set_chapter(chapter);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SECTION.captures(line) {
                let section = format!("제{}절 {}", &caps[1], caps[2].trim());
                self.current_state.set_section(section);
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SUBSECTION.captures(line) {
                let subsection = format!("제{}관 {}", &caps[1], caps[2].trim());
                self.current_state.set_subsection(subsection);
                current_idx += 1;
                continue;
            }

            // 조 파싱
            if RE_ARTICLE.is_match(line) {
                let (chunk, next_idx) = self.parse_article_block(&lines, current_idx, &base_metadata);
                if !chunk.content.is_empty() {
                    chunks.push(chunk);
                }
                current_idx = next_idx;
            } else {
                current_idx += 1;
            }
        }

        Ok(chunks)
    }

    /// 큰 조문을 항(Paragraph) 단위로 분할
    pub fn chunk_large_article(&self, chunk: LegalChunk) -> Vec<LegalChunk> {
        if chunk.token_count <= self.max_chunk_tokens {
            return vec![chunk];
        }

        let mut sub_chunks = Vec::new();
        let lines: Vec<&str> = chunk.content.lines().collect();
        let mut current_content = Vec::new();
        let mut current_paragraph: Option<String> = None;
        let parent_id = chunk.id.clone();

        for line in lines {
            // 항 시작 감지
            if let Some(caps) = RE_PARAGRAPH.captures(line) {
                if !current_content.is_empty() {
                    // 이전 항 저장
                    let content = current_content.join("\n").trim().to_string();
                    if !content.is_empty() {
                        let mut sub_meta = chunk.metadata.clone();
                        sub_meta.paragraph_number = current_paragraph.clone();

                        let sub_id = self.generate_chunk_id(&content, &sub_meta);
                        let token_count = self.estimate_tokens(&content);

                        sub_chunks.push(LegalChunk {
                            id: sub_id,
                            content,
                            metadata: sub_meta,
                            chunk_type: ChunkType::Paragraph,
                            token_count,
                            context_path: chunk.context_path.clone(),
                            parent_chunk_id: Some(parent_id.clone()),
                        });
                    }

                    current_content.clear();
                }

                current_content.push(line);

                // 원문자를 숫자로 변환
                let circled = caps[1].to_string();
                if let Some(first_char) = circled.chars().next() {
                    if let Some(num) = circled_to_number(first_char) {
                        current_paragraph = Some(num.to_string());
                    } else if circled.starts_with('(') && circled.ends_with(')') {
                        current_paragraph = Some(circled[1..circled.len()-1].to_string());
                    }
                }
            } else {
                current_content.push(line);
            }
        }

        // 마지막 항 저장
        if !current_content.is_empty() {
            let content = current_content.join("\n").trim().to_string();
            if !content.is_empty() {
                let mut sub_meta = chunk.metadata.clone();
                sub_meta.paragraph_number = current_paragraph;

                let sub_id = self.generate_chunk_id(&content, &sub_meta);
                let token_count = self.estimate_tokens(&content);

                sub_chunks.push(LegalChunk {
                    id: sub_id,
                    content,
                    metadata: sub_meta,
                    chunk_type: ChunkType::Paragraph,
                    token_count,
                    context_path: chunk.context_path.clone(),
                    parent_chunk_id: Some(parent_id),
                });
            }
        }

        if sub_chunks.is_empty() {
            vec![chunk]
        } else {
            sub_chunks
        }
    }
}

/// hex 인코딩 헬퍼
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let chunker = KoreanLegalChunker::new();
        
        // 한글 텍스트
        let korean = "이 규정은 유가증권시장의 상장에 관한 사항을 정한다.";
        let tokens = chunker.estimate_tokens(korean);
        assert!(tokens > 0);
        
        // 영문 텍스트
        let english = "This is a test sentence.";
        let eng_tokens = chunker.estimate_tokens(english);
        assert!(eng_tokens > 0);
    }

    #[test]
    fn test_generate_chunk_id() {
        let chunker = KoreanLegalChunker::new();
        let metadata = LegalMetadata {
            law_name: "유가증권시장 상장규정".to_string(),
            article_number: Some("1".to_string()),
            ..Default::default()
        };
        
        let id1 = chunker.generate_chunk_id("내용1", &metadata);
        let id2 = chunker.generate_chunk_id("내용2", &metadata);
        
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 16); // 8 bytes * 2 hex chars
    }

    #[test]
    fn test_extract_references() {
        let chunker = KoreanLegalChunker::new();
        
        let text = "「상법」 제42조제1항에 따라 제5조제2항을 적용한다.";
        let refs = chunker.extract_references(text);
        
        assert!(refs.len() >= 2);
        
        // 외부 참조 확인
        let external = refs.iter().find(|r| r.reference_type == "external");
        assert!(external.is_some());
        assert_eq!(external.unwrap().target_law, Some("상법".to_string()));
        
        // 내부 참조 확인
        let internal = refs.iter().find(|r| r.reference_type == "internal");
        assert!(internal.is_some());
    }

    #[test]
    fn test_build_context_path() {
        let mut chunker = KoreanLegalChunker::new();
        
        chunker.current_state.set_part("제1편 총칙".to_string());
        chunker.current_state.set_chapter("제1장 통칙".to_string());
        chunker.current_state.set_article("제1조(목적)".to_string());
        
        let path = chunker.build_context_path();
        assert_eq!(path, "제1편 총칙 > 제1장 통칙 > 제1조(목적)");
    }
}
