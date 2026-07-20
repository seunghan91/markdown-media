//! HWPML 2.x (XML 기반 HWP) 파서 — 시그니처 감지 + XML → IRBlock 변환.
//!
//! HWPML 은 한컴이 내보내는 XML 기반 HWP 문서 포맷으로, `.hml` 확장자를
//! 쓰거나 `.hwp` 로 잘못 라벨링된 채 유통되는 경우가 흔하다. HWP 5.x(OLE2
//! CFB)/HWPX(ZIP)와 달리 단일 XML 문서다.
//!
//! Ported from kkdoc (MIT): src/hwpml/parser.ts

mod parser;

pub use parser::{parse_hwpml_document, HwpmlDocument, HwpmlMetadata};

/// 버퍼가 HWPML 2.x 루트 요소(`<HWPML ...>`)로 시작하는지 검사한다.
///
/// XML 선언(`<?xml ...?>`)/DOCTYPE/주석을 건너뛰고 실제 루트 요소를 확인하며,
/// UTF-8/UTF-16 BOM 이 붙은 파일도 처리한다. 탐지 목적이므로 앞부분 일부
/// (최대 8KB)만 디코드한다 — [`parse_hwpml_document`]가 강제하는 50MB 상한
/// 파일에서도 저비용으로 호출 가능.
pub fn is_hwpml(buffer: &[u8]) -> bool {
    const PREFIX_BYTES: usize = 8192;
    let prefix = &buffer[..buffer.len().min(PREFIX_BYTES)];
    let text = parser::decode_hwpml_bytes(prefix);
    let text = text.trim_start_matches('\u{feff}');
    let stripped = parser::strip_prolog(text);
    stripped.trim_start().starts_with("<HWPML")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hwpml_root() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><HWPML Version="2.8"></HWPML>"#;
        assert!(is_hwpml(xml.as_bytes()));
    }

    #[test]
    fn rejects_other_xml() {
        let xml = r#"<?xml version="1.0"?><root><child/></root>"#;
        assert!(!is_hwpml(xml.as_bytes()));
    }

    #[test]
    fn detects_with_doctype_and_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(
            b"<?xml version=\"1.0\"?>\n<!DOCTYPE HWPML SYSTEM \"hwpml.dtd\">\n<HWPML Version=\"2.8\"/>",
        );
        assert!(is_hwpml(&bytes));
    }

    #[test]
    fn rejects_truncated_buffer() {
        assert!(!is_hwpml(b"<?xml"));
    }
}
