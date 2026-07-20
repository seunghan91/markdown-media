//! HWP 3.0 상용 조합형(Johab-like 2-byte) → 유니코드 디코더.
//!
//! 한국어 한글은 cho/jung/jong 비트 분해로 0xAC00 한글 음절 영역에 직접 매핑되고,
//! 한자/기호 등 그 외 영역은 [`johab_table::JOHAB_SYMBOLS`] lookup table 로 처리한다.
//! 매핑되지 않는 코드는 `None` 을 반환한다 (호출자가 조용히 skip).
//!
//! Ported from kkdoc (MIT): src/hwp3/johab.ts

use super::johab_table::JOHAB_SYMBOLS;

// 인덱스 → 자모 위치. -1 은 invalid (KS X 1001 johab 정의).
const CHO_MAP: [i8; 32] = [
    -1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1,
];
const JUNG_MAP: [i8; 32] = [
    -1, -1, -1, 0, 1, 2, 3, 4, -1, -1, 5, 6, 7, 8, 9, 10, -1, -1, 11, 12, 13, 14, 15, 16, -1, -1,
    17, 18, 19, 20, -1, -1,
];
const JONG_MAP: [i8; 32] = [
    -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, -1, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, 27, -1, -1,
];

/// `JOHAB_SYMBOLS` (key 로 정렬된 배열) 에서 key 이진 탐색.
fn lookup_symbol(ch: u16) -> Option<u32> {
    JOHAB_SYMBOLS
        .binary_search_by_key(&ch, |&(k, _)| k)
        .ok()
        .map(|idx| JOHAB_SYMBOLS[idx].1)
}

/// HWP3 hchar (u16) → 유니코드 코드포인트. 매핑 실패 시 `None`.
///
/// 매핑 실패 케이스를 `?` 로 fallback 시키면 검색 인덱스에 noise 가 누적된다
/// (특히 메타 컨트롤이 가득한 paragraph 가 `???` 시퀀스를 생산). 호출자가
/// unmapped 를 식별해서 silently skip 할 수 있도록 `None` 을 반환한다.
pub fn decode_johab(ch: u16) -> Option<u32> {
    // ASCII 영역 — 1바이트 직접 사용
    if ch < 0x80 {
        return Some(ch as u32);
    }

    // 조합형 한글 (상위 비트 1): cho 5b | jung 5b | jong 5b
    if ch >= 0x8000 {
        let cho_idx = ((ch >> 10) & 0x1f) as usize;
        let jung_idx = ((ch >> 5) & 0x1f) as usize;
        let jong_idx = (ch & 0x1f) as usize;

        let cho = CHO_MAP[cho_idx];
        let jung = JUNG_MAP[jung_idx];
        let mut jong = JONG_MAP[jong_idx];

        if cho != -1 && jung != -1 {
            if jong == -1 {
                jong = 0;
            }
            // 0xAC00 + (cho * 21 * 28) + (jung * 28) + jong
            return Some(0xac00 + cho as u32 * 588 + jung as u32 * 28 + jong as u32);
        }

        // 한자/기호: lookup table
        return lookup_symbol(ch);
    }

    // 사적 graphic char 영역 (0x0080~0x7FFF) — rhwp decode_hwp3_extra 포팅
    decode_hwp3_extra(ch)
}

/// HWP3 사적 graphic char (0x0080~0x7FFF) → 유니코드. 매핑 없으면 `None`.
///
/// rhwp 는 한컴 PUA(U+F03C5 등)를 보존하고 렌더러가 표시값으로 확장하지만,
/// 이 포팅은 렌더러 없이 markdown 으로 직행하므로 한컴오피스 표시값을 직접
/// 방출한다. 관계도 선문자(0x301E/0x3024/0x3027)는 표준 근사가 없어 미매핑 유지.
fn decode_hwp3_extra(ch: u16) -> Option<u32> {
    // 로마숫자 대문자 Ⅰ~Ⅹ ("Ⅰ. 사업개요" 류 장 제목)
    if (0x3590..=0x3599).contains(&ch) {
        return Some(0x2160 + (ch - 0x3590) as u32);
    }
    // 원문자 ①~⑩
    if (0x36e7..=0x36f0).contains(&ch) {
        return Some(0x2460 + (ch - 0x36e7) as u32);
    }
    match ch {
        0x0081 => Some(0x201c), // 왼쪽 큰따옴표
        0x0082 => Some(0x201d), // 오른쪽 큰따옴표
        0x301c => Some(0x2501), // ━ 굵은 가로선 (rhwp: U+F080F, 표시값 직행)
        0x303d => Some(0x25a0), // ■ (rhwp: U+F0827, 표시값 직행)
        0x3366 => Some(0x25a1), // □ 글머리 (rhwp: U+F03C5, 한컴 표시값 직행)
        0x3404 => Some(0x2024), // 한 점 리더
        0x3441 => Some(0x25a0), // ■
        0x3446 => Some(0x2192), // → 오른쪽 화살표
        0x35e1 => Some(0x2500), // ─ 상자 그리기 가로선
        0x3479 => Some(0x25b7), // ▷
        0x347a => Some(0x25b6), // ▶
        _ => None,
    }
}

/// HWP3 hchar stream (u16 LE 순서) 를 string 으로 디코딩.
///
/// DocSummary 의 56 hchar (112 byte) 영역에 사용. 본문 char stream 과 같은 단위인데
/// 그 영역은 ASCII 도 high byte 0 으로 padding 되어 있다 ("C\0r\0..."). byte 단위
/// 디코딩으로 처리하면 NUL 에서 break 되어 첫 글자만 남으므로, hchar 단위 LE u16 로
/// 읽고 그 값이 0 이면 종료한다.
pub fn decode_hchar_string(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let ch = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if ch == 0 {
            break;
        }
        if let Some(cp) = decode_johab(ch) {
            if let Some(c) = char::from_u32(cp) {
                out.push(c);
            }
        }
        i += 2;
    }
    out
}

/// HWP3 byte sequence (1바이트 ASCII < 0x80, 2바이트 johab >= 0x80) 를 string 으로 디코딩.
/// NUL byte 만나면 종료. link_print_file/description 같은 짧은 byte string 영역에 사용.
#[allow(dead_code)]
pub fn decode_hwp3_string(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b1 = bytes[i];
        if b1 == 0 {
            break;
        }
        if b1 < 0x80 {
            out.push(b1 as char);
            i += 1;
        } else if i + 1 < bytes.len() {
            let ch = ((b1 as u16) << 8) | bytes[i + 1] as u16;
            if let Some(cp) = decode_johab(ch) {
                if let Some(c) = char::from_u32(cp) {
                    out.push(c);
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(decode_johab(b'A' as u16), Some(b'A' as u32));
    }

    #[test]
    fn hangul_syllable_composition() {
        // 가 = cho 0 (ㄱ), jung 0 (ㅏ), jong 0 → 0x8861 in johab-ish combining form.
        // cho_idx maps CHO_MAP index 2 -> 0, jung_idx maps JUNG_MAP index 3 -> 0.
        let ch = (0x8000) | (2u16 << 10) | (3u16 << 5) | 1u16; // jong_idx=1 -> JONG_MAP[1]=0
        assert_eq!(decode_johab(ch), Some(0xac00)); // 가
    }

    #[test]
    fn symbol_table_lookup() {
        // 0x8441 -> U+3000 (ideographic space), first entry of johab-symbols.ts
        assert_eq!(decode_johab(0x8441), Some(0x3000));
    }

    #[test]
    fn unmapped_returns_none() {
        assert_eq!(decode_johab(0x0100), None);
    }

    #[test]
    fn extra_roman_numerals() {
        assert_eq!(decode_johab(0x3590), Some(0x2160)); // Ⅰ
        assert_eq!(decode_johab(0x3593), Some(0x2163)); // Ⅳ
    }

    #[test]
    fn extra_circled_numbers_and_punct() {
        assert_eq!(decode_johab(0x36e7), Some(0x2460)); // ①
        assert_eq!(decode_johab(0x0081), Some(0x201c)); // “
        assert_eq!(decode_johab(0x0082), Some(0x201d)); // ”
        assert_eq!(decode_johab(0x3446), Some(0x2192)); // →
        assert_eq!(decode_johab(0x3366), Some(0x25a1)); // □
        assert_eq!(decode_johab(0x3441), Some(0x25a0)); // ■
    }

    #[test]
    fn hchar_string_stops_at_nul() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(b'C' as u16).to_le_bytes());
        bytes.extend_from_slice(&(b'r' as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(b'X' as u16).to_le_bytes()); // should not appear
        assert_eq!(decode_hchar_string(&bytes), "Cr");
    }
}
