# Rust Parser Conversion Checklist

> Python 파서 로직을 Rust로 변환하기 위한 체크리스트
>
> 작성일: 2026-01-18
> 관련 프로젝트: markdown-media, krx_listing/krx_law

---

## Phase 0: 사전 준비

### 환경 설정
- [ ] Rust 개발 환경 확인 (`rustc --version`)
- [ ] `regex` crate 의존성 추가
- [ ] `lazy_static` crate 의존성 추가
- [ ] `sha2` crate 의존성 추가 (청크 ID 생성용)
- [ ] `serde` + `serde_json` crate 확인 (JSONL 출력용)

### 테스트 데이터 준비
- [ ] 샘플 HWP 파일 수집 (한글 텍스트 포함)
- [ ] 확장 제어 문자(0x16-0x1F) 포함 HWP 파일 확인
- [ ] 법률 문서 마크다운 샘플 수집
- [ ] Python 파서 출력 결과 저장 (비교용)

---

## Phase 1: 핵심 버그 수정 (P0)

### 1.1 확장 제어 문자 범위 수정
- [ ] `core/src/hwp/record.rs` 백업
- [ ] `EXTENDED_CTRL_CHARS` 상수 확장 (18개 → 31개)
  ```
  기존: 0x01-0x08, 0x0B, 0x0C, 0x0E, 0x0F, 0x10-0x15
  추가: 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F
  ```
- [ ] `extract_para_text()` 함수 수정
  - [ ] `0x16..=0x1F` 범위에서 14바이트 스킵하도록 변경
- [ ] `extract_para_text_with_positions()` 함수 동일 수정
- [ ] 단위 테스트 추가
  - [ ] `test_extended_ctrl_char_0x16_to_0x1f()`
- [ ] 기존 테스트 통과 확인

### 1.2 Surrogate 문자 처리 개선
- [ ] 유효하지 않은 codepoint 처리 로직 추가
- [ ] `\u{FFFD}` (replacement character) 대체 로직 구현
- [ ] 단위 테스트 추가
  - [ ] `test_invalid_surrogate_replacement()`

---

## Phase 2: 한국 법률 파서 모듈 생성 (P2)

### 2.1 모듈 구조 생성
- [ ] `core/src/legal/` 디렉토리 생성
- [ ] `core/src/legal/mod.rs` 생성
- [ ] `core/src/lib.rs`에 `pub mod legal;` 추가

### 2.2 타입 정의 (`types.rs`)
- [ ] `LegalHierarchy` enum 정의
  - [ ] Part, Chapter, Section, SubSection, Article, Paragraph, SubParagraph, Item
  - [ ] `korean_name()` 메서드 구현
- [ ] `LegalReference` struct 정의
  - [ ] target_law, target_article, reference_type, raw_text 필드
- [ ] `LegalMetadata` struct 정의
  - [ ] 법령 정보 필드 (law_name, law_id, category, revision_date 등)
  - [ ] 계층 구조 필드 (part, chapter, section, subsection)
  - [ ] 조항 정보 필드 (article_number, article_title, paragraph_number)
  - [ ] references, source_file, line_start, line_end 필드
- [ ] `LegalChunk` struct 정의
  - [ ] id, content, metadata, chunk_type, token_count, context_path, parent_chunk_id
- [ ] `ChunkType` enum 정의
  - [ ] Article, Paragraph, Definition
- [ ] Serde derive 매크로 추가 (Serialize, Deserialize)

### 2.3 정규식 패턴 (`patterns.rs`)
- [ ] `lazy_static!` 매크로로 정규식 패턴 정의
- [ ] `RE_PART` - `^제(\d+)편\s*(.*)$`
- [ ] `RE_CHAPTER` - `^제(\d+)장\s*(.*)$`
- [ ] `RE_SECTION` - `^제(\d+)절\s*(.*)$`
- [ ] `RE_SUBSECTION` - `^제(\d+)관\s*(.*)$`
- [ ] `RE_ARTICLE` - `^제(\d+)조(?:의(\d+))?(?:\(([^)]+)\))?`
- [ ] `RE_PARAGRAPH` - 원문자 패턴
- [ ] `RE_SUBPARAGRAPH` - `^(\d+)\.\s*`
- [ ] `RE_ITEM` - 한글 목 패턴
- [ ] `RE_LAW_REFERENCE` - 법률 참조 패턴
- [ ] `RE_INTERNAL_REFERENCE` - 내부 참조 패턴
- [ ] `RE_REVISION` - 개정 정보 패턴
- [ ] `CIRCLED_NUMBERS` HashMap (원문자→숫자)
- [ ] `KOREAN_ITEMS` 배열 (가나다라...)
- [ ] 패턴 테스트 케이스 작성

### 2.4 청커 구현 (`chunker.rs`)
- [ ] `KoreanLegalChunker` struct 정의
  - [ ] chunk_by_article, include_context, max_chunk_tokens, overlap_tokens 필드
  - [ ] _current_state HashMap (파싱 상태 추적)
- [ ] `impl KoreanLegalChunker`
  - [ ] `new()` 생성자
  - [ ] `estimate_tokens()` - 토큰 수 추정
  - [ ] `generate_chunk_id()` - SHA256 기반 ID 생성
  - [ ] `parse_metadata_header()` - 마크다운 헤더 파싱
  - [ ] `extract_references()` - 법조문 참조 추출
  - [ ] `build_context_path()` - 계층 경로 문자열 생성
  - [ ] `parse_article_block()` - 조(Article) 블록 파싱
  - [ ] `parse_markdown()` - 메인 파싱 함수
  - [ ] `chunk_large_article()` - 큰 조문 분할
- [ ] 단위 테스트 작성
  - [ ] `test_parse_article_pattern()`
  - [ ] `test_parse_hierarchy()`
  - [ ] `test_estimate_tokens()`
  - [ ] `test_chunk_large_article()`

### 2.5 내보내기 (`exporter.rs`)
- [ ] `WeKnoraExporter` struct 정의
- [ ] `export_for_embedding()` - 임베딩용 데이터 변환
- [ ] `export_to_jsonl()` - JSONL 파일 출력
- [ ] 단위 테스트 작성

---

## Phase 3: 통합 및 CLI (P3)

### 3.1 CLI 명령어 추가
- [ ] `core/src/main.rs` 수정
- [ ] `legal-chunk` 서브커맨드 추가
  - [ ] `--input` 입력 디렉토리
  - [ ] `--output` 출력 디렉토리
  - [ ] `--max-tokens` 최대 청크 토큰 수
  - [ ] `--single` 단일 파일 모드

### 3.2 Python 호환 인터페이스
- [ ] `pyo3` crate 추가 (선택사항)
- [ ] Python 바인딩 구현 (선택사항)

---

## Phase 4: 테스트 및 검증 (P4)

### 4.1 단위 테스트
- [x] `core/src/hwp/record.rs` 테스트 추가
- [x] `core/src/legal/` 모듈별 테스트 추가
- [x] `cargo test` 전체 통과 확인

### 4.2 통합 테스트
- [ ] 실제 HWP 파일로 텍스트 추출 테스트
- [ ] 실제 법률 마크다운으로 청킹 테스트
- [ ] Python 출력과 Rust 출력 비교

### 4.3 성능 테스트
- [ ] 대용량 파일 파싱 벤치마크
- [ ] Python 대비 성능 비교

---

## Phase 5: 문서화 및 배포 (P5)

### 5.1 문서화
- [ ] `README.md` 업데이트
- [ ] API 문서 생성 (`cargo doc`)
- [ ] 사용 예제 추가

### 5.2 배포
- [ ] `Cargo.toml` 버전 업데이트
- [ ] `cargo build --release` 빌드 확인
- [ ] krx_law 프로젝트에서 사용 테스트

---

## 진행 상태 요약

| Phase | 상태 | 완료율 |
|-------|------|--------|
| Phase 0 | ✅ 완료 | 100% |
| Phase 1 | ✅ 완료 | 100% |
| Phase 2 | ✅ 완료 | 100% |
| Phase 3 | 대기 | 0% |
| Phase 4 | 🔄 진행중 | 70% |
| Phase 5 | 대기 | 0% |

### 구현 완료 내역 (2026-01-18)

**Phase 1: 확장 제어 문자 버그 수정**
- `core/src/hwp/record.rs`: 0x16-0x1F 범위 14바이트 스킵 처리 추가
- 테스트 `test_extended_ctrl_char_0x16_to_0x1f()` 추가 및 통과

**Phase 2: 한국 법률 파서 모듈**
- `core/src/legal/mod.rs`: 모듈 진입점
- `core/src/legal/types.rs`: LegalHierarchy, LegalReference, LegalMetadata, LegalChunk 등 타입 정의
- `core/src/legal/patterns.rs`: 정규식 패턴 (RE_PART, RE_CHAPTER, RE_ARTICLE 등)
- `core/src/legal/chunker.rs`: KoreanLegalChunker 청킹 로직
- `core/src/legal/exporter.rs`: WeKnoraExporter JSONL 내보내기
- 23개 테스트 모두 통과

**Phase 4: 테스트 결과**
- lib 테스트: 84개 통과
- main 테스트: 46개 통과
- legal 모듈 테스트: 23개 통과
- 전체 `cargo test` 통과 확인

---

## 참고 자료

- Python 파서: `/Users/seunghan/krx_listing/krx_law/legal_chunker.py`
- Python HWP 변환기: `/Users/seunghan/krx_listing/tmp/markdown-media/converters/hwp_converter.py`
- Rust 레코드 파서: `/Users/seunghan/krx_listing/tmp/markdown-media/core/src/hwp/record.rs`
- Rust HWP 파서: `/Users/seunghan/krx_listing/tmp/markdown-media/core/src/hwp/parser.rs`
