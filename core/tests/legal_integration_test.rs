//! Integration tests for Korean Legal Document Parser
//!
//! 실제 법률 마크다운 파일을 파싱하고 청킹 결과 검증

use mdm_core::legal::{KoreanLegalChunker, WeKnoraExporter};
use std::path::Path;

/// 실제 법률 마크다운 파일 파싱 테스트
#[test]
fn test_parse_real_legal_markdown_files() {
    let test_files = vec![
        "/Users/seunghan/krx_listing/krx_law/markdown_v2/210073336__유가증권시장_상장규정.md",
        "/Users/seunghan/krx_listing/krx_law/markdown_v2/210089573__코스닥시장_상장규정.md",
        "/Users/seunghan/krx_listing/krx_law/markdown_v2/210138546__시장감시규정.md",
        "/Users/seunghan/krx_listing/krx_law/markdown_v2/210073008__회원관리규정.md",
        "/Users/seunghan/krx_listing/krx_law/markdown_v2/210015705__분쟁조정규정.md",
    ];

    let mut chunker = KoreanLegalChunker::new();
    let mut total_chunks = 0;
    let mut total_tokens = 0;

    println!("\n=== 한국 법률 마크다운 파싱 테스트 ===\n");

    for filepath in &test_files {
        let path = Path::new(filepath);
        if !path.exists() {
            println!("⚠️ 파일 없음: {}", filepath);
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();

        match chunker.parse_markdown(filepath) {
            Ok(chunks) => {
                let chunk_count = chunks.len();
                let token_count: usize = chunks.iter().map(|c| c.token_count).sum();
                total_chunks += chunk_count;
                total_tokens += token_count;

                println!("✅ {}", filename);
                println!("   청크 수: {}", chunk_count);
                println!("   총 토큰: {}", token_count);

                // 첫 번째 청크 샘플 출력
                if let Some(first) = chunks.first() {
                    println!("   법령명: {}", first.metadata.law_name);
                    println!("   규정 ID: {}", first.metadata.law_id);
                    println!("   분류: {}", first.metadata.category);
                    println!("   첫 조문: {}", first.metadata.article_number.as_deref().unwrap_or("-"));
                }

                // 마지막 청크 정보
                if let Some(last) = chunks.last() {
                    println!("   마지막 조문: {}", last.metadata.article_number.as_deref().unwrap_or("-"));
                    println!("   컨텍스트: {}", &last.context_path.chars().take(50).collect::<String>());
                }

                println!();
            }
            Err(e) => {
                println!("❌ {} - 에러: {}", filename, e);
            }
        }
    }

    println!("=== 요약 ===");
    println!("총 청크 수: {}", total_chunks);
    println!("총 토큰 수: {}", total_tokens);
    println!("평균 청크당 토큰: {:.1}", total_tokens as f64 / total_chunks.max(1) as f64);

    assert!(total_chunks > 0, "적어도 하나의 청크가 생성되어야 함");
}

/// 청크 내용 품질 검증
#[test]
fn test_chunk_content_quality() {
    let filepath = "/Users/seunghan/krx_listing/krx_law/markdown_v2/210073336__유가증권시장_상장규정.md";
    let path = Path::new(filepath);

    if !path.exists() {
        println!("⚠️ 테스트 파일 없음, 스킵");
        return;
    }

    let mut chunker = KoreanLegalChunker::new();
    let chunks = chunker.parse_markdown(filepath).unwrap();

    println!("\n=== 청크 품질 검증: 유가증권시장 상장규정 ===\n");

    // 계층 구조 확인
    let mut has_part = false;
    let mut has_chapter = false;
    let mut has_section = false;
    let mut has_article = false;
    let mut reference_count = 0;

    for chunk in &chunks {
        if chunk.metadata.part.is_some() { has_part = true; }
        if chunk.metadata.chapter.is_some() { has_chapter = true; }
        if chunk.metadata.section.is_some() { has_section = true; }
        if chunk.metadata.article_number.is_some() { has_article = true; }
        reference_count += chunk.metadata.references.len();
    }

    println!("편(Part) 파싱: {}", if has_part { "✅" } else { "❌" });
    println!("장(Chapter) 파싱: {}", if has_chapter { "✅" } else { "❌" });
    println!("절(Section) 파싱: {}", if has_section { "✅" } else { "❌" });
    println!("조(Article) 파싱: {}", if has_article { "✅" } else { "❌" });
    println!("법조문 참조 추출: {} 개", reference_count);

    // 샘플 청크 출력
    println!("\n--- 샘플 청크 (처음 3개) ---");
    for (i, chunk) in chunks.iter().take(3).enumerate() {
        println!("\n[청크 {}]", i + 1);
        println!("ID: {}", chunk.id);
        println!("조: 제{}조", chunk.metadata.article_number.as_deref().unwrap_or("-"));
        println!("제목: {}", chunk.metadata.article_title.as_deref().unwrap_or("-"));
        println!("경로: {}", chunk.context_path);
        println!("토큰: {}", chunk.token_count);
        println!("내용 (100자): {}...", &chunk.content.chars().take(100).collect::<String>());
        println!("참조 수: {}", chunk.metadata.references.len());
    }

    assert!(has_article, "조(Article) 파싱 필수");
}

/// JSONL 내보내기 테스트
#[test]
fn test_jsonl_export() {
    let filepath = "/Users/seunghan/krx_listing/krx_law/markdown_v2/210015705__분쟁조정규정.md";
    let path = Path::new(filepath);

    if !path.exists() {
        println!("⚠️ 테스트 파일 없음, 스킵");
        return;
    }

    let mut chunker = KoreanLegalChunker::new();
    let chunks = chunker.parse_markdown(filepath).unwrap();

    let exporter = WeKnoraExporter::new();
    let output_path = "/tmp/legal_test_output.jsonl";

    let count = exporter.export_to_jsonl(&chunks, output_path).unwrap();

    println!("\n=== JSONL 내보내기 테스트 ===");
    println!("청크 수: {}", count);
    println!("출력 파일: {}", output_path);

    // 파일 내용 확인
    let content = std::fs::read_to_string(output_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    println!("JSONL 라인 수: {}", lines.len());

    if let Some(first_line) = lines.first() {
        // JSON 파싱 검증
        let json: serde_json::Value = serde_json::from_str(first_line).unwrap();
        assert!(json.get("id").is_some(), "ID 필드 필수");
        assert!(json.get("content").is_some(), "content 필드 필수");
        assert!(json.get("metadata").is_some(), "metadata 필드 필수");
        println!("✅ JSONL 형식 검증 통과");
    }

    assert_eq!(count, chunks.len());
}
