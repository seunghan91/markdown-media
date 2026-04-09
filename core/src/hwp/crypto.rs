//! HWP 배포용(distribution / 열람 제한) 문서 복호화.
//!
//! 배포용 HWP 파일은 일반 `BodyText/Section{N}` 대신 `ViewText/Section{N}`
//! 스트림에 암호화된 본문을 저장한다. 각 ViewText 스트림의 첫 레코드
//! `HWPTAG_DISTRIBUTE_DOC_DATA` (tag 0x1C) 의 256바이트 payload 에서 AES-128
//! 키를 추출한 뒤, 나머지 바이트를 AES-128-ECB 로 복호화한다.
//!
//! 복호화 파이프라인 (rhwp MIT 알고리즘 + FIPS-197 AES):
//!   1. 레코드 헤더(4B) 파싱 → payload 오프셋/크기 확인
//!   2. 256B payload 의 MSVC LCG + XOR 복호화 (첫 4B 는 LCG seed, 보존)
//!   3. `offset = 4 + (decrypted[0] & 0x0F)` 위치에서 16B AES 키 추출
//!   4. payload 뒤의 바이트를 16B 블록 단위로 AES-128-ECB 복호화
//!   5. FileHeader `compressed` 플래그가 켜져 있으면 raw deflate 로 푼다
//!
//! 참조:
//! - `reference/kordoc/src/hwp5/crypto.ts` (순수 JS 포팅)
//! - `reference/kordoc/src/hwp5/aes.ts` (AES FIPS-197)
//! - rhwp (MIT) `src/parser/crypto.rs`

use crate::utils::bounded_io::{decompress_raw_deflate_limited, MAX_HWP_SECTION};
use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use aes::Aes128;
use std::io;

/// `HWPTAG_DISTRIBUTE_DOC_DATA` = HWPTAG_BEGIN (0x10) + 12 = 0x1C
const TAG_DISTRIBUTE_DOC_DATA: u16 = 0x10 + 12;

/// 256B payload 복호화에 쓰이는 LCG 시퀀스 길이와 동일
const PAYLOAD_LEN: usize = 256;

// ── MSVC CRT rand() 호환 LCG ─────────────────────────────────────────────────

/// Microsoft Visual C++ CRT `rand()` 구현을 그대로 재현하는 LCG.
/// 상수 `214013 * seed + 2531011` 은 MSVC 런타임의 문서화된 상수로,
/// HWP 배포용 복호화 시퀀스가 이 동작에 의존한다.
struct MsvcLcg {
    seed: u32,
}

impl MsvcLcg {
    fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// 0..=0x7FFF 범위의 다음 난수를 반환.
    fn next(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
        (self.seed >> 16) & 0x7fff
    }
}

// ── 256B payload 복호화 ──────────────────────────────────────────────────────

/// `HWPTAG_DISTRIBUTE_DOC_DATA` 레코드의 256바이트 payload 를 LCG+XOR 로
/// 복호화한다.
///
/// 구조:
/// - `bytes[0..4]`: LCG seed (u32 LE) — 복호화 결과에서도 원본 그대로 유지
/// - `bytes[4..256]`: XOR 암호화된 본체. `n = (lcg.next() & 0xF) + 1` 바이트마다
///   키(`lcg.next() & 0xFF`)가 갱신된다. rhwp 와의 호환을 위해 `i ∈ [0,4)` 구간
///   에서도 `n` 카운터는 소비하되 XOR 은 건너뛴다.
fn decrypt_distribute_payload(payload: &[u8]) -> io::Result<[u8; PAYLOAD_LEN]> {
    if payload.len() < PAYLOAD_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "distribution payload < 256 bytes",
        ));
    }

    let seed = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let mut lcg = MsvcLcg::new(seed);

    let mut out = [0u8; PAYLOAD_LEN];
    out.copy_from_slice(&payload[..PAYLOAD_LEN]);

    let mut i: usize = 0;
    let mut n: u32 = 0;
    let mut key: u8 = 0;

    while i < PAYLOAD_LEN {
        if n == 0 {
            key = (lcg.next() & 0xff) as u8;
            n = (lcg.next() & 0x0f) + 1;
        }
        if i >= 4 {
            out[i] ^= key;
        }
        i += 1;
        n -= 1;
    }

    Ok(out)
}

/// 복호화된 256B payload 에서 AES-128 키(16B) 를 추출.
/// 키는 `offset = 4 + (decrypted[0] & 0x0F)` 위치에 저장된다.
fn extract_aes_key(decrypted: &[u8; PAYLOAD_LEN]) -> io::Result<[u8; 16]> {
    let offset = 4 + (decrypted[0] & 0x0f) as usize;
    if offset + 16 > PAYLOAD_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AES key offset out of range",
        ));
    }
    let mut key = [0u8; 16];
    key.copy_from_slice(&decrypted[offset..offset + 16]);
    Ok(key)
}

// ── 레코드 헤더 파싱 ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct RecordHeader {
    tag_id: u16,
    size: usize,
    header_size: usize,
}

/// HWP 레코드 헤더 파싱. 표준 4B 헤더 + `size == 0xFFF` 시 확장 4B 크기 필드.
fn parse_record_header(data: &[u8], offset: usize) -> io::Result<RecordHeader> {
    if offset + 4 > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "record header truncated",
        ));
    }
    let header = u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    let tag_id = (header & 0x3ff) as u16;
    let mut size = ((header >> 20) & 0xfff) as usize;
    let mut header_size = 4usize;

    if size == 0xfff {
        if offset + 8 > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "extended record size truncated",
            ));
        }
        size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        header_size = 8;
    }

    Ok(RecordHeader {
        tag_id,
        size,
        header_size,
    })
}

// ── AES-128 ECB 복호화 ────────────────────────────────────────────────────────

fn aes128_ecb_decrypt(ciphertext: &[u8], key: &[u8; 16]) -> Vec<u8> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut out = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext.chunks_exact(16) {
        let mut block = *GenericArray::from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    out
}

// ── 공개 API ─────────────────────────────────────────────────────────────────

/// `ViewText/Section{N}` 스트림의 원본 바이트를 받아 복호화된 레코드 데이터를
/// 돌려준다. `compressed` 가 true 면 AES 복호화 후 raw deflate 도 같이 푼다.
///
/// 실패 유형:
/// - 첫 레코드 태그가 `DISTRIBUTE_DOC_DATA` 가 아님 (= 배포용이 아님)
/// - payload/key offset 이 버퍼 범위를 벗어남
/// - 암호화 본문 길이가 16B 미만
/// - `MAX_HWP_SECTION` 초과 decompress 시도
pub fn decrypt_view_text(raw: &[u8], compressed: bool) -> io::Result<Vec<u8>> {
    // 1. 첫 레코드 헤더 파싱
    let rec = parse_record_header(raw, 0)?;
    if rec.tag_id != TAG_DISTRIBUTE_DOC_DATA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "first record is not DISTRIBUTE_DOC_DATA (got tag {:#x})",
                rec.tag_id
            ),
        ));
    }
    if rec.size < PAYLOAD_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "distribution payload record < 256 bytes",
        ));
    }
    let payload_start = rec.header_size;
    let payload_end = payload_start + rec.size;
    if payload_end > raw.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "distribution payload truncated",
        ));
    }

    // 2. 256B payload → MSVC LCG + XOR 복호화
    let payload = &raw[payload_start..payload_start + PAYLOAD_LEN];
    let decrypted_payload = decrypt_distribute_payload(payload)?;

    // 3. AES-128 키 추출
    let aes_key = extract_aes_key(&decrypted_payload)?;

    // 4. payload 뒤의 바이트를 AES-128 ECB 로 푼다 (16B 블록 정렬)
    let encrypted = &raw[payload_end..];
    if encrypted.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no encrypted body after distribution header",
        ));
    }
    let aligned_len = encrypted.len() - (encrypted.len() % 16);
    if aligned_len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "encrypted body < 16 bytes",
        ));
    }
    let decrypted = aes128_ecb_decrypt(&encrypted[..aligned_len], &aes_key);

    // 5. 압축 해제 (compressed 플래그 시). raw deflate 가 실패하면 그대로 반환.
    if compressed {
        match decompress_raw_deflate_limited(&decrypted, MAX_HWP_SECTION) {
            Ok(unzipped) if !unzipped.is_empty() => Ok(unzipped),
            _ => Ok(decrypted),
        }
    } else {
        Ok(decrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msvc_lcg_matches_reference() {
        // MSVC CRT rand() with seed 1 — documented reference sequence:
        // https://learn.microsoft.com/cpp/c-runtime-library/reference/rand
        let mut lcg = MsvcLcg::new(1);
        assert_eq!(lcg.next(), 41);
        assert_eq!(lcg.next(), 18467);
        assert_eq!(lcg.next(), 6334);
        assert_eq!(lcg.next(), 26500);
        assert_eq!(lcg.next(), 19169);
    }

    #[test]
    fn payload_decrypt_preserves_seed_bytes() {
        // First 4 bytes must survive round-trip unchanged.
        let mut payload = [0u8; 300];
        payload[0] = 0x78;
        payload[1] = 0x56;
        payload[2] = 0x34;
        payload[3] = 0x12;
        // Fill the rest with known pattern
        for (i, b) in payload.iter_mut().enumerate().skip(4) {
            *b = (i & 0xff) as u8;
        }
        let decrypted = decrypt_distribute_payload(&payload).unwrap();
        assert_eq!(&decrypted[..4], &payload[..4]);
    }

    #[test]
    fn payload_too_short_errors() {
        let short = [0u8; 100];
        assert!(decrypt_distribute_payload(&short).is_err());
    }

    #[test]
    fn key_extraction_respects_offset() {
        let mut payload = [0u8; PAYLOAD_LEN];
        payload[0] = 0x05; // offset = 4 + 5 = 9
        for i in 9..25 {
            payload[i] = (i - 9) as u8;
        }
        let key = extract_aes_key(&payload).unwrap();
        for (i, b) in key.iter().enumerate() {
            assert_eq!(*b, i as u8);
        }
    }

    #[test]
    fn record_header_parses_standard_form() {
        // tag=0x1C (DISTRIBUTE_DOC_DATA), size=256, level=0
        // header layout: bits [0..10]=tag, [10..20]=level, [20..32]=size
        // size=256 → bits [20..32] = 256
        let header: u32 = (256u32 << 20) | 0x1C;
        let mut data = [0u8; 10];
        data[..4].copy_from_slice(&header.to_le_bytes());
        let rec = parse_record_header(&data, 0).unwrap();
        assert_eq!(rec.tag_id, 0x1C);
        assert_eq!(rec.size, 256);
        assert_eq!(rec.header_size, 4);
    }

    #[test]
    fn decrypt_view_text_rejects_non_distribution() {
        // First record with a different tag id
        let header: u32 = (256u32 << 20) | 0x42;
        let mut data = vec![0u8; 300];
        data[..4].copy_from_slice(&header.to_le_bytes());
        let err = decrypt_view_text(&data, false).unwrap_err();
        assert!(err.to_string().contains("DISTRIBUTE_DOC_DATA"));
    }
}
