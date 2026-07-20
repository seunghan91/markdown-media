// Ported from kkdoc (MIT): src/hwpx/gongmun-lint.ts

//! 공문서 표기법 검수기 — 「행정업무의 운영 및 혁신에 관한 규정」 시행규칙 및
//! 행정안전부 행정업무운영 편람의 날짜·시간·금액·기호 표기법을 정규식으로 검사.
//!
//! 원전: jkf87/hwpx-skill gonmun_lint.py(2025 편람 기준 13룰)를 kkdoc에 맞게 이식한
//! `gongmun-lint.ts`(v4.0.1)를 다시 Rust로 이식. 검사는 조언용이다 — 생성은 막지
//! 않고 경고만 낸다.

use fancy_regex::Regex;
use lazy_static::lazy_static;

/// 규칙 심각도
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    Error,
    Warning,
}

/// 린트 발견 항목
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LintIssue {
    /// 1-based 줄 번호
    pub line: usize,
    /// 걸린 원문 조각
    #[serde(rename = "match")]
    pub matched: String,
    /// 규칙 코드 (DATE_NO_SPACE 등)
    pub rule: &'static str,
    pub severity: LintSeverity,
    pub message: &'static str,
    pub suggest: Option<&'static str>,
}

struct LintRule {
    code: &'static str,
    severity: LintSeverity,
    pattern: Regex,
    message: &'static str,
    suggest: Option<&'static str>,
}

lazy_static! {
    // 규칙 순서·코드·문구는 편람 기준 원전(gonmun_lint.py) 유지 — 대조 검증 용이성
    static ref RULES: Vec<LintRule> = vec![
        // 날짜 ─ 온점 뒤 한 칸, 0 패딩 금지, 연도 4자리, 끝 마침표
        LintRule {
            code: "DATE_NO_SPACE", severity: LintSeverity::Error,
            pattern: Regex::new(r"\b\d{4}\.\d{1,2}\.\d{1,2}\.?").unwrap(),
            message: "날짜 온점 뒤에 한 칸씩 띄워야 함", suggest: Some("예) 2025. 1. 6."),
        },
        LintRule {
            code: "DATE_ZERO_PAD", severity: LintSeverity::Error,
            pattern: Regex::new(r"\b\d{4}\.\s*0\d\.|\b\d{4}\.\s*\d{1,2}\.\s*0\d").unwrap(),
            message: "월·일 앞의 '0'은 표기하지 않음", suggest: Some("예) 2025. 1. 6. (2025. 01. 06. ✕)"),
        },
        LintRule {
            code: "DATE_2DIGIT_YR", severity: LintSeverity::Error,
            pattern: Regex::new(r"(?<!\d)['’]\d{2}\.\s*\d").unwrap(),
            message: "연도는 네 자리로 표기('24 ✕)", suggest: Some("예) 2025. 1. 6."),
        },
        LintRule {
            code: "DATE_NO_END_DOT", severity: LintSeverity::Warning,
            pattern: Regex::new(r"\b\d{4}\.\s\d{1,2}\.\s\d{1,2}(?!\s*[.\d(])").unwrap(),
            message: "날짜의 '일' 다음에 마침표(.)를 찍어야 함", suggest: Some("예) 2025. 1. 6."),
        },
        // 시간 ─ 24시각제, 쌍점 붙여쓰기
        LintRule {
            code: "TIME_AMPM", severity: LintSeverity::Error,
            pattern: Regex::new(r"(오전|오후|아침|밤|낮)\s*\d{1,2}\s*시").unwrap(),
            message: "24시각제 숫자로 표기(오전/오후 사용 안 함)", suggest: Some("예) 09:00, 15:30"),
        },
        LintRule {
            code: "TIME_24H", severity: LintSeverity::Warning,
            pattern: Regex::new(r"(?<!\d)24\s*시(?!각)").unwrap(),
            message: "'24시'보다 익일 00:00 또는 '18:00까지' 권장", suggest: Some("예) 18:00"),
        },
        LintRule {
            code: "TIME_COLON_SP", severity: LintSeverity::Error,
            pattern: Regex::new(r"\b\d{1,2}\s+:\s*\d{2}\b|\b\d{1,2}:\s+\d{2}\b").unwrap(),
            message: "시와 분 사이 쌍점은 양쪽을 붙여 씀", suggest: Some("예) 13:20"),
        },
        // 금액 ─ '천원' 금지, 금+숫자 붙여쓰기
        LintRule {
            code: "MONEY_CHEONWON", severity: LintSeverity::Error,
            pattern: Regex::new(r"\d+\s*천\s*원").unwrap(),
            message: "금액은 '천원'으로 줄이지 않고 아라비아 숫자로", suggest: Some("예) 345,000원"),
        },
        LintRule {
            code: "MONEY_GEUM_SP", severity: LintSeverity::Warning,
            pattern: Regex::new(r"금\s+\d").unwrap(),
            message: "'금'과 숫자 사이는 붙여 쓰는 것이 원칙", suggest: Some("예) 금113,560원"),
        },
        // 붙임 ─ 쌍점 금지(2타 띄움)
        LintRule {
            code: "BUNIM_COLON", severity: LintSeverity::Error,
            pattern: Regex::new(r"붙\s*임\s*:").unwrap(),
            message: "'붙임' 다음에 쌍점(:)을 붙이지 않음(2타 띄움)", suggest: Some("예) 붙임  계획서 1부."),
        },
        // 표기 ─ 물결표+까지 중복, 한글 먼저, 쌍점 띄어쓰기
        LintRule {
            code: "KKAJI_DUP", severity: LintSeverity::Error,
            pattern: Regex::new(r"[∼~～][^\n]{0,20}?까지").unwrap(),
            message: "물결표(∼)와 '까지'를 함께 쓰지 않음", suggest: Some("예) 2. 20.∼2. 24."),
        },
        LintRule {
            code: "FOREIGN_FIRST", severity: LintSeverity::Warning,
            pattern: Regex::new(r"\b[A-Z]{2,5}\s*\([가-힣]").unwrap(),
            message: "한글을 먼저 쓰고 괄호 안에 외국어를 병기", suggest: Some("예) 업무 협약(MOU)"),
        },
        // 쌍점 — URL(https:// 등)·시각(13:20)은 제외
        LintRule {
            code: "COLON_SPACE", severity: LintSeverity::Warning,
            pattern: Regex::new(r"\S\s+:(?!//)|\S:(?!//)[^\s\d]").unwrap(),
            message: "쌍점은 앞말에 붙이고 뒤는 한 칸 띄움", suggest: Some("예) 원장: 김갑동"),
        },
    ];

    // 펜스는 같은 마커 종류(``` 또는 ~~~)로만 닫힌다
    static ref FENCE_RE: Regex = Regex::new(r"^\s*(```+|~~~+)").unwrap();
}

/// 텍스트(마크다운 포함) 표기법 검수. 마크다운 펜스 코드블록(``` ~ ```) 안은
/// 건너뛴다 — 코드·URL이 날짜/쌍점 규칙에 오탐되는 것 방지.
pub fn lint_document(text: &str) -> Vec<LintIssue> {
    let mut findings = Vec::new();
    // 여는 마커 종류를 기억해, 다른 마커 종류의 줄이 안쪽에 있어도 조기에
    // 열리거나 닫히지 않게 한다.
    let mut fence_marker: Option<char> = None;
    for (i, line) in text.lines().enumerate() {
        if let Ok(Some(caps)) = FENCE_RE.captures(line) {
            let kind = caps.get(1).unwrap().as_str().chars().next().unwrap();
            match fence_marker {
                None => fence_marker = Some(kind),
                Some(open) if open == kind => fence_marker = None,
                _ => {}
            }
            continue;
        }
        if fence_marker.is_some() {
            continue;
        }
        for rule in RULES.iter() {
            for caps_result in rule.pattern.captures_iter(line) {
                let Ok(caps) = caps_result else { continue };
                let Some(m) = caps.get(0) else { continue };
                findings.push(LintIssue {
                    line: i + 1,
                    matched: m.as_str().trim().to_string(),
                    rule: rule.code,
                    severity: rule.severity,
                    message: rule.message,
                    suggest: rule.suggest,
                });
            }
        }
    }
    findings
}

/// 검수 결과를 사람이 읽는 경고 문자열로 — generate 경고 채널용.
pub fn lint_warnings(text: &str, limit: usize) -> Vec<String> {
    let findings = lint_document(text);
    let mut shown: Vec<String> = findings
        .iter()
        .take(limit)
        .map(|f| {
            let suggest = f
                .suggest
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            format!(
                "표기법 [{}] L{} \"{}\" — {}{}",
                f.rule, f.line, f.matched, f.message, suggest
            )
        })
        .collect();
    if findings.len() > limit {
        shown.push(format!(
            "표기법 경고 {}건 더 있음 — 전체 확인 필요",
            findings.len() - limit
        ));
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_hits<'a>(findings: &'a [LintIssue], code: &str) -> Vec<&'a LintIssue> {
        findings.iter().filter(|f| f.rule == code).collect()
    }

    // ── 날짜 ─────────────────────────────────────────────────────
    #[test]
    fn date_no_space_violation() {
        let findings = lint_document("공고일자 2025.1.6.");
        assert_eq!(rule_hits(&findings, "DATE_NO_SPACE").len(), 1);
    }

    #[test]
    fn date_correct_format_passes_no_space_rule() {
        let findings = lint_document("공고일자 2025. 1. 6.");
        assert!(rule_hits(&findings, "DATE_NO_SPACE").is_empty());
    }

    #[test]
    fn date_zero_pad_violation() {
        let findings = lint_document("접수기간 2025. 01. 06.");
        assert!(!rule_hits(&findings, "DATE_ZERO_PAD").is_empty());
    }

    #[test]
    fn date_zero_pad_correct_passes() {
        let findings = lint_document("접수기간 2025. 1. 6.");
        assert!(rule_hits(&findings, "DATE_ZERO_PAD").is_empty());
    }

    #[test]
    fn date_2digit_year_violation() {
        let findings = lint_document("작성일 '24. 3");
        assert_eq!(rule_hits(&findings, "DATE_2DIGIT_YR").len(), 1);
    }

    #[test]
    fn date_no_end_dot_violation() {
        let findings = lint_document("접수 2025. 1. 6 까지 진행합니다");
        assert!(!rule_hits(&findings, "DATE_NO_END_DOT").is_empty());
    }

    #[test]
    fn date_no_end_dot_passes_with_trailing_dot() {
        let findings = lint_document("접수 2025. 1. 6. 까지 진행합니다");
        assert!(rule_hits(&findings, "DATE_NO_END_DOT").is_empty());
    }

    // ── 시간 ─────────────────────────────────────────────────────
    #[test]
    fn time_ampm_violation() {
        let findings = lint_document("오후 3시에 회의를 시작합니다");
        assert_eq!(rule_hits(&findings, "TIME_AMPM").len(), 1);
    }

    #[test]
    fn time_24h_format_passes_ampm_rule() {
        let findings = lint_document("15:00에 회의를 시작합니다");
        assert!(rule_hits(&findings, "TIME_AMPM").is_empty());
    }

    #[test]
    fn time_colon_space_violation() {
        let findings = lint_document("회의는 13 : 20 시작");
        assert!(!rule_hits(&findings, "TIME_COLON_SP").is_empty());
    }

    #[test]
    fn time_colon_tight_passes() {
        let findings = lint_document("회의는 13:20 시작");
        assert!(rule_hits(&findings, "TIME_COLON_SP").is_empty());
    }

    // ── 금액 ─────────────────────────────────────────────────────
    #[test]
    fn money_cheonwon_violation() {
        let findings = lint_document("지원금 345천원 지급");
        assert_eq!(rule_hits(&findings, "MONEY_CHEONWON").len(), 1);
    }

    #[test]
    fn money_geum_space_violation() {
        let findings = lint_document("금 113,560원 지급");
        assert_eq!(rule_hits(&findings, "MONEY_GEUM_SP").len(), 1);
    }

    #[test]
    fn money_geum_no_space_passes() {
        let findings = lint_document("금113,560원 지급");
        assert!(rule_hits(&findings, "MONEY_GEUM_SP").is_empty());
    }

    // ── 붙임 ─────────────────────────────────────────────────────
    #[test]
    fn bunim_colon_violation() {
        let findings = lint_document("붙임: 계획서 1부.");
        assert_eq!(rule_hits(&findings, "BUNIM_COLON").len(), 1);
    }

    #[test]
    fn bunim_double_space_passes() {
        let findings = lint_document("붙임  계획서 1부.");
        assert!(rule_hits(&findings, "BUNIM_COLON").is_empty());
    }

    // ── 물결표/까지, 외국어 병기 ──────────────────────────────────
    #[test]
    fn kkaji_dup_violation() {
        let findings = lint_document("기간 2. 20.∼2. 24.까지");
        assert_eq!(rule_hits(&findings, "KKAJI_DUP").len(), 1);
    }

    #[test]
    fn kkaji_without_dup_passes() {
        let findings = lint_document("기간 2. 20.∼2. 24.");
        assert!(rule_hits(&findings, "KKAJI_DUP").is_empty());
    }

    #[test]
    fn foreign_first_violation() {
        let findings = lint_document("업무 MOU(협약) 체결");
        assert_eq!(rule_hits(&findings, "FOREIGN_FIRST").len(), 1);
    }

    #[test]
    fn foreign_after_korean_passes() {
        let findings = lint_document("업무 협약(MOU) 체결");
        assert!(rule_hits(&findings, "FOREIGN_FIRST").is_empty());
    }

    // ── 쌍점 ─────────────────────────────────────────────────────
    #[test]
    fn colon_space_violation() {
        let findings = lint_document("원장 : 김갑동");
        assert!(!rule_hits(&findings, "COLON_SPACE").is_empty());
    }

    #[test]
    fn colon_url_not_flagged() {
        let findings = lint_document("참고 https://example.com 확인");
        assert!(rule_hits(&findings, "COLON_SPACE").is_empty());
    }

    // ── 펜스 코드블록 제외 ───────────────────────────────────────
    #[test]
    fn fenced_code_block_skipped() {
        let text = "```\n2025.1.6.\n오후 3시\n```\n실제 위반 2025.1.6.";
        let findings = lint_document(text);
        // 코드블록 밖의 위반 1건만 잡혀야 함
        assert_eq!(rule_hits(&findings, "DATE_NO_SPACE").len(), 1);
        assert_eq!(findings.iter().find(|f| f.rule == "DATE_NO_SPACE").unwrap().line, 5);
    }

    #[test]
    fn tilde_fence_marker_respected() {
        let text = "~~~\n2025.1.6.\n~~~";
        let findings = lint_document(text);
        assert!(rule_hits(&findings, "DATE_NO_SPACE").is_empty());
    }

    // ── 라인 번호 ────────────────────────────────────────────────
    #[test]
    fn line_number_is_1_based_and_accurate() {
        let text = "정상 문장\n둘째 줄 2025.1.6. 위반";
        let findings = lint_document(text);
        let hit = findings.iter().find(|f| f.rule == "DATE_NO_SPACE").unwrap();
        assert_eq!(hit.line, 2);
    }

    // ── warnings 헬퍼 ────────────────────────────────────────────
    #[test]
    fn lint_warnings_formats_and_truncates() {
        let text = "2025.1.6.\n2025.2.7.\n2025.3.8.";
        let warnings = lint_warnings(text, 2);
        assert_eq!(warnings.len(), 3); // 2건 + "더 있음" 안내 1건
        assert!(warnings[2].contains("더 있음"));
    }
}
