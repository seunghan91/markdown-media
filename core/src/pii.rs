// Ported from kkdoc (MIT): src/redact.ts

//! 한국 공문서 PII(개인정보) 탐지·마스킹 순수 로직.
//!
//! 텍스트 in → 마스킹된 텍스트 + 히트 리포트 out. 문서 파싱/patch는 별도 인프라가
//! 담당하고 여기서는 텍스트만 다룬다.
//!
//! 원칙:
//! - 서식 보존 마스킹 — 자릿수·구분자를 유지해 마스킹 전후 "문자 수"가 동일
//!   (마스크 문자가 멀티바이트인 경우 바이트 길이는 달라질 수 있음 — 원본 TS는
//!   UTF-16 코드유닛 기준 길이 보존이나, 여기서는 유니코드 스칼라(char) 개수 기준)
//! - `redact_text`/`redact_markdown`의 히트 리포트(`RedactHit`)에는 원본 PII를
//!   담지 않는다(`masked` 필드만). 탐지 전용 `detect_pii`의 `PiiMatch`는 감사(audit)
//!   목적이라 원문(`text`)을 포함한다 — 두 API의 용도가 다르므로 타입을 분리했다.
//! - 룰 우선순위 겹침 처리 — 우선순위순으로 매치를 수집하고, 이미 점유된
//!   구간과 겹치는 하위 룰 매치는 스킵 (RULE_PRIORITY 참조)
//! - `index`/`length`는 (원본 TS의 UTF-16 코드유닛과 달리) UTF-8 바이트 오프셋 —
//!   Rust `&str` 슬라이싱과 바로 호환되도록 하기 위함

use fancy_regex::{Captures, Regex};
use lazy_static::lazy_static;
use thiserror::Error;

/// PII 룰 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PiiRule {
    Rrn,
    Phone,
    Email,
    Card,
    Account,
    Passport,
    Driver,
}

impl PiiRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            PiiRule::Rrn => "rrn",
            PiiRule::Phone => "phone",
            PiiRule::Email => "email",
            PiiRule::Card => "card",
            PiiRule::Account => "account",
            PiiRule::Passport => "passport",
            PiiRule::Driver => "driver",
        }
    }
}

/// 룰 우선순위 (앞이 높음). rrn > card > phone > account가 스펙 요구 —
/// 나머지는 포섭 관계로 배치: email을 phone 앞에(로컬파트 숫자에 전화 패턴 오탐 방지),
/// driver를 account 앞에(면허번호 12자리 4그룹이 계좌 패턴에 포섭됨).
const RULE_PRIORITY: [PiiRule; 7] = [
    PiiRule::Rrn,
    PiiRule::Email,
    PiiRule::Card,
    PiiRule::Phone,
    PiiRule::Driver,
    PiiRule::Account,
    PiiRule::Passport,
];

/// 기본 적용 룰 (redact) — passport(여권)·driver(운전면허)는 오탐 여지가 있어 opt-in
pub const DEFAULT_REDACT_RULES: [PiiRule; 5] = [
    PiiRule::Rrn,
    PiiRule::Phone,
    PiiRule::Email,
    PiiRule::Card,
    PiiRule::Account,
];

/// 전체 룰 (detect_pii 기본값) — 7종 모두 탐지
pub const ALL_PII_RULES: [PiiRule; 7] = RULE_PRIORITY;

lazy_static! {
    // 주민/외국인등록번호 — 뒷자리 첫 숫자 1-8 + 생년월일 유효성으로 오탐 축소.
    // 유니코드 대시 변형(‐ ‑ – —)은 rrn만 허용. 앞 6자리 유지, 뒤 7자리 전부 마스크.
    static ref RE_RRN: Regex =
        Regex::new(r"(?<!\d)(\d{6})([-‐‑–—])([1-8]\d{6})(?!\d)").unwrap();
    // 이메일 — 로컬파트 첫 글자만 남기고 마스크, 도메인 유지
    static ref RE_EMAIL: Regex =
        Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap();
    // 카드번호 — 구분자 필수(무구분 16자리는 오탐 높아 제외), 동일 구분자 강제(\2),
    // Luhn 체크. 가운데 8자리 마스크.
    static ref RE_CARD: Regex =
        Regex::new(r"(?<!\d)(\d{4})([- ])(\d{4})\2(\d{4})\2(\d{4})(?!\d)").unwrap();
    // 전화번호 — 휴대폰(01[016789])·서울(02)·지역(0[3-6]\d)·인터넷(070)은 구분자
    // -·.·공백 또는 무구분(동일 구분자 강제), 대표번호(15xx/16xx/18xx)는 구분자 필수.
    // 가운데 자리만 마스크 (대표번호는 뒤 4자리). 선행 [\d-] 금지 — 계좌 부분매치 방지.
    static ref RE_PHONE: Regex = Regex::new(
        r"(?<![\d-])(?:(01[016789]|070|02|0[3-6]\d)([-. ]?)(\d{3,4})\2(\d{4})|(1[568]\d{2})([-. ])(\d{4}))(?!\d)"
    ).unwrap();
    // 운전면허 (기본 OFF) — 신형 12자리만(지역명 2글자 선행 구버전은 스킵). 뒷 8자리 마스크.
    static ref RE_DRIVER: Regex =
        Regex::new(r"(?<![\d-])(\d{2})-(\d{2})-\d{6}-\d{2}(?!-?\d)").unwrap();
    // 계좌번호 — 3~4그룹 + 총 자릿수 10~16. rrn·card·phone과 겹치면 그쪽 우선.
    // 마지막 그룹 빼고 전부 마스크. 사업자등록번호(3-2-5, 10자리)도 걸린다 — 원전 의도.
    static ref RE_ACCOUNT: Regex =
        Regex::new(r"(?<!\d)(?<!\d-)\d{2,6}(?:-\d{2,6}){1,2}-\d{2,8}(?!-?\d)").unwrap();
    // 여권번호 (기본 OFF) — 단어 경계, 첫 글자만 남기고 전부 마스크
    static ref RE_PASSPORT: Regex =
        Regex::new(r"\b([MSRODG])\d{8}(?![0-9A-Za-z])").unwrap();
}

fn regex_for(rule: PiiRule) -> &'static Regex {
    match rule {
        PiiRule::Rrn => &RE_RRN,
        PiiRule::Email => &RE_EMAIL,
        PiiRule::Card => &RE_CARD,
        PiiRule::Phone => &RE_PHONE,
        PiiRule::Driver => &RE_DRIVER,
        PiiRule::Account => &RE_ACCOUNT,
        PiiRule::Passport => &RE_PASSPORT,
    }
}

/// Luhn 체크섬 (카드번호 오탐 축소)
fn luhn_valid(digits: &str) -> bool {
    let bytes = digits.as_bytes();
    let mut sum: u32 = 0;
    for i in 0..bytes.len() {
        let mut d = (bytes[bytes.len() - 1 - i] - b'0') as u32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

/// 주민번호 앞 6자리(YYMMDD)의 월 01-12, 일 01-31 검증
fn birthdate_valid(front6: &str) -> bool {
    let mm: u32 = front6[2..4].parse().unwrap_or(0);
    let dd: u32 = front6[4..6].parse().unwrap_or(0);
    (1..=12).contains(&mm) && (1..=31).contains(&dd)
}

fn validate(rule: PiiRule, caps: &Captures) -> bool {
    match rule {
        PiiRule::Rrn => caps
            .get(1)
            .map(|g| birthdate_valid(g.as_str()))
            .unwrap_or(false),
        PiiRule::Card => {
            let g1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let g3 = caps.get(3).map(|g| g.as_str()).unwrap_or("");
            let g4 = caps.get(4).map(|g| g.as_str()).unwrap_or("");
            let g5 = caps.get(5).map(|g| g.as_str()).unwrap_or("");
            luhn_valid(&format!("{g1}{g3}{g4}{g5}"))
        }
        PiiRule::Account => {
            let whole = caps.get(0).map(|g| g.as_str()).unwrap_or("");
            let digits = whole.chars().filter(|c| *c != '-').count();
            (10..=16).contains(&digits)
        }
        _ => true,
    }
}

fn mask(rule: PiiRule, caps: &Captures, mc: &str) -> String {
    match rule {
        PiiRule::Rrn => {
            let g1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let g2 = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            format!("{g1}{g2}{}", mc.repeat(7))
        }
        PiiRule::Email => {
            let whole = caps.get(0).map(|g| g.as_str()).unwrap_or("");
            let at = whole.find('@').unwrap_or(whole.len());
            let local = &whole[..at];
            let domain = &whole[at..];
            let first_len = local.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            let rest_count = local.chars().count().saturating_sub(1);
            format!("{}{}{}", &local[..first_len], mc.repeat(rest_count), domain)
        }
        PiiRule::Card => {
            let g1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let g2 = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            let g5 = caps.get(5).map(|g| g.as_str()).unwrap_or("");
            format!("{g1}{g2}{}{g2}{}{g2}{g5}", mc.repeat(4), mc.repeat(4))
        }
        PiiRule::Phone => {
            if let Some(g1) = caps.get(1) {
                let g2 = caps.get(2).map(|g| g.as_str()).unwrap_or("");
                let g3 = caps.get(3).map(|g| g.as_str()).unwrap_or("");
                let g4 = caps.get(4).map(|g| g.as_str()).unwrap_or("");
                format!(
                    "{}{g2}{}{g2}{g4}",
                    g1.as_str(),
                    mc.repeat(g3.chars().count())
                )
            } else {
                let g5 = caps.get(5).map(|g| g.as_str()).unwrap_or("");
                let g6 = caps.get(6).map(|g| g.as_str()).unwrap_or("");
                format!("{g5}{g6}{}", mc.repeat(4))
            }
        }
        PiiRule::Driver => {
            let g1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let g2 = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            format!("{g1}-{g2}-{}-{}", mc.repeat(6), mc.repeat(2))
        }
        PiiRule::Account => {
            let whole = caps.get(0).map(|g| g.as_str()).unwrap_or("");
            let parts: Vec<&str> = whole.split('-').collect();
            let last = parts.len() - 1;
            parts
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    if i == last {
                        p.to_string()
                    } else {
                        mc.repeat(p.chars().count())
                    }
                })
                .collect::<Vec<_>>()
                .join("-")
        }
        PiiRule::Passport => {
            let g1 = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            format!("{g1}{}", mc.repeat(8))
        }
    }
}

struct RawHit {
    rule: PiiRule,
    start: usize,
    end: usize,
    masked: String,
}

/// 우선순위순으로 매치를 수집 — 점유 구간과 겹치는 하위 룰 매치는 스킵.
fn scan(text: &str, rules: &[PiiRule], mask_char: char) -> Vec<RawHit> {
    let mc = mask_char.to_string();
    let mut occupied: Vec<(usize, usize)> = Vec::new();
    let mut hits: Vec<RawHit> = Vec::new();
    for &rule in RULE_PRIORITY.iter() {
        if !rules.contains(&rule) {
            continue;
        }
        let re = regex_for(rule);
        for caps_result in re.captures_iter(text) {
            let Ok(caps) = caps_result else { continue };
            let Some(whole) = caps.get(0) else { continue };
            let (start, end) = (whole.start(), whole.end());
            if !validate(rule, &caps) {
                continue;
            }
            if occupied.iter().any(|&(os, oe)| start < oe && end > os) {
                continue;
            }
            occupied.push((start, end));
            let masked = mask(rule, &caps, &mc);
            hits.push(RawHit {
                rule,
                start,
                end,
                masked,
            });
        }
    }
    hits.sort_by_key(|h| h.start);
    hits
}

/// 탐지(감사) 전용 매치 — 원문(`text`)을 포함한다.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PiiMatch {
    pub rule: PiiRule,
    /// 원문 내 시작 오프셋 (UTF-8 바이트 단위)
    pub index: usize,
    /// 매치 길이 (바이트 단위)
    pub length: usize,
    /// 매치된 원문 조각
    pub text: String,
}

/// 텍스트에서 PII를 탐지 (마스킹 없음, 전체 7종 룰 적용). 감사·리포트 용도라
/// 원문을 포함한다 — 리포트를 외부로 내보낼 때는 [`redact_text`]의
/// [`RedactHit`](원본 미포함)을 대신 사용할 것.
pub fn detect_pii(text: &str) -> Vec<PiiMatch> {
    scan(text, &ALL_PII_RULES, '●')
        .into_iter()
        .map(|h| PiiMatch {
            rule: h.rule,
            index: h.start,
            length: h.end - h.start,
            text: text[h.start..h.end].to_string(),
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum PiiError {
    #[error("mask_char는 영숫자가 아닌 1글자여야 함: '{0}'")]
    InvalidMaskChar(char),
}

/// [`redact_text`]/[`redact_markdown`] 옵션
#[derive(Debug, Clone)]
pub struct RedactOptions {
    /// 적용할 룰 (기본: [`DEFAULT_REDACT_RULES`] — passport·driver는 기본 OFF)
    pub rules: Vec<PiiRule>,
    /// 마스크 문자 — 영숫자 금지. 기본 '●'
    pub mask_char: char,
}

impl Default for RedactOptions {
    fn default() -> Self {
        Self {
            rules: DEFAULT_REDACT_RULES.to_vec(),
            mask_char: '●',
        }
    }
}

/// 마스킹 히트 리포트 — 원본 PII는 담지 않는다(`masked` 필드만).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedactHit {
    pub rule: PiiRule,
    /// 마스킹 후 문자열 — 원본 PII는 리포트에 담지 않는다
    pub masked: String,
    /// 원문 내 시작 오프셋 (UTF-8 바이트 단위)
    pub index: usize,
    /// 매치 길이 (원본 기준, 바이트 단위)
    pub length: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedactResult {
    pub text: String,
    pub hits: Vec<RedactHit>,
}

fn validate_mask_char(mask_char: char) -> Result<(), PiiError> {
    if mask_char.is_ascii_alphanumeric() {
        return Err(PiiError::InvalidMaskChar(mask_char));
    }
    Ok(())
}

/// 텍스트에서 PII를 탐지해 서식 보존 마스킹 (텍스트 + 히트 리포트).
pub fn redact_text(text: &str, opts: &RedactOptions) -> Result<RedactResult, PiiError> {
    validate_mask_char(opts.mask_char)?;
    if text.is_empty() || opts.rules.is_empty() {
        return Ok(RedactResult {
            text: text.to_string(),
            hits: Vec::new(),
        });
    }

    let raw = scan(text, &opts.rules, opts.mask_char);
    let mut out = String::with_capacity(text.len());
    let mut hits = Vec::with_capacity(raw.len());
    let mut cursor = 0usize;
    for h in raw {
        out.push_str(&text[cursor..h.start]);
        out.push_str(&h.masked);
        cursor = h.end;
        hits.push(RedactHit {
            rule: h.rule,
            index: h.start,
            length: h.end - h.start,
            masked: h.masked,
        });
    }
    out.push_str(&text[cursor..]);
    Ok(RedactResult { text: out, hits })
}

/// 텍스트에서 PII를 탐지해 서식 보존 마스킹한 결과 텍스트만 반환.
pub fn redact(text: &str, opts: &RedactOptions) -> Result<String, PiiError> {
    Ok(redact_text(text, opts)?.text)
}

/// 마크다운 문서 전용 래퍼 — base64 이미지(data URI) 라인은 마스킹에서 제외한다.
/// base64 페이로드의 숫자열이 phone 등에 오탐되면 이미지가 깨지기 때문.
/// hits의 index는 문서 전체 기준 절대 바이트 오프셋으로 환산된다.
pub fn redact_markdown(markdown: &str, opts: &RedactOptions) -> Result<RedactResult, PiiError> {
    validate_mask_char(opts.mask_char)?;
    let mut hits = Vec::new();
    let mut offset = 0usize;
    let mut out_lines: Vec<String> = Vec::new();
    for line in markdown.split('\n') {
        if line.contains("data:image/") {
            offset += line.len() + 1;
            out_lines.push(line.to_string());
            continue;
        }
        let r = redact_text(line, opts)?;
        for h in r.hits {
            hits.push(RedactHit {
                rule: h.rule,
                masked: h.masked,
                index: h.index + offset,
                length: h.length,
            });
        }
        offset += line.len() + 1;
        out_lines.push(r.text);
    }
    Ok(RedactResult {
        text: out_lines.join("\n"),
        hits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_opts() -> RedactOptions {
        RedactOptions::default()
    }

    // ── 주민등록번호 ──────────────────────────────────────────────
    #[test]
    fn rrn_valid_detected_and_masked() {
        let text = "주민번호: 900101-1234567 입니다";
        let hits = detect_pii(text);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, PiiRule::Rrn);
        assert_eq!(hits[0].text, "900101-1234567");

        let out = redact(text, &default_opts()).unwrap();
        assert_eq!(out, "주민번호: 900101-●●●●●●● 입니다");
    }

    #[test]
    fn rrn_invalid_birthdate_not_detected() {
        // 월=13, 일=40 — 생년월일 유효성 실패로 미탐지
        let text = "번호 901340-1234567";
        let hits = detect_pii(text);
        assert!(hits.iter().all(|h| h.rule != PiiRule::Rrn));
    }

    #[test]
    fn rrn_bad_checksum_prefix_not_detected() {
        // 뒷자리 첫 숫자 0/9는 [1-8] 범위 밖 → 형식 자체가 미매치
        let text = "900101-0234567";
        let hits = detect_pii(text);
        assert!(hits.iter().all(|h| h.rule != PiiRule::Rrn));
    }

    // ── 전화번호 ──────────────────────────────────────────────────
    #[test]
    fn phone_mobile_masked() {
        let out = redact("연락처 010-1234-5678 입니다", &default_opts()).unwrap();
        assert_eq!(out, "연락처 010-●●●●-5678 입니다");
    }

    #[test]
    fn phone_representative_number_masked() {
        let out = redact("전화 1588-1234", &default_opts()).unwrap();
        assert_eq!(out, "전화 1588-●●●●");
    }

    #[test]
    fn phone_not_matched_when_preceded_by_digit_hyphen() {
        // 전화 패턴 직전이 "숫자-"면 시작하지 않아야 함(다른 숫자열의 부분매치 방지) —
        // 5개 그룹짜리 긴 숫자열의 내부 시작점은 전부 "숫자-"로 막혀 있어 미탐지.
        let hits = detect_pii("110-483-010-1234-5678");
        assert!(hits.iter().all(|h| h.rule != PiiRule::Phone));
        assert!(hits.iter().all(|h| h.rule != PiiRule::Account));
    }

    // ── 이메일 ────────────────────────────────────────────────────
    #[test]
    fn email_masked_local_part_only() {
        let out = redact("문의: hong@example.com", &default_opts()).unwrap();
        assert_eq!(out, "문의: h●●●@example.com");
    }

    // ── 신용카드 ──────────────────────────────────────────────────
    #[test]
    fn card_luhn_valid_masked() {
        // 4532015112830366 — Luhn 유효 테스트 번호
        let out = redact("카드 4532-0151-1283-0366", &default_opts()).unwrap();
        assert_eq!(out, "카드 4532-●●●●-●●●●-0366");
    }

    #[test]
    fn card_luhn_invalid_not_detected() {
        let hits = detect_pii("카드 1234-5678-9012-3456");
        assert!(hits.iter().all(|h| h.rule != PiiRule::Card));
    }

    // ── 계좌번호 ──────────────────────────────────────────────────
    #[test]
    fn account_masked_keeps_last_group() {
        let out = redact("계좌 110-483-020394", &default_opts()).unwrap();
        assert_eq!(out, "계좌 ●●●-●●●-020394");
    }

    #[test]
    fn account_digit_count_out_of_range_not_detected() {
        // 총 자릿수 8 (< 10) — 날짜류 오탐 방지
        let hits = detect_pii("2026-07-19");
        assert!(hits.iter().all(|h| h.rule != PiiRule::Account));
    }

    // ── 여권/운전면허 (opt-in) ───────────────────────────────────
    #[test]
    fn passport_detected_when_enabled() {
        let opts = RedactOptions {
            rules: vec![PiiRule::Passport],
            ..Default::default()
        };
        let out = redact("여권 M12345678", &opts).unwrap();
        assert_eq!(out, "여권 M●●●●●●●●");
    }

    #[test]
    fn driver_not_in_default_rules() {
        let hits = detect_pii("면허 12-34-567890-12");
        // detect_pii는 전체 룰 적용이므로 driver가 잡혀야 함
        assert!(hits.iter().any(|h| h.rule == PiiRule::Driver));
        // 반면 기본 redact 룰에는 driver가 없다
        let out = redact("면허 12-34-567890-12", &default_opts()).unwrap();
        assert_ne!(out, "12-34-●●●●●●-●●");
    }

    // ── 우선순위 겹침 ────────────────────────────────────────────
    #[test]
    fn rrn_takes_priority_over_overlapping_rules() {
        let hits = detect_pii("900101-1234567");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule, PiiRule::Rrn);
    }

    // ── 마크다운 래퍼 ────────────────────────────────────────────
    #[test]
    fn markdown_skips_base64_image_lines() {
        let md = "전화 010-1234-5678\n![x](data:image/png;base64,010123456789)\n";
        let result = redact_markdown(md, &default_opts()).unwrap();
        assert!(result.text.contains("010123456789")); // base64 라인은 그대로
        assert!(result.text.contains("010-●●●●-5678"));
    }

    #[test]
    fn markdown_link_and_table_syntax_preserved() {
        let md = "| 연락처 | [문의](mailto:a@b.com) |\n| --- | --- |\n| 010-1234-5678 | hong@example.com |";
        let result = redact_markdown(md, &default_opts()).unwrap();
        assert!(result.text.contains("| --- | --- |"));
        assert!(result.text.starts_with("| 연락처 |"));
    }

    // ── 옵션 검증 ────────────────────────────────────────────────
    #[test]
    fn invalid_mask_char_rejected() {
        let opts = RedactOptions {
            mask_char: 'X',
            ..Default::default()
        };
        assert!(matches!(
            redact("010-1234-5678", &opts),
            Err(PiiError::InvalidMaskChar('X'))
        ));
    }

    #[test]
    fn custom_mask_char_preserves_char_count() {
        let opts = RedactOptions {
            mask_char: '*',
            ..Default::default()
        };
        let out = redact("010-1234-5678", &opts).unwrap();
        assert_eq!(out, "010-****-5678");
    }
}
