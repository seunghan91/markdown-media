#![allow(
    // MDM 추가: 벤더 코드 무수정 원칙 — rhwp upstream이 core MSRV(1.75)보다
    // 새 API/관용구를 쓰는 지점은 여기서 흡수한다 (재동기화 시 유지).
    clippy::manual_div_ceil,
    clippy::incompatible_msrv,
    clippy::wrong_self_convention,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::enum_variant_names,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::upper_case_acronyms,
    clippy::wildcard_imports,
    non_camel_case_types,
    non_snake_case,
    unexpected_cfgs,
    dead_code,
    unused_imports,
    unused_variables,
)]

// tracing 스텁 매크로 (converter/parser 모듈보다 먼저 정의해야 하위 모듈에서 사용 가능)
#[allow(unused_macros)]
macro_rules! debug {
    ($($arg:tt)+) => {};
}
#[allow(unused_macros)]
macro_rules! info {
    ($($arg:tt)+) => {};
}
#[allow(unused_macros)]
macro_rules! warn {
    ($($arg:tt)+) => {};
}
#[allow(unused_macros)]
macro_rules! error {
    ($($arg:tt)+) => {};
}

pub mod converter;
pub mod parser;

mod imports {
    pub use std::{
        borrow::ToOwned,
        boxed::Box,
        collections::{BTreeMap, BTreeSet, VecDeque},
        string::{String, ToString},
        vec::Vec,
    };
}

pub use embedded_io::Read;

// ---------------------------------------------------------------------------
// MDM 래퍼 — 이 함수만 MDM 작성분이고, 위/하위 모듈은 rhwp v0.7.2 vendored
// 사본(바이트 동일, vendor/rhwp/src/wmf 참조)이다. 벤더 코드는 수정하지 않고
// 업스트림 재동기화 시 이 블록만 보존한다.
// ---------------------------------------------------------------------------

/// WMF 바이트를 자립 SVG 문서로 변환한다.
///
/// rhwp의 `renderer/svg.rs::convert_wmf_to_svg` 래퍼와 동일한 진입이되,
/// 퇴화 SVG 가드를 더한다: 변환은 성공했지만 그릴 수 있는 요소가 없거나
/// (자식 element 0개) viewBox 넓이가 0인 결과는 None으로 강등해 호출자가
/// 원본 바이트 보존 폴백을 타게 한다. 미지 레코드에서 컨버터가 Err을
/// 내는 경우도 동일하게 None이다.
pub fn convert_wmf_to_svg(data: &[u8]) -> Option<Vec<u8>> {
    use converter::{SVGPlayer, WMFConverter};
    let player = SVGPlayer::new();
    let converter = WMFConverter::new(data, player);
    let svg = converter.run().ok()?;

    let text = std::str::from_utf8(&svg).ok()?;
    let doc = roxmltree::Document::parse(text).ok()?;
    let root = doc.root_element();
    if !root.children().any(|c| c.is_element()) {
        return None;
    }
    if let Some(vb) = root.attribute("viewBox") {
        let nums: Vec<f64> = vb
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() == 4 && (nums[2] <= 0.0 || nums[3] <= 0.0) {
            return None;
        }
    }
    Some(svg)
}

#[cfg(test)]
mod mdm_wrapper_tests {
    use super::convert_wmf_to_svg;

    /// 표준(placeable 아님) WMF: METAHEADER(18B) + SETWINDOWEXT +
    /// RECTANGLE + EOF. fixtures/make_docx_fixtures.py 의 생성 로직과 동형.
    /// record = [size u32(총 워드 수)] [function u16] [params u16...]
    fn push_record(w: &mut Vec<u8>, function: u16, params: &[u16]) {
        let size = (params.len() + 3) as u32; // size(2워드) + function(1워드) + params
        w.extend_from_slice(&size.to_le_bytes());
        w.extend_from_slice(&function.to_le_bytes());
        for x in params {
            w.extend_from_slice(&x.to_le_bytes());
        }
    }

    /// METAHEADER: type=1(disk), headerSize=9, version=0x0300,
    /// fileSize(4B, 나중에 채움), numObjects=0, maxRecord(4B), noParameters=0
    fn push_header(w: &mut Vec<u8>) {
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&9u16.to_le_bytes());
        w.extend_from_slice(&0x0300u16.to_le_bytes());
        w.extend_from_slice(&0u32.to_le_bytes()); // fileSize placeholder
        w.extend_from_slice(&0u16.to_le_bytes());
        w.extend_from_slice(&0u32.to_le_bytes()); // maxRecord placeholder
        w.extend_from_slice(&0u16.to_le_bytes());
    }

    fn finish(mut w: Vec<u8>) -> Vec<u8> {
        let words = (w.len() / 2) as u32;
        w[6..10].copy_from_slice(&words.to_le_bytes());
        w
    }

    fn minimal_wmf_with_rect() -> Vec<u8> {
        let mut w: Vec<u8> = Vec::new();
        push_header(&mut w);
        push_record(&mut w, 0x020C, &[200, 300]); // SETWINDOWEXT (y, x)
        push_record(&mut w, 0x020B, &[0, 0]); // SETWINDOWORG
        push_record(&mut w, 0x041B, &[150, 100, 50, 20]); // RECTANGLE (b r t l)
        push_record(&mut w, 0x0000, &[]); // EOF
        finish(w)
    }

    #[test]
    fn converts_minimal_rect_wmf() {
        let svg = convert_wmf_to_svg(&minimal_wmf_with_rect())
            .expect("rect WMF should convert");
        let text = String::from_utf8(svg).unwrap();
        assert!(text.contains("<svg"), "not an svg: {text}");
        assert!(
            text.contains("rect") || text.contains("path") || text.contains("polygon"),
            "no drawable element in output: {text}"
        );
    }

    #[test]
    fn garbage_bytes_return_none() {
        assert!(convert_wmf_to_svg(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]).is_none());
        assert!(convert_wmf_to_svg(&[]).is_none());
    }

    #[test]
    fn truncated_header_returns_none() {
        let full = minimal_wmf_with_rect();
        assert!(convert_wmf_to_svg(&full[..10]).is_none());
    }

    #[test]
    fn empty_wmf_degenerates_to_none() {
        // 헤더 + EOF만: 그릴 요소가 없으므로 가드가 None으로 강등해야 한다.
        let mut w: Vec<u8> = Vec::new();
        push_header(&mut w);
        push_record(&mut w, 0x0000, &[]); // EOF
        let out = convert_wmf_to_svg(&finish(w));
        assert!(out.is_none(), "empty WMF must degrade to None");
    }
}
