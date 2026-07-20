// Ported from kkdoc (MIT): src/form/match.ts
//! Form field matching utilities — label normalization, prefix matching,
//! multi-value cursor, and value formatting. Shared by fill strategies.

use std::collections::HashMap;

use super::recognize::LABEL_KEYWORDS;

/// A fill value: a scalar (repeated for every occurrence of the same label)
/// or a list (consumed one-per-occurrence, for repeated forms / roster tables).
#[derive(Debug, Clone)]
pub enum FillValue {
    Scalar(String),
    List(Vec<String>),
}

impl From<&str> for FillValue {
    fn from(s: &str) -> Self {
        FillValue::Scalar(s.to_string())
    }
}
impl From<String> for FillValue {
    fn from(s: String) -> Self {
        FillValue::Scalar(s)
    }
}
impl From<Vec<String>> for FillValue {
    fn from(v: Vec<String>) -> Self {
        FillValue::List(v)
    }
}

/// Multi-value cursor — tracks per-label consumption state.
/// Scalars repeat indefinitely; lists are consumed in appearance order and,
/// once exhausted, `consume` returns `None` so later occurrences stay empty.
pub struct ValueCursor {
    values: HashMap<String, FillValue>,
    next_idx: HashMap<String, usize>,
}

impl ValueCursor {
    pub fn new(values: HashMap<String, FillValue>) -> Self {
        Self { values, next_idx: HashMap::new() }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn is_array(&self, key: &str) -> bool {
        matches!(self.values.get(key), Some(FillValue::List(_)))
    }

    /// Whether a remaining value exists (scalars: always true).
    pub fn available(&self, key: &str) -> bool {
        match self.values.get(key) {
            Some(FillValue::Scalar(_)) => true,
            Some(FillValue::List(v)) => *self.next_idx.get(key).unwrap_or(&0) < v.len(),
            None => false,
        }
    }

    /// Preview the current value without consuming.
    pub fn peek(&self, key: &str) -> Option<&str> {
        match self.values.get(key)? {
            FillValue::Scalar(s) => Some(s.as_str()),
            FillValue::List(v) => {
                let i = *self.next_idx.get(key).unwrap_or(&0);
                v.get(i).map(|s| s.as_str())
            }
        }
    }

    /// Consume a value — advances the cursor for lists; `None` when exhausted.
    pub fn consume(&mut self, key: &str) -> Option<String> {
        match self.values.get(key)? {
            FillValue::Scalar(s) => Some(s.clone()),
            FillValue::List(v) => {
                let i = *self.next_idx.get(key).unwrap_or(&0);
                if i >= v.len() {
                    return None;
                }
                let out = v[i].clone();
                self.next_idx.insert(key.to_string(), i + 1);
                Some(out)
            }
        }
    }
}

/// Normalize a label for comparison — strip colons, whitespace, parens, middots.
pub fn normalize_label(label: &str) -> String {
    label
        .chars()
        .filter(|c| !matches!(c, ':' | '：' | '(' | ')' | '（' | '）' | '·') && !c.is_whitespace())
        .collect()
}

/// Find the best matching key for a normalized cell label.
/// Priority: (1) exact match, (2) prefix match (>=60% overlap, longest wins).
pub fn find_matching_key(cell_label: &str, cursor: &ValueCursor) -> Option<String> {
    if cursor.has(cell_label) {
        return Some(cell_label.to_string());
    }
    let cell_len = cell_label.chars().count() as f64;
    let mut best_key: Option<String> = None;
    let mut best_len = 0usize;
    for key in cursor.keys() {
        let key_len = key.chars().count();
        if cell_label.starts_with(key.as_str()) {
            if key_len as f64 >= cell_len * 0.6 && key_len > best_len {
                best_len = key_len;
                best_key = Some(key.clone());
            }
        } else if key.starts_with(cell_label) {
            // reverse (short cell absorbs long key) is stricter: 0.75
            let cl = cell_label.chars().count();
            if cl as f64 >= key_len as f64 * 0.75 && cl > best_len {
                best_len = cl;
                best_key = Some(key.clone());
            }
        }
    }
    best_key
}

/// Whether the (value) cell text is itself a keyword label → skip target.
pub fn is_keyword_label(text: &str) -> bool {
    let trimmed = text
        .trim()
        .trim_end_matches(|c| matches!(c, '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹' | '⁰' | '*' | '※'))
        .trim();
    if trimmed.is_empty() || trimmed.chars().count() > 15 {
        return false;
    }
    LABEL_KEYWORDS.iter().any(|kw| trimmed.contains(kw))
}

/// Normalize the input value map to normalized keys, applying `format` if given.
/// Collisions after normalization push a warning.
pub fn normalize_values(
    values: &HashMap<String, RawFillInput>,
    warnings: &mut Vec<String>,
) -> HashMap<String, FillValue> {
    let mut map: HashMap<String, FillValue> = HashMap::new();
    for (label, raw) in values {
        let key = normalize_label(label);
        if map.contains_key(&key) {
            warnings.push(format!(
                "입력 라벨 \"{label}\"이 정규화 키 \"{key}\"에서 다른 라벨과 충돌 — 뒤 값으로 덮어씀"
            ));
        }
        let fmt = raw.format.as_deref();
        let value = match &raw.value {
            FillValue::Scalar(s) => FillValue::Scalar(format_fill_value(s, fmt)),
            FillValue::List(v) => FillValue::List(v.iter().map(|s| format_fill_value(s, fmt)).collect()),
        };
        map.insert(key, value);
    }
    map
}

/// A fill input carrying an optional format directive (see `format_fill_value`).
#[derive(Debug, Clone)]
pub struct RawFillInput {
    pub value: FillValue,
    pub format: Option<String>,
}

impl<T: Into<FillValue>> From<T> for RawFillInput {
    fn from(v: T) -> Self {
        RawFillInput { value: v.into(), format: None }
    }
}

/// Restore unmatched normalized keys back to their original input labels.
pub fn resolve_unmatched(
    normalized: &HashMap<String, FillValue>,
    matched: &std::collections::HashSet<String>,
    original: &HashMap<String, RawFillInput>,
) -> Vec<String> {
    normalized
        .keys()
        .filter(|k| !matched.contains(*k))
        .map(|k| {
            for orig in original.keys() {
                if &normalize_label(orig) == k {
                    return orig.clone();
                }
            }
            k.clone()
        })
        .collect()
}

// ─── value formatting (claw-hwp secure-fill port) ──────────────────────────

struct Ymd {
    y: String,
    yy: String,
    m: String,
    d: String,
}

fn parse_ymd(v: &str) -> Option<Ymd> {
    let parts: Vec<&str> = v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
    let (y, m, d);
    if parts.len() == 3 {
        let yp = parts[0];
        let yn: i32 = yp.parse().ok()?;
        y = if yp.len() >= 3 {
            yp.to_string()
        } else if yn <= 29 {
            (2000 + yn).to_string()
        } else {
            (1900 + yn).to_string()
        };
        m = format!("{:0>2}", parts[1]);
        d = format!("{:0>2}", parts[2]);
    } else {
        let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 8 {
            y = digits[0..4].to_string();
            m = digits[4..6].to_string();
            d = digits[6..8].to_string();
        } else if digits.len() == 6 {
            let yy: i32 = digits[0..2].parse().ok()?;
            y = if yy <= 29 { (2000 + yy).to_string() } else { (1900 + yy).to_string() };
            m = digits[2..4].to_string();
            d = digits[4..6].to_string();
        } else {
            return None;
        }
    }
    let mi: i32 = m.parse().ok()?;
    let di: i32 = d.parse().ok()?;
    if !(1..=12).contains(&mi) || !(1..=31).contains(&di) {
        return None;
    }
    let yy = y[y.len().saturating_sub(2)..].to_string();
    Some(Ymd { y, yy, m, d })
}

fn fmt_date(v: &str, style: &str) -> String {
    let p = match parse_ymd(v) {
        Some(p) => p,
        None => return v.to_string(),
    };
    let style = if style.is_empty() { "yyyy-mm-dd" } else { style };
    // longest token first; case-insensitive done via lowercase scan replace
    let mut out = style.to_string();
    for (tok, rep) in [("yyyy", &p.y), ("yy", &p.yy), ("mm", &p.m), ("dd", &p.d)] {
        out = replace_ci(&out, tok, rep);
    }
    out
}

fn replace_ci(hay: &str, needle: &str, rep: &str) -> String {
    // simple case-insensitive replace over ASCII tokens (yyyy/mm/dd)
    let lower = hay.to_lowercase();
    let nl = needle.to_lowercase();
    let mut out = String::new();
    let bytes = hay.as_bytes();
    let mut i = 0;
    while i < hay.len() {
        if lower[i..].starts_with(&nl) {
            out.push_str(rep);
            i += needle.len();
        } else {
            // advance one char (ASCII-safe for our styles)
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn fmt_phone(v: &str, style: &str) -> String {
    let d: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() < 9 {
        return v.to_string();
    }
    let area_len = if d.starts_with("02") { 2 } else { 3 };
    let a = &d[0..area_len];
    let b = &d[area_len..d.len() - 4];
    let c = &d[d.len() - 4..];
    match style {
        "digits" => d.clone(),
        "dot" => format!("{a}.{b}.{c}"),
        "space" => format!("{a} {b} {c}"),
        _ => format!("{a}-{b}-{c}"),
    }
}

fn fmt_rrn(v: &str, style: &str) -> String {
    let d: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    if d.len() != 13 {
        return v.to_string();
    }
    match style {
        "digits" => d.clone(),
        "front" => d[0..6].to_string(),
        "masked" => format!("{}-{}******", &d[0..6], &d[6..7]),
        _ => format!("{}-{}", &d[0..6], &d[6..]),
    }
}

fn mask_digits(v: &str, pattern: &str) -> String {
    let ds: Vec<char> = v.chars().filter(|c| c.is_ascii_digit()).collect();
    let need = pattern.chars().filter(|c| *c == '#').count();
    if need == 0 || ds.len() != need {
        return v.to_string();
    }
    let mut it = ds.into_iter();
    pattern.chars().map(|c| if c == '#' { it.next().unwrap() } else { c }).collect()
}

/// Transform a value per a `kind:style` (or free) format directive.
/// Unknown formats return the value unchanged (fail-open).
pub fn format_fill_value(value: &str, format: Option<&str>) -> String {
    let format = match format {
        Some(f) if !f.is_empty() => f,
        _ => return value.to_string(),
    };
    let (kind, style) = match format.find(':') {
        Some(ci) => (&format[..ci], &format[ci + 1..]),
        None => (format, ""),
    };
    match kind {
        "date" => fmt_date(value, style),
        "phone" => fmt_phone(value, style),
        "rrn" => fmt_rrn(value, style),
        "mask" => mask_digits(value, style),
        "digits" => {
            let only: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
            if only.is_empty() { value.to_string() } else { only }
        }
        "upper" => value.to_uppercase(),
        "lower" => value.to_lowercase(),
        "nospace" => value.chars().filter(|c| !c.is_whitespace()).collect(),
        _ => {
            if format.contains('#') {
                mask_digits(value, format)
            } else if format.contains("yyyy") || format.contains("yy") || format.contains("mm") || format.contains("dd") {
                fmt_date(value, format)
            } else {
                value.to_string()
            }
        }
    }
}
