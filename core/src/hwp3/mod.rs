//! HWP 3.0 (한글 워드프로세서 3.x, 1996~2002) 텍스트 추출 파서.
//!
//! CFB(OLE2) 컨테이너가 아닌 단일 binary stream 포맷. 내부 구조 요약:
//!   30 byte signature
//! + 128 byte DocInfo  (compressed/encrypted 플래그 + InfoBlock 길이)
//! + 1008 byte DocSummary (제목/저자/날짜)
//! + InfoBlock (가변 — 폰트/스타일 메타데이터)
//! + Body  (compressed != 0 이면 raw deflate 압축)
//!
//! Body 는 paragraph 의 list. 각 paragraph 는 헤더 + LineInfos + (inline char
//! shapes) + char stream. char stream 은 hchar (u16 little-endian) 시퀀스로,
//! 1..31 (13 제외) 영역은 제어 문자다.
//!
//! 본 구현은 **텍스트 추출 전용** — 표/그림 레이아웃, 글자 속성 등은 무시한다.
//! 표 셀과 캡션, 머리말/꼬리말, 각주의 본문 텍스트는 재귀로 모아서 포함시킨다.
//!
//! 핵심 서브모듈:
//! - [`johab`] + `johab_table` — 조합형(Johab 유사 2바이트) → 유니코드 변환.
//!   5,893개 한자/기호 lookup 은 `scripts/extract_hwp3_johab_table.py` 로
//!   `reference/kkdoc/src/hwp3/johab-symbols.ts` 에서 추출한 정적 배열이다.
//! - [`reader`] — little-endian binary cursor.
//! - [`records`] — 헤더(signature/DocInfo/DocSummary) 고정 레이아웃.
//! - [`parser`] — paragraph list 재귀 파싱 + IR 변환.
//!
//! Ported from kkdoc (MIT): src/hwp3/*.ts

mod johab;
mod johab_table;
mod parser;
mod reader;
mod records;

pub use parser::{parse_hwp3_document, Hwp3Document, Hwp3Metadata};
pub use records::SIGNATURE_PREFIX;

/// 버퍼가 HWP3 시그니처("HWP Document File V3.00")로 시작하는지 검사한다.
/// HWP5(OLE2 CFB)/HWPX(ZIP)와 달리 HWP3 는 magic byte 가 아닌 ASCII 문자열
/// signature 라 앞부분 prefix 비교로 충분하다.
pub fn is_hwp3(buffer: &[u8]) -> bool {
    buffer.len() >= SIGNATURE_PREFIX.len() && &buffer[..SIGNATURE_PREFIX.len()] == SIGNATURE_PREFIX
}
