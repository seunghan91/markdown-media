// Ported from kkdoc (MIT): src/form/recognize.ts
//! Form (서식) field recognition — table-based label/value pattern matching.

use lazy_static::lazy_static;
use regex::Regex;

/// Korean official-document field label keywords.
pub const LABEL_KEYWORDS: &[&str] = &[
    "성명", "이름", "주소", "전화", "전화번호", "휴대폰", "핸드폰", "연락처",
    "생년월일", "주민등록번호", "소속", "직위", "직급", "부서",
    "이메일", "팩스", "학교", "학년", "반", "번호",
    "신청인", "대표자", "담당자", "작성자", "확인자", "승인자",
    "일시", "날짜", "기간", "장소", "목적", "사유", "비고",
    "금액", "수량", "단가", "합계", "계", "소계",
    "등록기준지", "본적", "위임인", "청구사유", "소명자료",
];

const ENGLISH_LABEL_WORDS: &[&str] = &[
    "name", "date", "address", "tel", "phone", "mobile", "fax", "email", "e-mail",
    "dept", "department", "division", "title", "position", "grade", "rank",
    "birth", "nationality", "sex", "gender", "signature", "sign", "seal",
    "remarks", "note", "period", "place", "purpose", "reason", "amount", "total",
    "sum", "qty", "quantity", "unit", "no", "id", "passport",
];
const ENGLISH_STOPWORDS: &[&str] = &["of", "the", "and", "or", "in"];

lazy_static! {
    static ref NUMERIC_VALUE_RE: Regex = Regex::new(
        r"^제?\d+(?:[.,]\d+)*[십백천만억조]*(?:원|명|건|개|회|부|매|장|점|호|번|년|월|일|시|분|초|개월|주년|차례|퍼센트)?$"
    ).unwrap();
    static ref SENTENCE_ENDING_RE: Regex = Regex::new(
        r"(?:입니다|합니다|습니다|하세요|십시오|시오|바랍니다|바람|할 것|할것|하며|하고|한다|된다|됨|음|임)$"
    ).unwrap();
    static ref COMPACT_LABEL_RE: Regex = Regex::new(r"^[가-힣0-9()（）·:：\-]+$").unwrap();
    static ref COLON_LABEL_RE: Regex = Regex::new(r"^[가-힣A-Za-z\s]+[:：]$").unwrap();
    static ref ENGLISH_LABEL_RE: Regex = Regex::new(r"^[A-Za-z][A-Za-z\s./&-]*$").unwrap();
    static ref TRAILING_MARK_RE: Regex = Regex::new(r"[¹²³⁴⁵⁶⁷⁸⁹⁰*※]+$").unwrap();
    static ref COMPANY_PREFIX_RE: Regex = Regex::new(r"^[(（]주[)）]|^주식회사").unwrap();
}

fn count_hangul(s: &str) -> usize {
    s.chars().filter(|c| ('가'..='힣').contains(c)).count()
}

/// Whether a cell looks like a label cell.
pub fn is_label_cell(text: &str) -> bool {
    let trimmed = TRAILING_MARK_RE.replace(text.trim(), "").trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > 30 {
        return false;
    }
    // keyword match
    if LABEL_KEYWORDS.iter().any(|kw| trimmed.contains(kw)) {
        return true;
    }
    // short hangul text (2-12 chars), digits allowed but not numeric-value/sentence/company
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let clen = compact.chars().count();
    if COMPACT_LABEL_RE.is_match(&compact)
        && (2..=12).contains(&clen)
        && count_hangul(&compact) >= 2
        && (clen <= 8 || trimmed.split_whitespace().count() <= 2)
        && !NUMERIC_VALUE_RE.is_match(&compact)
        && !SENTENCE_ENDING_RE.is_match(&trimmed)
        && !COMPANY_PREFIX_RE.is_match(&compact)
    {
        return true;
    }
    // "label:" pattern
    if COLON_LABEL_RE.is_match(&trimmed) {
        return true;
    }
    // colon-less english label
    if ENGLISH_LABEL_RE.is_match(&trimmed) && trimmed.chars().count() <= 20 {
        let words: Vec<String> = trimmed
            .to_lowercase()
            .split(|c: char| c == ' ' || c == '/' || c == '&')
            .filter(|w| !w.is_empty() && !ENGLISH_STOPWORDS.contains(w))
            .map(|s| s.to_string())
            .collect();
        if (1..=3).contains(&words.len())
            && words.iter().all(|w| ENGLISH_LABEL_WORDS.contains(&w.trim_end_matches('.')))
        {
            return true;
        }
    }
    false
}

/// Inferred form field type — drives form UI widget selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormFieldType {
    Text,
    Date,
    Phone,
    Email,
    Amount,
    Checkbox,
    Idnum,
}

lazy_static! {
    static ref RRN_RE: Regex = Regex::new(r"^\d{6}[-\s]?[1-4]\d{6}$").unwrap();
    static ref DATE_VAL_RE: Regex = Regex::new(r"^\d{4}\s*[-./년]\s*\d{1,2}\s*[-./월]\s*\d{1,2}\s*일?\s*\.?$").unwrap();
    static ref PHONE_VAL_RE: Regex = Regex::new(r"^0\d{1,2}[-.)\s]?\d{3,4}[-.\s]?\d{4}$").unwrap();
    static ref EMAIL_VAL_RE: Regex = Regex::new(r"^[\w.+-]+@[\w-]+(?:\.[\w-]+)+$").unwrap();
    static ref AMOUNT_UNIT_RE: Regex = Regex::new(r"^[\d,.\s]+(?:원|명|건|개|회|부|매|%)$").unwrap();
    static ref AMOUNT_COMMA_RE: Regex = Regex::new(r"^\d{1,3}(?:,\d{3})+$").unwrap();
    static ref REQUIRED_RE: Regex = Regex::new(r"[*※★]|\(\s*필수\s*\)|（\s*필수\s*）").unwrap();
    static ref EMPTY_VAL_RE: Regex = Regex::new(r"^[\s_()（）\-—–~.·,]*$").unwrap();
    // label-keyword → type
    static ref TYPE_IDNUM_RE: Regex = Regex::new(r"주민등록번호|외국인등록번호").unwrap();
    static ref TYPE_DATE_RE: Regex = Regex::new(r"생년월일|일시|날짜|일자|기간|연월일|년월일|신청일|작성일|발급일|접수일").unwrap();
    static ref TYPE_PHONE_RE: Regex = Regex::new(r"전화|연락처|휴대폰|핸드폰|팩스").unwrap();
    static ref TYPE_EMAIL_RE: Regex = Regex::new(r"(?i)이메일|전자우편|email").unwrap();
    static ref TYPE_AMOUNT_RE: Regex = Regex::new(r"금액|단가|수량|합계|소계|예산|비용|인원|급여|연봉").unwrap();
    static ref CHECKBOX_RE: Regex = Regex::new(r"[□☑✓✔]").unwrap();
}

/// Infer field type — existing value pattern first, label keyword fallback.
pub fn infer_field_type(label: &str, value: &str) -> FormFieldType {
    if CHECKBOX_RE.is_match(value) || CHECKBOX_RE.is_match(label) {
        return FormFieldType::Checkbox;
    }
    let v = value.trim();
    if !v.is_empty() {
        if RRN_RE.is_match(v) {
            return FormFieldType::Idnum;
        }
        if DATE_VAL_RE.is_match(v) {
            return FormFieldType::Date;
        }
        if PHONE_VAL_RE.is_match(v) {
            return FormFieldType::Phone;
        }
        if EMAIL_VAL_RE.is_match(v) {
            return FormFieldType::Email;
        }
        if AMOUNT_UNIT_RE.is_match(v) && v.chars().any(|c| c.is_ascii_digit()) {
            return FormFieldType::Amount;
        }
        if AMOUNT_COMMA_RE.is_match(v) {
            return FormFieldType::Amount;
        }
    }
    let norm: String = label.chars().filter(|c| !c.is_whitespace()).collect();
    if TYPE_IDNUM_RE.is_match(&norm) {
        return FormFieldType::Idnum;
    }
    if TYPE_DATE_RE.is_match(&norm) {
        return FormFieldType::Date;
    }
    if TYPE_PHONE_RE.is_match(&norm) {
        return FormFieldType::Phone;
    }
    if TYPE_EMAIL_RE.is_match(&norm) {
        return FormFieldType::Email;
    }
    if TYPE_AMOUNT_RE.is_match(&norm) {
        return FormFieldType::Amount;
    }
    FormFieldType::Text
}

/// Whether the label carries a required marker (※·*·★·"(필수)").
pub fn is_required_label(label: &str) -> bool {
    REQUIRED_RE.is_match(label)
}

/// Whether the value is empty or placeholder-only (underscores/parens/dashes).
pub fn is_empty_value(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() {
        return true;
    }
    EMPTY_VAL_RE.is_match(v)
}
