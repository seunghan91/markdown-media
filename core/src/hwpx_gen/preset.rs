// Ported from kkdoc (MIT): src/hwpx/gongmun.ts, src/hwpx/geometry.ts
//! Korean public-document presets (공문서 프리셋) + item-marker sequences.
//!
//! This is a bounded port of the reference's pure logic: preset resolution
//! (margins, body size, line spacing, numbering system), the 8-level legal
//! marker sequence, report bullets, and per-depth indentation. The measured
//! decorative assets (cover/toc/chapter boxes, 결재란, docframe) are out of
//! scope — see module docs in mod.rs.

// ─── Geometry (A4 portrait, HWPUNIT) ────────────────
// 210mm × 297mm. 1 HWPUNIT = 1/7200 inch → 1mm ≈ 283.46 HU.
pub const A4_W_HU: u32 = 59528;
pub const A4_H_HU: u32 = 84188;

/// 1mm → HWPUNIT (rounded). Keep the `(mm*7200)/25.4` form for byte-parity.
pub fn mm_to_hwpunit(mm: f64) -> i32 {
    ((mm * 7200.0) / 25.4).round() as i32
}

// ─── Preset kinds ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// 기안문 — general official document (legal 8-level numbering).
    Official,
    /// 보고서 — report (□○- bullets).
    Report,
    /// 계획서 — plan (□ㅇ* bullets).
    Plan,
    /// 통지 — notice/announcement (numbered h2 headings).
    Notice,
    /// 회의록 — minutes (tight line spacing).
    Minutes,
    /// 개조식 — government standard structured report.
    Gaejosik,
    /// 보도자료 — press release.
    Press,
}

impl Preset {
    /// Resolve a Korean/English alias to a preset. Unknown → Official.
    pub fn from_alias(s: &str) -> Preset {
        match s.trim() {
            "official" | "기안문" | "시행문" | "공문" | "공문서" => Preset::Official,
            "report" | "보고서" => Preset::Report,
            "plan" | "계획서" | "계획" => Preset::Plan,
            "notice" | "통지" | "알림" | "안내" => Preset::Notice,
            "minutes" | "회의록" => Preset::Minutes,
            "gaejosik" | "개조식" | "개조식보고서" | "정부보고서" | "정부표준개조식보고서" => {
                Preset::Gaejosik
            }
            "press" | "보도자료" => Preset::Press,
            _ => Preset::Official,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    /// Legal 8-level (1. 가. 1) …).
    Standard,
    /// Report bullets (□ ○ - ㆍ).
    Report,
    /// Structured (□ ○ - ㆍ, per-symbol fonts — rendered same as Report here).
    Gaejosik,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H2Marker {
    Box,
    Number,
    None,
}

#[derive(Debug, Clone)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// Fully-resolved preset configuration consumed by the generator.
#[derive(Debug, Clone)]
pub struct ResolvedPreset {
    pub preset: Preset,
    /// charPr height for body = pt × 100.
    pub body_height: u32,
    pub line_spacing: u32,
    pub numbering: Numbering,
    pub margins: Margins,
    pub center_title: bool,
    pub page_numbers: bool,
    pub header_footer: u32,
    pub h2_marker: H2Marker,
    /// Second-level bullet: 'ㅇ' or '○'.
    pub bullet2: char,
    /// Uses '*' as third-level bullet (plan/press).
    pub asterisk_third: bool,
}

// 기안문 여백(mm) — 실결재 지배값.
const OFFICIAL_MARGINS: Margins = Margins { top: 20.0, bottom: 15.0, left: 20.0, right: 15.0 };
// 보고서 계열 여백(mm).
const GAEJOSIK_MARGINS: Margins = Margins { top: 15.0, bottom: 15.0, left: 20.0, right: 20.0 };
const GAEJOSIK_HEADER_FOOTER: u32 = 4251;

/// Resolve a preset to concrete generation parameters.
pub fn resolve_preset(preset: Preset) -> ResolvedPreset {
    let (body_pt, line_spacing, numbering) = match preset {
        Preset::Official => (12, 160, Numbering::Standard),
        Preset::Report => (15, 160, Numbering::Report),
        Preset::Plan => (15, 160, Numbering::Report),
        Preset::Notice => (15, 160, Numbering::Standard),
        Preset::Minutes => (14, 130, Numbering::Standard),
        Preset::Gaejosik => (15, 160, Numbering::Gaejosik),
        Preset::Press => (14, 160, Numbering::Report),
    };
    let report_family = matches!(
        preset,
        Preset::Gaejosik | Preset::Report | Preset::Plan | Preset::Notice | Preset::Press
    );
    let uses_report_fonts = matches!(preset, Preset::Gaejosik | Preset::Report | Preset::Plan);
    let header_footer = if uses_report_fonts {
        GAEJOSIK_HEADER_FOOTER
    } else if matches!(preset, Preset::Notice | Preset::Press) {
        2835
    } else {
        0
    };
    let h2_marker = match preset {
        Preset::Report | Preset::Plan => H2Marker::Box,
        Preset::Notice => H2Marker::Number,
        _ => H2Marker::None,
    };
    let bullet2 = if matches!(preset, Preset::Plan | Preset::Notice | Preset::Press) {
        'ㅇ'
    } else {
        '○'
    };
    ResolvedPreset {
        preset,
        body_height: body_pt * 100,
        line_spacing,
        numbering,
        margins: if report_family { GAEJOSIK_MARGINS.clone() } else { OFFICIAL_MARGINS.clone() },
        center_title: true,
        page_numbers: matches!(preset, Preset::Gaejosik | Preset::Report | Preset::Plan),
        header_footer,
        h2_marker,
        bullet2,
        asterisk_third: matches!(preset, Preset::Plan | Preset::Press),
    }
}

// ─── Item-marker sequences ──────────────────────────

// 가나다 initials (14, no double consonants).
const HANGUL_INITIALS: [u32; 14] = [0, 2, 3, 5, 6, 7, 9, 11, 12, 14, 15, 16, 17, 18];
// Simple vowels: ㅏ ㅓ ㅗ ㅜ ㅡ ㅣ.
const HANGUL_MEDIALS: [u32; 6] = [0, 4, 8, 13, 18, 20];

/// 0-based n → 가, 나, … 하, 거, … (simple-vowel sequence).
pub fn hangul_ordinal(n: usize) -> char {
    let cols = HANGUL_INITIALS.len();
    let vowel = HANGUL_MEDIALS[(n / cols).min(HANGUL_MEDIALS.len() - 1)];
    let init = HANGUL_INITIALS[n % cols];
    char::from_u32(0xac00 + init * 588 + vowel * 28).unwrap_or('가')
}

/// 0-based n → ① … ⑳ ㉑ … ㊿, then "(n+1)".
pub fn circled_number(n: usize) -> String {
    if n < 20 {
        char::from_u32(0x2460 + n as u32).unwrap().to_string()
    } else if n < 35 {
        char::from_u32(0x3251 + (n as u32 - 20)).unwrap().to_string()
    } else if n < 50 {
        char::from_u32(0x32b1 + (n as u32 - 35)).unwrap().to_string()
    } else {
        format!("({})", n + 1)
    }
}

/// 0-based n → ㉮ ㉯ … ㉻ (14), then hangul ordinal.
pub fn circled_hangul(n: usize) -> String {
    if n < 14 {
        char::from_u32(0x326e + n as u32).unwrap().to_string()
    } else {
        hangul_ordinal(n).to_string()
    }
}

const REPORT_BULLETS: [&str; 4] = ["□", "○", "-", "ㆍ"];
const ASTERISK_BULLETS: [&str; 4] = ["□", "○", "*", "ㆍ"];

/// 'standard' (legal 8-level) marker. `depth` 0..=7, `n` 0-based sibling index.
pub fn standard_marker(depth: usize, n: usize) -> String {
    match depth {
        0 => format!("{}.", n + 1),
        1 => format!("{}.", hangul_ordinal(n)),
        2 => format!("{})", n + 1),
        3 => format!("{})", hangul_ordinal(n)),
        4 => format!("({})", n + 1),
        5 => format!("({})", hangul_ordinal(n)),
        6 => circled_number(n),
        _ => circled_hangul(n),
    }
}

/// 'report' bullet marker (sibling index irrelevant).
pub fn report_marker(depth: usize, bullet2: char, asterisk_third: bool) -> String {
    let bullets = if asterisk_third { ASTERISK_BULLETS } else { REPORT_BULLETS };
    let m = bullets[depth.min(bullets.len() - 1)];
    if depth == 1 {
        bullet2.to_string()
    } else {
        m.to_string()
    }
}

/// Rough render width (HWPUNIT) of a marker + 1 space, for hanging indent.
/// Approximation: hangul/circled ≈ 0.97em, digit 0.55em, dot/paren 0.32em,
/// plus 0.5em trailing space.
pub fn marker_width(marker: &str, body_height: u32) -> i32 {
    let mut em = 500.0f64; // trailing space 0.5em
    for c in marker.chars() {
        em += char_width_em1000(c);
    }
    ((em / 1000.0) * body_height as f64).round() as i32
}

fn char_width_em1000(c: char) -> f64 {
    match c {
        '.' | ',' | '(' | ')' | ':' | ';' | '\'' => 320.0,
        '0'..='9' => 550.0,
        ' ' => 500.0,
        '-' | '~' => 500.0,
        c if (c as u32) < 0x80 => 500.0,
        _ => 970.0, // CJK / circled
    }
}

/// Per-depth indentation. `left` = depth × body_height; `indent` = negative
/// marker width (hanging indent so wrapped lines align under content).
pub fn level_indent(res: &ResolvedPreset, depth: usize) -> (i32, i32) {
    let marker = match res.numbering {
        Numbering::Report | Numbering::Gaejosik => {
            report_marker(depth, res.bullet2, res.asterisk_third)
        }
        Numbering::Standard => standard_marker(depth, 0),
    };
    let left = (depth as f64 * res.body_height as f64).round() as i32;
    (left, -marker_width(&marker, res.body_height))
}

/// Stateful marker counter: tracks per-depth sibling index, resets deeper
/// levels when a higher level advances.
pub struct Numberer {
    counts: Vec<usize>,
    numbering: Numbering,
    bullet2: char,
    asterisk_third: bool,
}

impl Numberer {
    pub fn new(res: &ResolvedPreset) -> Self {
        Numberer {
            counts: Vec::new(),
            numbering: res.numbering,
            bullet2: res.bullet2,
            asterisk_third: res.asterisk_third,
        }
    }

    /// Marker for one item at `depth`.
    pub fn next(&mut self, depth: usize) -> String {
        self.counts.truncate(depth + 1);
        while self.counts.len() <= depth {
            self.counts.push(0);
        }
        let n = self.counts[depth];
        self.counts[depth] = n + 1;
        match self.numbering {
            Numbering::Report | Numbering::Gaejosik => {
                report_marker(depth, self.bullet2, self.asterisk_third)
            }
            Numbering::Standard => standard_marker(depth, n),
        }
    }
}
