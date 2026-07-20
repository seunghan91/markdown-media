//! HWP3 fixed-size record definitions.
//!
//! 모두 little-endian. 텍스트 추출에 필요한 필드만 명시적으로 노출하고,
//! 레이아웃 전용 필드(margins, tab stops 등)는 단순 skip 한다.
//!
//! Ported from kkdoc (MIT): src/hwp3/records.ts

use super::johab::decode_hchar_string;
use super::reader::Hwp3Reader;
use std::io;

/// HWP3 file signature: "HWP Document File V3.00 " (24 bytes) + 6-byte tail.
/// 총 30 byte. 일부 실제 파일은 첫 30 byte 이내에 NUL pad 가 끼어있는 경우가 있어
/// 앞 24 byte 의 ASCII signature 만 strict 비교하고 나머지는 advisory.
pub const SIGNATURE_PREFIX: &[u8] = b"HWP Document File V3.00";
pub const SIGNATURE_LEN: usize = 30;

/// rhwp 와 같은 고정 byte size. cursor 가 정확히 이 만큼 advance 한다.
pub const DOC_INFO_SIZE: usize = 128;
pub const DOC_SUMMARY_SIZE: usize = 9 * 112; // 9 fields x 112 bytes (56 hchar each)

#[derive(Debug, Clone, Default)]
pub struct Hwp3Header {
    /// DocInfo 의 압축 플래그 (0 이 아니면 InfoBlock 이후 raw deflate 압축).
    pub compressed: u8,
    /// DocInfo 의 encrypted 플래그 (0 이 아니면 본문 암호화 -> 복호화 못함).
    pub encrypted: u16,
    /// InfoBlock 길이 (DocSummary 뒤 가변 길이 metadata).
    pub info_block_length: u16,
    /// DocSummary 에서 추출한 메타데이터.
    pub title: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

/// 헤더 파싱: 30 byte signature + 128 byte DocInfo + 1008 byte DocSummary.
/// 호출 시 reader 위치는 0 이어야 하고, 반환 후엔 InfoBlock 시작점.
pub fn read_header(reader: &mut Hwp3Reader) -> io::Result<Hwp3Header> {
    // signature 30 byte — strict prefix check
    let sig = reader.read_bytes(SIGNATURE_LEN)?;
    if &sig[..SIGNATURE_PREFIX.len()] != SIGNATURE_PREFIX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HWP3: invalid file signature",
        ));
    }

    // DocInfo 128 byte — 텍스트 추출에 필요한 3개 필드 외엔 skip.
    // 절대 offset 기준 (DocInfo 시작점 = doc_info_start):
    //   encrypted          : offset 96..97  (u16)  — 0 이 아니면 본문 암호 보호
    //   compressed         : offset 124    (u8)   — 0 이 아니면 InfoBlock 이후 raw deflate
    //   info_block_length  : offset 126..127 (u16) — 가변 InfoBlock 길이
    let doc_info_start = reader.position();
    reader.skip(96)?;
    let encrypted = reader.read_u16()?; // 96..97
    reader.skip(124 - 98)?; // -> 124
    let compressed = reader.read_u8()?; // 124
    reader.skip(1)?; // sub_revision (125)
    let info_block_length = reader.read_u16()?; // 126..127
    // DocInfo 끝까지 정확히 advance — sanity
    if reader.position() != doc_info_start + DOC_INFO_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "HWP3: DocInfo size mismatch (got {}, expected {})",
                reader.position() - doc_info_start,
                DOC_INFO_SIZE
            ),
        ));
    }

    // DocSummary 1008 byte — title/subject/author/date 만 추출, 나머지(keywords, etc) skip.
    // DocSummary 의 string 은 56 hchar x 2 byte 로 구성 — byte 단위가 아닌 u16 hchar 단위로
    // 디코딩해야 ASCII 문자가 high byte 0 padding 으로 인해 잘리지 않는다.
    let summary_start = reader.position();
    let title = decode_hchar_string(reader.read_bytes(112)?);
    let subject = decode_hchar_string(reader.read_bytes(112)?);
    let author = decode_hchar_string(reader.read_bytes(112)?);
    let date = decode_hchar_string(reader.read_bytes(112)?);
    // 나머지 (keywords x2 + etc x3 = 5 x 112 = 560 byte) skip
    reader.skip(5 * 112)?;
    if reader.position() != summary_start + DOC_SUMMARY_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HWP3: DocSummary size mismatch",
        ));
    }

    Ok(Hwp3Header {
        compressed,
        encrypted,
        info_block_length,
        title,
        subject,
        author,
        date,
    })
}
