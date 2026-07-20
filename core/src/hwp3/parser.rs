//! HWP 3.0 텍스트 추출 파서 본체 — paragraph list 재귀 파싱 + IR 변환.
//!
//! Ported from kkdoc (MIT): src/hwp3/parser.ts

use super::johab::decode_johab;
use super::reader::Hwp3Reader;
use super::records::{read_header, Hwp3Header};
use crate::ir::{blocks_to_markdown, IRBlock};
use crate::utils::bounded_io::decompress_deflate_limited;
use std::io;

/// 압축 해제 최대 크기 (100MB) — decompression bomb 방지 (hwp5/record.rs 와 동일 캡).
const MAX_DECOMPRESS_SIZE: usize = 100 * 1024 * 1024;

const PARA_SHAPE_SIZE: usize = 187; // ParaShape 구조
const LINE_INFO_SIZE: usize = 14; // Hwp3LineInfo (u16 x 7)
const INLINE_CHAR_SHAPE_SIZE: usize = 31; // Hwp3CharShape (rep_char_shape 와 동일)

/// 문서에서 추출한 메타데이터.
#[derive(Debug, Clone, Default)]
pub struct Hwp3Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
}

/// HWP3 파싱 결과.
#[derive(Debug, Clone)]
pub struct Hwp3Document {
    pub markdown: String,
    pub blocks: Vec<IRBlock>,
    pub metadata: Hwp3Metadata,
    /// 부분 파싱/미지원 요소에 대한 경고 메시지 ("CODE: message" 형식).
    pub warnings: Vec<String>,
}

struct ParaContext {
    /// 누적된 paragraph text 의 목록 — 각 entry 가 한 paragraph.
    paragraphs: Vec<String>,
    warnings: Vec<String>,
}

/// 제어 문자별 ch 외 추가 read byte 수. 단위는 byte, hchar 는 char_count 에서
/// 차지하는 hchar 개수.
///
/// rhwp/src/parser/hwp3/mod.rs ch 분기 그대로 옮긴 표.
///   9   (Tab)         : extra=6 byte, hchar=3 — hchar+hunit 탭폭+word 점끌기+hchar 닫기
///   18~21 (각종 번호)  : extra=6 byte, hchar=3
///   22  (메일머지)     : extra=22 byte, hchar=11
///   23  (글자겹침)     : extra=8 byte, hchar=4
///   24,25 (하이픈)     : extra=4 byte, hchar=2
///   26  (찾아보기)     : extra=244 byte, hchar=122
///   28  (개요번호)     : extra=62 byte, hchar=31
///   30,31 (빈칸류)     : extra=2 byte, hchar=1
///   7,8 (날짜)         : extra=6 byte, hchar=3
struct CtrlSimple {
    extra_bytes: usize,
    extra_hchar: u32,
    emit: Option<char>,
}

fn simple_ctrl(ch: u16) -> Option<CtrlSimple> {
    const OBJ: char = '\u{FFFC}'; // OBJECT REPLACEMENT CHARACTER
    Some(match ch {
        9 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some('\t') },
        7 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(OBJ) },
        8 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(OBJ) },
        18 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(' ') }, // AutoNumber → 공백 (HWP5 패턴)
        19 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(OBJ) },
        20 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(OBJ) },
        21 => CtrlSimple { extra_bytes: 6, extra_hchar: 3, emit: Some(OBJ) },
        22 => CtrlSimple { extra_bytes: 22, extra_hchar: 11, emit: Some(OBJ) },
        23 => CtrlSimple { extra_bytes: 8, extra_hchar: 4, emit: Some(OBJ) },
        24 => CtrlSimple { extra_bytes: 4, extra_hchar: 2, emit: Some('-') },
        25 => CtrlSimple { extra_bytes: 4, extra_hchar: 2, emit: Some('-') },
        26 => CtrlSimple { extra_bytes: 244, extra_hchar: 122, emit: Some(OBJ) },
        28 => CtrlSimple { extra_bytes: 62, extra_hchar: 31, emit: Some(OBJ) },
        30 => CtrlSimple { extra_bytes: 2, extra_hchar: 1, emit: Some(' ') },
        31 => CtrlSimple { extra_bytes: 2, extra_hchar: 1, emit: Some(' ') },
        _ => return None,
    })
}

/// HWP3 buffer → [`Hwp3Document`].
///
/// `encrypted` 본문은 복호화하지 못하고 `Err` 를 반환한다. signature 불일치,
/// DocInfo/DocSummary 크기 불일치, InfoBlock skip 실패, 압축 해제 실패도 모두
/// `Err`. paragraph list 본문 파싱 도중의 실패는 최대한 모은 결과를
/// `warnings` 에 기록하고 부분 결과를 반환한다 (한 문서에서 일부 paragraph
/// 만 깨지는 경우가 흔하므로 전체를 포기하지 않는다).
pub fn parse_hwp3_document(buffer: &[u8]) -> io::Result<Hwp3Document> {
    let mut head_reader = Hwp3Reader::new(buffer);
    let header: Hwp3Header = read_header(&mut head_reader)?;

    if header.encrypted != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HWP3 본문이 암호로 보호되어 있어 추출할 수 없습니다.",
        ));
    }

    // InfoBlock skip — 폰트/스타일 메타데이터, 텍스트 추출엔 불필요.
    head_reader.skip(header.info_block_length as usize)?;

    // Body: compressed != 0 이면 raw deflate (zlib 헤더 없는 RFC 1951)
    let tail = head_reader.read_to_end();
    let body: Vec<u8> = if header.compressed != 0 {
        decompress_deflate_limited(tail, MAX_DECOMPRESS_SIZE).map_err(|e| {
            io::Error::new(e.kind(), format!("HWP3 압축 해제 실패: {}", e))
        })?
    } else {
        tail.to_vec()
    };

    let mut body_reader = Hwp3Reader::new(&body);
    let mut ctx = ParaContext {
        paragraphs: Vec::new(),
        warnings: Vec::new(),
    };

    let parse_result = skip_font_faces_and_styles(&mut body_reader)
        .and_then(|_| parse_paragraph_list(&mut body_reader, &mut ctx));
    if let Err(err) = parse_result {
        // 부분 파싱 실패 — 모은 만큼이라도 반환. truncated 경고 추가.
        ctx.warnings.push(format!(
            "PARTIAL_PARSE: HWP3 paragraph stream 도중 파싱 중단: {}",
            err
        ));
    }

    let paragraphs: Vec<String> = ctx
        .paragraphs
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    let blocks: Vec<IRBlock> = paragraphs.iter().cloned().map(IRBlock::paragraph).collect();
    let markdown = blocks_to_markdown(&blocks);

    let metadata = Hwp3Metadata {
        title: non_empty(header.title),
        subject: non_empty(header.subject),
        author: non_empty(header.author),
        date: non_empty(header.date),
    };

    Ok(Hwp3Document {
        markdown,
        blocks,
        metadata,
        warnings: ctx.warnings,
    })
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 본문 paragraph_list 진입 전 — 압축 해제된 body 의 앞쪽에는 font / style
/// 메타데이터가 있다. rhwp/src/parser/hwp3/mod.rs:1654~1700 흐름 그대로:
///   - 7개 언어별 font face: n_fonts(u16) + n_fonts x 40 byte name
///   - n_styles(u16) + n_styles x (20 byte name + 31 byte char_shape + 187 byte para_shape)
fn skip_font_faces_and_styles(reader: &mut Hwp3Reader) -> io::Result<()> {
    const STYLE_RECORD_SIZE: usize = 20 + 31 + 187; // = 238
    for _lang in 0..7 {
        let n = reader.read_u16()? as usize;
        reader.skip(n * 40)?;
    }
    let n_styles = reader.read_u16()? as usize;
    reader.skip(n_styles * STYLE_RECORD_SIZE)?;
    Ok(())
}

/// char_count==0 빈 paragraph 가 list 끝.
///
/// paragraph 헤더 필드(followPrev/charCount/lineCount 등) 읽기 실패는 그대로
/// 전파한다(호출자 — 최상위는 `parse_hwp3_document`, 중첩 리스트는 char
/// stream 처리 중인 부모 paragraph 의 로컬 catch — 가 흡수한다). char stream
/// 자체의 실패만 이 함수 안에서 흡수해 해당 리스트 파싱을 조용히 종료한다.
fn parse_paragraph_list(reader: &mut Hwp3Reader, ctx: &mut ParaContext) -> io::Result<()> {
    loop {
        if reader.eof() {
            return Ok(());
        }

        // ParaInfo 헤더 (가변 size). list 끝 sentinel(empty para)이 아니어도 stream sync 가
        // 어긋난 이전 paragraph 의 잔재로 비정상 헤더가 들어올 수 있다. char_count 가
        // 1 paragraph 한도(64K) 를 넘는다거나 lineCount 가 비정상적이면 list 종료로 간주.
        let follow_prev = reader.read_u8()?;
        let char_count = reader.read_u16()?;
        if char_count == 0 {
            // 빈 paragraph: 이미 3 byte 읽었으므로 40 byte 더 read 후 종료
            reader.skip(40)?;
            return Ok(());
        }
        let line_count = reader.read_u16()?;
        // 방어: 한 paragraph 의 line 수가 4096 을 넘는 건 stream 어긋남으로 간주.
        if char_count > 60000 || line_count > 4096 {
            ctx.warnings.push(format!(
                "PARTIAL_PARSE: HWP3 비정상 paragraph 헤더 (char_count={}, line_count={}) → 이후 stream 포기",
                char_count, line_count
            ));
            return Ok(());
        }
        let include_char_shape = reader.read_u8()?;
        reader.skip(1)?; // flags
        reader.skip(4)?; // special_char_flags
        reader.skip(1)?; // style_index
        reader.skip(31)?; // rep_char_shape
        if follow_prev == 0 {
            reader.skip(PARA_SHAPE_SIZE)?;
        }

        // LineInfos
        reader.skip(line_count as usize * LINE_INFO_SIZE)?;

        // Inline char shapes — char_count 만큼 (flag u8, flag != 1 이면 charshape 31 byte)
        if include_char_shape != 0 {
            for _ in 0..char_count {
                let flag = reader.read_u8()?;
                if flag != 1 {
                    reader.skip(INLINE_CHAR_SHAPE_SIZE)?;
                }
            }
        }

        // Char stream — paragraph 단위로 catch 해서 한 paragraph 가 깨져도 list 전체는
        // 살리되, sync 가 어긋난 후의 후속 paragraph 들도 비정상 헤더가 나올 가능성이 커서
        // 헤더 sanity check 로 방어한다.
        match parse_char_stream(reader, char_count, ctx) {
            Ok(text) => ctx.paragraphs.push(text),
            Err(err) => {
                ctx.warnings.push(format!(
                    "PARTIAL_PARSE: HWP3 paragraph #{} char stream 파싱 실패: {}",
                    ctx.paragraphs.len(),
                    err
                ));
                return Ok(());
            }
        }
    }
}

/// paragraph 본문 char_count 개의 hchar 를 처리해 텍스트 추출.
/// 제어 문자는 제어 byte 만큼 정확히 소비하고 일부 (10/11/15/16/17 등) 는
/// nested paragraph list 를 별도로 ctx 에 모은다.
fn parse_char_stream(
    reader: &mut Hwp3Reader,
    char_count: u16,
    ctx: &mut ParaContext,
) -> io::Result<String> {
    let mut out = String::new();
    let mut i: u32 = 0;
    let char_count = char_count as u32;
    while i < char_count {
        let ch = reader.read_u16()?;
        i += 1;

        if ch == 13 {
            out.push('\n');
            continue;
        }
        if ch == 0 {
            // 일부 패딩/오류 케이스 — 무시
            continue;
        }
        if ch >= 32 {
            // 일반 hchar (ASCII < 0x80 영역도 u16 으로 들어옴)
            if let Some(cp) = decode_johab(ch) {
                if let Some(c) = char::from_u32(cp) {
                    out.push(c);
                }
            }
            continue;
        }

        // 1..31 (13 제외) 제어 문자
        if let Some(simple) = simple_ctrl(ch) {
            reader.skip(simple.extra_bytes)?;
            i += simple.extra_hchar;
            if let Some(c) = simple.emit {
                out.push(c);
            }
            continue;
        }

        // ch=10/11/12/14/15/16/17/27/29 등: 8 byte 추가 헤더 + 종류별 추가 처리
        // 8 byte = u32 header_val1 + u16 ch2 (ch 자신의 2 byte 는 이미 위에서 read)
        let header_val1 = reader.read_u32()?;
        let _ch2 = reader.read_u16()?; // sanity, ch 와 같아야 함 (미검증)
        i += 3; // 8 byte 헤더는 char_count 에서 4 hchar 차지 (1 이미 + 3)

        match ch {
            10 => {
                // 표 / 글상자 / 수식 / 버튼: 84 byte info + cells + caption
                out.push_str(&parse_table_like(reader, ctx)?);
            }
            11 => {
                // 그림: 348 byte info + n_ext byte
                parse_picture(reader)?;
            }
            12 => {
                // 선: 84 byte info
                reader.skip(84)?;
            }
            14 => {
                // 선 (alternate path) — rhwp mod.rs line 943: 84 byte info
                reader.skip(84)?;
            }
            15 => {
                // 숨은 설명: 8 byte info + nested paragraph list
                reader.skip(8)?;
                parse_paragraph_list(reader, ctx)?;
            }
            16 => {
                // 머리말/꼬리말: 10 byte info + nested
                reader.skip(10)?;
                parse_paragraph_list(reader, ctx)?;
            }
            17 => {
                // 각주/미주: 14 byte info + nested
                reader.skip(14)?;
                parse_paragraph_list(reader, ctx)?;
            }
            5 => {
                // 필드 코드 (spec §10.1 표 33): 8 byte 헤더 + header_val1 byte 세부 정보
                // (rhwp dcf64b4 #877 정합 — 미소비 시 stream desync). 1MB 이상은 비정상.
                if header_val1 > 0 && header_val1 < 1_000_000 {
                    reader.skip(header_val1 as usize)?;
                }
            }
            6 => {
                // 책갈피 (spec §10.2 표 36): 42 byte total = 8 byte 헤더 + 이름 32 + 종류 2
                // (rhwp dcf64b4 #877 정합 — 34 byte 미소비 시 이후 문단 전체 오염)
                reader.skip(34)?;
            }
            29 => {
                // 상호참조: header_val1 size raw skip (1MB 이상 비정상)
                if header_val1 < 1_000_000 {
                    reader.skip(header_val1 as usize)?;
                }
            }
            _ => {
                // ch=2/3/4/27 등: rhwp mod.rs:1011 의 "알 수 없음" 분기에서
                // header_val1 을 길이로 사용하지 않는다고 명시 ("ch=3 실증: 헤더 직후가 정상 단락
                // 내용이므로 추가 skip 없음"). 즉 8 byte 헤더만 소비하고 다음 char 로.
                // 경고는 첫 등장만 기록 — 본문에 페이지번호/필드코드가 많이 깔린 paragraph 가
                // 전형적인 케이스라 logging 폭주 방지.
                if !ctx.warnings.iter().any(|w| w.starts_with("UNSUPPORTED_ELEMENT")) {
                    ctx.warnings.push(format!(
                        "UNSUPPORTED_ELEMENT: HWP3 부분 처리 제어 문자 ch={} (이후 동일 코드 경고 생략)",
                        ch
                    ));
                }
            }
        }
    }
    Ok(out.trim().to_string())
}

/// ch=10 표/글상자/수식/버튼 본문 텍스트 추출.
fn parse_table_like(reader: &mut Hwp3Reader, ctx: &mut ParaContext) -> io::Result<String> {
    // 84 byte info_buf
    let info = reader.read_bytes(84)?;
    let cell_count_raw = u16::from_le_bytes([info[80], info[81]]);
    let cell_count: u32 = if cell_count_raw == 0 { 1 } else { cell_count_raw as u32 };
    // 방어: cellCount 가 비정상적으로 크면 stream 어긋남으로 간주, 추가 처리 포기.
    // 한 표에 cell 256 개 초과는 사실상 없음 (HWP3 spec 상 행/열 한계도 그 미만).
    if cell_count > 256 {
        ctx.warnings.push(format!(
            "PARTIAL_PARSE: HWP3 표 cell_count={} 비정상 — 표 본문 추출 포기",
            cell_count
        ));
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("HWP3 비정상 cell_count={}", cell_count),
        ));
    }
    // 각 셀: 27 byte 정보 → 셀별 nested paragraph list (재귀)
    reader.skip(27 * cell_count as usize)?;

    // 셀별 텍스트 collect — 셀 내부 paragraph 는 ctx 에 직접 push 되므로
    // 본 paragraph 의 "표 자리" 에는 빈 문자열만 남기고 셀 텍스트는 ctx 안에서 별도 paragraph 로 보존.
    for _ in 0..cell_count {
        parse_paragraph_list(reader, ctx)?;
    }
    // 캡션 paragraph list 1회
    parse_paragraph_list(reader, ctx)?;
    Ok(String::new())
}

/// ch=11 그림 — info 348 byte + n_ext bytes (info[0..4] 가 n_ext).
fn parse_picture(reader: &mut Hwp3Reader) -> io::Result<()> {
    let info = reader.read_bytes(348)?;
    let n_ext = u32::from_le_bytes([info[0], info[1], info[2], info[3]]);
    if n_ext > 0 && (n_ext as usize) < 100 * 1024 * 1024 {
        reader.skip(n_ext as usize)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 합성 HWP3 파일: 30B 시그니처 + 128B DocInfo + 1008B DocSummary + body (비압축).
    fn build_hwp3(body: &[u8]) -> Vec<u8> {
        let mut sig = vec![0u8; 30];
        sig[..23].copy_from_slice(b"HWP Document File V3.00");
        let doc_info = vec![0u8; 128]; // encrypted=0, compressed=0, infoBlockLength=0
        let doc_summary = vec![0u8; 1008];
        [sig, doc_info, doc_summary, body.to_vec()].concat()
    }

    /// body 선두의 font face(언어 7종 x u16 count=0) + style(u16 count=0) 프리앰블.
    fn preamble() -> Vec<u8> {
        vec![0u8; 16]
    }

    /// 단일 paragraph + 종료 sentinel 로 이루어진 body 구성.
    /// char_count 는 hchar 단위 — 호출자가 스트림 구조에 맞게 계산해서 넘긴다.
    fn build_body(char_stream: &[u8], char_count: u16) -> Vec<u8> {
        let mut header = vec![0u8; 43];
        header[0] = 1; // followPrev=1 → ParaShape 없음
        header[1..3].copy_from_slice(&char_count.to_le_bytes());
        header[3..5].copy_from_slice(&0u16.to_le_bytes()); // lineCount=0
        header[5] = 0; // includeCharShape=0
        let terminator = vec![0u8; 43]; // followPrev+charCount(0)+잔여 40
        [preamble(), header, char_stream.to_vec(), terminator].concat()
    }

    fn u16seq(codes: &[u16]) -> Vec<u8> {
        codes.iter().flat_map(|c| c.to_le_bytes()).collect()
    }

    const A: u16 = b'A' as u16;
    const B: u16 = b'B' as u16;

    #[test]
    fn signature_and_header_roundtrip() {
        let file = build_hwp3(&build_body(&u16seq(&[A]), 1));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "A");
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn invalid_signature_rejected() {
        let mut file = build_hwp3(&build_body(&u16seq(&[A]), 1));
        file[0] = b'X';
        assert!(parse_hwp3_document(&file).is_err());
    }

    #[test]
    fn encrypted_flag_rejected() {
        let mut file = build_hwp3(&build_body(&u16seq(&[A]), 1));
        // encrypted u16 at DocInfo offset 96 → absolute offset 30+96=126
        file[126] = 1;
        let err = parse_hwp3_document(&file).unwrap_err();
        assert!(err.to_string().contains("암호"));
    }

    // 회귀 rhwp-1: 탭(ch=9) 8 byte 구조 정합 (d89b689 #929) —
    // 2 byte 만 소비하면 탭마다 6 byte desync 로 이후 텍스트가 오염된다.
    #[test]
    fn tab_control_consumes_8_bytes() {
        // A, [9, hunit(600), word(0), 9], B — 탭은 4 hchar
        let stream = u16seq(&[A, 9, 600, 0, 9, B]);
        let file = build_hwp3(&build_body(&stream, 6));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "A\tB");
        assert!(!doc.warnings.iter().any(|w| w.starts_with("PARTIAL_PARSE")));
    }

    #[test]
    fn consecutive_tabs_stay_in_sync() {
        let stream = u16seq(&[A, 9, 600, 0, 9, 9, 600, 0, 9, B]);
        let file = build_hwp3(&build_body(&stream, 10));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "A\t\tB");
    }

    // 회귀 rhwp-2: ch=5 필드코드 / ch=6 책갈피 스트림 소비 (dcf64b4 #877)
    #[test]
    fn field_code_ch5_consumes_header_val1_bytes() {
        // A, [5, len(u32)=10, ch2=5, 세부 10 byte], B — 헤더는 4 hchar
        let head = u16seq(&[A, 5]);
        let mut len_and_close = vec![0u8; 6];
        len_and_close[0..4].copy_from_slice(&10u32.to_le_bytes());
        len_and_close[4..6].copy_from_slice(&5u16.to_le_bytes());
        let field_data = vec![0xeeu8; 10]; // 소비 안 되면 hchar 로 오독됨
        let tail = u16seq(&[B]);
        let stream = [head, len_and_close, field_data, tail].concat();
        let file = build_hwp3(&build_body(&stream, 6));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "AB");
    }

    #[test]
    fn bookmark_ch6_consumes_34_extra_bytes() {
        let head = u16seq(&[A, 6]);
        let mut len_and_close = vec![0u8; 6];
        len_and_close[0..4].copy_from_slice(&34u32.to_le_bytes());
        len_and_close[4..6].copy_from_slice(&6u16.to_le_bytes());
        let bookmark_extra = vec![0xeeu8; 34];
        let tail = u16seq(&[B]);
        let stream = [head, len_and_close, bookmark_extra, tail].concat();
        let file = build_hwp3(&build_body(&stream, 6));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "AB");
    }

    // 회귀 rhwp-3: 사적 graphic char 매핑 (e184718~aa8b47c)
    #[test]
    fn roman_numeral_chapter_titles_survive() {
        // "Ⅰ. 사업개요"의 로마숫자 부분 — 0x3590~0x3599
        let stream = u16seq(&[0x3590, b'.' as u16, b' ' as u16, 0x3593]);
        let file = build_hwp3(&build_body(&stream, 4));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "Ⅰ. Ⅳ");
    }

    #[test]
    fn circled_numbers_quotes_arrow_bullet_map() {
        let stream = u16seq(&[0x36e7, 0x0081, A, 0x0082, 0x3446, 0x3366, 0x3441]);
        let file = build_hwp3(&build_body(&stream, 7));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "①\u{201c}A\u{201d}→□■");
    }

    #[test]
    fn unmapped_private_area_silently_skipped() {
        let stream = u16seq(&[A, 0x0100, B]); // 0x0100: 매핑 없는 사적영역
        let file = build_hwp3(&build_body(&stream, 3));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "AB");
    }

    #[test]
    fn carriage_return_becomes_newline_within_paragraph() {
        let stream = u16seq(&[A, 13, B]);
        let file = build_hwp3(&build_body(&stream, 3));
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "A\nB");
    }

    #[test]
    fn multiple_paragraphs_join_with_blank_line() {
        fn para_header(char_count: u16) -> Vec<u8> {
            let mut header = vec![0u8; 43];
            header[0] = 1; // followPrev=1 → ParaShape 없음
            header[1..3].copy_from_slice(&char_count.to_le_bytes());
            header
        }
        // preamble + para1(A) + para2(B) + terminator(char_count=0 sentinel)
        let body = [
            preamble(),
            para_header(1),
            u16seq(&[A]),
            para_header(1),
            u16seq(&[B]),
            vec![0u8; 43],
        ]
        .concat();
        let file = build_hwp3(&body);
        let doc = parse_hwp3_document(&file).unwrap();
        assert_eq!(doc.markdown, "A\n\nB");
    }

    #[test]
    fn oversized_cell_count_reports_partial_parse_warning() {
        // ch=10 (table) 헤더 뒤 84 byte info, offset 80..82 에 cell_count=9999
        let mut info = vec![0u8; 84];
        info[80..82].copy_from_slice(&9999u16.to_le_bytes());
        let head = u16seq(&[A, 10]);
        let mut len_and_close = vec![0u8; 6]; // header_val1(u32) + ch2(u16), unused by ch=10
        len_and_close[4..6].copy_from_slice(&10u16.to_le_bytes());
        let stream = [head, len_and_close, info].concat();
        // char_count: A(1) + ch10 header(4) = 5
        let file = build_hwp3(&build_body(&stream, 5));
        let doc = parse_hwp3_document(&file).unwrap();
        assert!(doc.warnings.iter().any(|w| w.contains("cell_count")));
    }
}
