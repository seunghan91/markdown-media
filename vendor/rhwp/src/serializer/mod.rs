//! HWP 문서 직렬화 모듈
//!
//! Document IR을 HWP 5.0 바이너리 파일로 변환하는 기능을 제공한다.
//! `parser` 모듈의 역방향으로 동작한다.

pub mod body_text;
pub mod byte_writer;
pub mod cfb_writer;
pub mod control;
pub mod doc_info;
pub mod header;
pub mod mini_cfb;
pub mod record_writer;

pub use cfb_writer::{serialize_hwp, SerializeError};

use crate::model::document::Document;

// ---------------------------------------------------------------------------
// Trait 추상화: DocumentSerializer
// ---------------------------------------------------------------------------

/// 문서 직렬화 trait — Document IR을 바이트로 변환
pub trait DocumentSerializer {
    fn serialize(&self, doc: &Document) -> Result<Vec<u8>, SerializeError>;
}

/// HWP 5.0 바이너리 직렬화
pub struct HwpSerializer;

impl DocumentSerializer for HwpSerializer {
    fn serialize(&self, doc: &Document) -> Result<Vec<u8>, SerializeError> {
        serialize_hwp(doc)
    }
}

/// 현재 지원 포맷(HWP)으로 직렬화
pub fn serialize_document(doc: &Document) -> Result<Vec<u8>, SerializeError> {
    HwpSerializer.serialize(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSerializer;
    impl DocumentSerializer for MockSerializer {
        fn serialize(&self, _doc: &Document) -> Result<Vec<u8>, SerializeError> {
            Ok(vec![0xDE, 0xAD])
        }
    }

    #[test]
    fn test_mock_serializer() {
        let doc = Document::default();
        assert_eq!(MockSerializer.serialize(&doc).unwrap(), vec![0xDE, 0xAD]);
    }
}
