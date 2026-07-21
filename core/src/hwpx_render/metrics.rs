// Ported from kkdoc (MIT): src/hwpx/text-metrics.ts
//! 한글 조판 텍스트 폭 계산 — 함초롬바탕(HCR Batang) 실측 advance 테이블.
//!
//! 단위: em×1000. 실제 폭(HWPUNIT) = w/1000 × charPr.height × ratio(장평)/100.
//! (1em = 글자크기 = charPr.height HWPUNIT — 1pt = 100 HWPUNIT)

/// ASCII 0x20~0x7E advance (em×1000). 0x20은 useFontSpace=1일 때의 글꼴값(300)
#[rustfmt::skip]
const ASCII_W: [i32; 95] = [
    300, 320, 320, 610, 610, 830, 724, 320, 320, 320, 550, 550, 320, 550, 320, 550, // 0x20-0x2F
    550, 550, 550, 550, 550, 550, 550, 550, 550, 550, 320, 320, 550, 550, 550, 550, // 0x30-0x3F
    830, 706, 605, 685, 719, 627, 617, 683, 734, 305, 315, 660, 605, 839, 734, 732, // 0x40-0x4F
    603, 705, 660, 627, 664, 731, 706, 910, 705, 705, 626, 320, 550, 320, 550, 550, // 0x50-0x5F
    320, 569, 597, 552, 597, 536, 356, 562, 635, 287, 288, 582, 287, 907, 635, 588, // 0x60-0x6F
    597, 579, 478, 496, 356, 635, 563, 720, 542, 543, 486, 320, 320, 320, 550,      // 0x70-0x7E
];

/// 개별 실측 예외 기호 (em×1000)
fn sym_w(cp: u32) -> Option<i32> {
    Some(match cp {
        0xa0 => 300, 0xa3 => 568, 0xa5 => 707, 0xa7 => 498, 0xab => 440, 0xac => 564, 0xb0 => 291,
        0xb1 => 798, 0xb6 => 606, 0xb7 => 320, 0xbb => 440, 0xd7 => 617, 0xf7 => 678,
        0x2013 => 625, 0x2014 => 875, 0x2015 => 875, 0x2018 => 320, 0x2019 => 320,
        0x201c => 480, 0x201d => 480, 0x2020 => 558, 0x2021 => 438, 0x2025 => 640, 0x2026 => 960,
        0x2030 => 988, 0x2032 => 335, 0x2033 => 474, 0x203b => 770, 0x20ac => 656,
        0x261c => 1012, 0x261e => 1012,
        _ => return None,
    })
}

/// 코드포인트의 advance(em×1000). 미상 문자는 CJK권 970 / 라틴권 550 폴백
pub fn char_width_em1000(cp: u32) -> i32 {
    if (0x20..=0x7e).contains(&cp) {
        return ASCII_W[(cp - 0x20) as usize];
    }
    if let Some(s) = sym_w(cp) {
        return s;
    }
    if (0xac00..=0xd7a3).contains(&cp) {
        return 970; // 한글 음절 (전수 균일 확인)
    }
    if (0x1100..=0x11ff).contains(&cp) {
        return 970; // 옛한글 자모
    }
    if (0x3131..=0x318e).contains(&cp) {
        return 970; // 호환 자모 (ㆍ 포함)
    }
    if (0x4e00..=0x9fff).contains(&cp) || (0xf900..=0xfaff).contains(&cp) {
        return 1000; // 한자
    }
    if (0x3008..=0x3011).contains(&cp) || (0x3014..=0x301b).contains(&cp) {
        return 500; // 「」『』〈〉《》〔〕【】
    }
    if cp == 0x3000 {
        return 970; // 전각 공백
    }
    if (0x2160..=0x2183).contains(&cp) {
        return 970; // 로마숫자 Ⅰ~Ⅻ
    }
    if (0x2190..=0x22ff).contains(&cp) {
        return 970; // 화살표·수학 기호
    }
    if (0x2460..=0x24ff).contains(&cp) {
        return 970; // 원문자 ①⑴
    }
    if (0x25a0..=0x26ff).contains(&cp) {
        return 970; // 도형 □○◆★
    }
    if (0x3200..=0x33ff).contains(&cp) {
        return 970; // 괄호한글 ㉮·단위 ㎡㎏
    }
    if (0xff01..=0xff60).contains(&cp) {
        return 970; // 전각형 ！～
    }
    if cp >= 0x2e80 {
        970
    } else {
        550
    }
}

/// HWP 공백 폭(em×1000) — useFontSpace=0(기본): 반각 고정 500
pub const SPACE_EM_FIXED: i32 = 500;

/// 폭 테이블 클래스 — 'hcr'(함초롬 실측, 기본) / 'fixedPitch'(고정폭 글꼴)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaceClass {
    Hcr,
    FixedPitch,
}

/// HWP 글꼴명 → 폭 테이블 클래스. 미상/미지정은 hcr
pub fn face_class_of(face: Option<&str>) -> FaceClass {
    match face.map(|f| f.trim()) {
        Some("굴림체") | Some("돋움체") | Some("바탕체") | Some("궁서체") => FaceClass::FixedPitch,
        _ => FaceClass::Hcr,
    }
}

fn width_em(cp: u32, cls: FaceClass) -> i32 {
    match cls {
        FaceClass::FixedPitch => {
            if cp < 0x80 {
                500
            } else {
                1000
            }
        }
        FaceClass::Hcr => char_width_em1000(cp),
    }
}

/// 폭 측정 옵션
#[derive(Clone, Copy)]
pub struct MeasureOptions {
    pub space_em: i32,
    pub spacing_pct: f64,
    pub face_class: FaceClass,
}

impl Default for MeasureOptions {
    fn default() -> Self {
        MeasureOptions { space_em: SPACE_EM_FIXED, spacing_pct: 0.0, face_class: FaceClass::Hcr }
    }
}

/// 텍스트 폭(HWPUNIT). height=charPr height(pt×100), ratio_pct=장평 %.
pub fn measure_text_width(text: &str, height: f64, ratio_pct: f64, opts: &MeasureOptions) -> f64 {
    let mut em = 0.0_f64;
    for ch in text.chars() {
        let cp = ch as u32;
        let w = if cp == 0x20 { opts.space_em } else { width_em(cp, opts.face_class) };
        em += (w as f64) * (1.0 + opts.spacing_pct / 100.0);
    }
    (em / 1000.0) * height * (ratio_pct / 100.0)
}

// ─── 줄바꿈 시뮬레이션 (한컴 조판 모델 — 실측 linesegarray 98% 일치) ───

/// 줄머리 금지(시작금칙) 문자
const FORBID_START: &str = "!%),.:;?]}¢°′″℃〉》」』】〕!%),.:;?]}₩~…·、。〃";
/// 줄끝 금지(끝금칙) 문자
const FORBID_END: &str = "$([{£¥〈《「『【〔$([{₩";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WrapMode {
    /// 어절 단위 (저장 속성 breakNonLatinWord="BREAK_WORD")
    Keep,
    /// 글자 단위 (breakNonLatinWord="KEEP_WORD")
    CharAll,
}

pub struct WrapResult {
    pub lines: usize,
    /// 각 줄의 시작 오프셋(char 인덱스, UTF-16 유닛과 무관하게 char 단위) — [0, …]
    pub starts: Vec<usize>,
    pub last_line_width: f64,
}

fn forbid_start(ch: char) -> bool {
    FORBID_START.contains(ch)
}
fn forbid_end(ch: char) -> bool {
    FORBID_END.contains(ch)
}

/// 문단 줄바꿈 시뮬레이션. starts는 char(코드포인트) 인덱스 기준.
///
/// 주의: 원본 TS는 UTF-16 유닛 오프셋을 쓰지만, Rust 포트는 char 인덱스로 통일한다.
/// 호출자(reflow)에서 char 인덱스 → chars 슬롯 매핑(realIdx)을 동일 기준으로 만든다.
pub fn simulate_wrap(
    text: &str,
    first_width: f64,
    cont_width: f64,
    height: f64,
    ratio_pct: f64,
    mode: WrapMode,
    opts: &MeasureOptions,
) -> WrapResult {
    const EPS: f64 = 0.5;
    let chars: Vec<char> = text.chars().collect();
    let k = height * ratio_pct / 100.0 / 1000.0;
    let cw = |ch: char| -> f64 {
        let cp = ch as u32;
        let base = if cp == 0x20 { opts.space_em } else { width_em(cp, opts.face_class) };
        (base as f64) * (1.0 + opts.spacing_pct / 100.0) * k
    };
    let range_w = |from: usize, to: usize| -> f64 {
        let mut w = 0.0;
        for &ch in &chars[from..to.min(chars.len())] {
            w += cw(ch);
        }
        w
    };

    // 유닛 분해 — 공백 런 / 비공백 런(keep) 또는 단일 비공백(charAll)
    let mut units: Vec<(usize, usize)> = Vec::new(); // (start, end) char index
    {
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == ' ' {
                let s = i;
                while i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                units.push((s, i));
            } else {
                let s = i;
                match mode {
                    WrapMode::Keep => {
                        while i < chars.len() && chars[i] != ' ' {
                            i += 1;
                        }
                    }
                    WrapMode::CharAll => {
                        i += 1;
                    }
                }
                units.push((s, i));
            }
        }
    }

    let mut starts = vec![0usize];
    let mut line_w = 0.0_f64;
    let mut avail = first_width;

    // 유닛이 안 들어갈 때 줄바꿈 + 금칙 보정
    let break_before = |unit_pos: usize, w: f64, starts: &mut Vec<usize>, line_w: &mut f64, avail: &mut f64| {
        let line_start = *starts.last().unwrap();
        let mut bp = unit_pos;
        // 시작금칙: 줄머리 금지 문자면 직전 글자 1개를 함께 내린다
        if unit_pos < chars.len()
            && forbid_start(chars[unit_pos])
            && bp > 0
            && bp - 1 > line_start
            && chars[bp - 1] != ' '
        {
            bp -= 1;
        }
        // 끝금칙: 남는 줄 끝이 여는 괄호류면 그 글자(들)도 함께 내린다
        while bp > 0 && bp - 1 > line_start && forbid_end(chars[bp - 1]) {
            bp -= 1;
        }
        if bp <= line_start {
            bp = unit_pos;
        }
        starts.push(bp);
        *avail = cont_width;
        *line_w = range_w(bp, unit_pos) + w;
    };

    for (us, ue) in units {
        if chars[us] == ' ' {
            line_w += cw(' ') * ((ue - us) as f64); // 줄 끝 공백은 hang
            continue;
        }
        let w = range_w(us, ue);
        if line_w + w <= avail + EPS {
            line_w += w;
            continue;
        }
        if line_w == 0.0 || w > cont_width + EPS {
            // 빈 줄이거나 다음 줄에도 안 들어가는 초장 유닛 — 글자 단위 강제 분해
            for (pos, &ch) in (us..).zip(chars[us..ue].iter()) {
                let c = cw(ch);
                if line_w + c > avail + EPS && line_w > 0.0 {
                    break_before(pos, 0.0, &mut starts, &mut line_w, &mut avail);
                }
                line_w += c;
            }
            continue;
        }
        break_before(us, w, &mut starts, &mut line_w, &mut avail);
    }

    WrapResult { lines: starts.len(), starts, last_line_width: line_w }
}
