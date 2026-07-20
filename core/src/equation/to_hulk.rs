// Ported from kkdoc (MIT): src/hwpx/equation-generate.ts (`latexLikeToEqEdit`
// and friends) — the reverse direction of to_latex.rs (`hulk_to_latex`).
//
// Vocabulary is kept in lock-step with the read side (tables.rs's
// CONVERT_MAP/MIDDLE_CONVERT_MAP): every token this module emits must be one
// `hulk_to_latex` reads back to the same LaTeX command (see the round-trip
// fixed-point test in tests.rs, ported from
// hwpx-equation-generation.test.ts's "전 토큰 왕복 정합" suite).

use std::collections::{HashMap, HashSet};

use lazy_static::lazy_static;
use regex::Regex;

use super::find_substring;
use super::tables::{CONVERT_MAP, MIDDLE_CONVERT_MAP};

// Untrusted-input guards — markdown_to_hwpx (HWPX generator) accepts
// arbitrary MCP/CLI input.
const MAX_EQUATION_SOURCE: usize = 10_000;
const MAX_GROUP_DEPTH: usize = 64;

// LaTeX command -> EqEdit token. Every value here must be a token the read
// map (CONVERT_MAP) sends back to the same LaTeX command.
static EXPLICIT_COMMAND_MAP: &[(&str, &str)] = &[
    ("alpha", "alpha"), ("beta", "beta"), ("gamma", "gamma"), ("delta", "delta"),
    ("epsilon", "epsilon"), ("zeta", "zeta"), ("eta", "eta"), ("theta", "theta"),
    ("iota", "iota"), ("kappa", "kappa"), ("lambda", "lambda"), ("mu", "mu"),
    ("nu", "nu"), ("xi", "xi"), ("pi", "pi"), ("rho", "rho"), ("sigma", "sigma"),
    ("tau", "tau"), ("upsilon", "upsilon"), ("phi", "phi"), ("chi", "chi"),
    ("psi", "psi"), ("omega", "omega"),
    ("Gamma", "GAMMA"), ("Delta", "DELTA"), ("Theta", "THETA"), ("Lambda", "LAMBDA"),
    ("Xi", "XI"), ("Pi", "PI"), ("Sigma", "SIGMA"), ("Upsilon", "UPSILON"),
    ("Phi", "PHI"), ("Psi", "PSI"), ("Omega", "OMEGA"),
    ("le", "LEQ"), ("leq", "LEQ"), ("ge", "GEQ"), ("geq", "GEQ"),
    ("ne", "!="), ("neq", "!="),
    ("pm", "+-"), ("mp", "-+"),
    ("times", "TIMES"), ("cdot", "cdot"),
    ("ast", "AST"), ("circ", "CIRC"), ("bullet", "BULLET"),
    ("in", "IN"), ("notin", "NOTIN"),
    ("subset", "SUBSET"), ("subseteq", "SUBSETEQ"), ("supset", "SUPERSET"), ("supseteq", "SUPSETEQ"),
    ("cup", "CUP"), ("cap", "SMALLINTER"),
    ("emptyset", "EMPTYSET"), ("forall", "FORALL"), ("exists", "EXIST"),
    ("infinity", "INF"), ("infty", "INF"),
    ("partial", "Partial"), ("nabla", "NABLA"),
    ("int", "int"), ("iint", "dint"), ("iiint", "tint"), ("oint", "oint"),
    ("sum", "sum"), ("prod", "prod"), ("lim", "lim"),
    ("to", "->"), ("rightarrow", "->"), ("leftarrow", "larrow"), ("leftrightarrow", "<->"),
    ("Rightarrow", "RARROW"), ("Leftarrow", "LARROW"), ("Leftrightarrow", "LRARROW"),
    ("cdots", "CDOTS"), ("ldots", "LDOTS"), ("vdots", "VDOTS"), ("ddots", "DDOTS"),
];

// Accents (also structural, ported from MIDDLE_CONVERT_MAP tokens). A round
// trip fixed point: value token -> hulk_to_latex's command must be a key here
// again (bar -> \overline -> bar). overrightarrow is vec's read-side alias.
static ACCENT_COMMANDS_ENTRIES: &[(&str, &str)] = &[
    ("bar", "bar"),
    ("overline", "bar"),
    ("vec", "vec"),
    ("overrightarrow", "vec"),
    ("hat", "hat"),
    ("widehat", "hat"),
    ("tilde", "tilde"),
    ("widetilde", "tilde"),
    ("dot", "dot"),
    ("ddot", "ddot"),
    ("underline", "under"),
];

/// EqEdit function keywords the renderer knows as upright function names —
/// not a "command" to hulk_to_latex (read back as plain text like `sin`), so
/// excluded from the unsupported-command quoting fallback (identity pass-through).
static EQEDIT_FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc",
    "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "coth",
    "log", "ln", "exp", "det", "gcd", "mod",
    "max", "min", "arg", "deg", "hom", "ker", "Pr",
];

fn single_command_name(latex: &str) -> Option<&str> {
    let rest = latex.strip_prefix('\\')?;
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(rest)
    } else {
        None
    }
}

/// Same LaTeX command, multiple candidate tokens: prefer the exact name
/// match, then an alphabetic token, then lexicographic order. Fold order
/// over CONVERT_MAP is irrelevant since `prefer` only looks at properties
/// of its two arguments (not visit order).
fn prefer<'a>(name: &str, a: &'a str, b: &'a str) -> &'a str {
    if a == name {
        return a;
    }
    if b == name {
        return b;
    }
    let a_alpha = !a.is_empty() && a.chars().all(|c| c.is_ascii_alphabetic());
    let b_alpha = !b.is_empty() && b.chars().all(|c| c.is_ascii_alphabetic());
    if a_alpha != b_alpha {
        return if a_alpha { a } else { b };
    }
    if a < b { a } else { b }
}

/// Backfills CONVERT_MAP's ~150 read-side operators (`\div \approx
/// \therefore \because \oplus \uparrow \propto \cong \equiv \sim \angle
/// \mapsto \ll \gg \dagger \models \owns` etc — ~60 entries) that had no
/// write-side token, which otherwise leaked as a bare, unescaped command
/// name into EqEdit. Only single-command values (`^\[A-Za-z]+$`) are
/// reverse-indexed; composite values (`\mathop ⩅`, `{\Large\ominus}`,
/// `^{\circ}C` — read-only) are excluded. Explicit entries always win
/// (keeps existing canonical forms like leq -> LEQ).
fn build_command_map() -> HashMap<String, String> {
    let mut m: HashMap<String, String> = EXPLICIT_COMMAND_MAP
        .iter()
        .map(|&(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let mut reverse: HashMap<String, String> = HashMap::new();
    for (&token, &latex) in CONVERT_MAP.iter() {
        let Some(name) = single_command_name(latex) else { continue };
        if m.contains_key(name) {
            continue;
        }
        match reverse.get(name) {
            Some(prev) => {
                let preferred = prefer(name, token, prev).to_string();
                reverse.insert(name.to_string(), preferred);
            }
            None => {
                reverse.insert(name.to_string(), token.to_string());
            }
        }
    }
    for (name, token) in reverse {
        m.insert(name, token);
    }
    m
}

fn is_alpha_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic())
}

lazy_static! {
    static ref COMMAND_MAP: HashMap<String, String> = build_command_map();
    static ref ACCENT_COMMANDS: HashMap<&'static str, &'static str> =
        ACCENT_COMMANDS_ENTRIES.iter().copied().collect();
    static ref EQEDIT_FUNCTIONS_SET: HashSet<&'static str> = EQEDIT_FUNCTIONS.iter().copied().collect();

    // EqEdit vocabulary a bare subscript/superscript word could collide with
    // (T_{int} rendering as T_{\int} instead of literal T sub int).
    static ref RESERVED_WORDS: HashSet<String> = {
        let mut s: HashSet<String> = HashSet::new();
        for k in CONVERT_MAP.keys() {
            if is_alpha_word(k) {
                s.insert((*k).to_string());
            }
        }
        for k in MIDDLE_CONVERT_MAP.keys() {
            if is_alpha_word(k) {
                s.insert((*k).to_string());
            }
        }
        s.insert("over".to_string());
        s.insert("root".to_string());
        s.insert("of".to_string());
        s
    };

    static ref WHITESPACE_RE: Regex = Regex::new(r"\s+").unwrap();
    static ref RESERVED_SCRIPT_RE: Regex = Regex::new(r"([_^])\s*\{\s*([A-Za-z]+)\s*\}").unwrap();
}

struct ReadResult {
    value: String,
    next: usize,
}

fn skip_spaces(chars: &[char], mut idx: usize) -> usize {
    while idx < chars.len() && chars[idx].is_whitespace() {
        idx += 1;
    }
    idx
}

fn normalize_eq_edit(input: &str) -> String {
    WHITESPACE_RE.replace_all(input.trim(), " ").into_owned()
}

fn strip_math_delimiters(input: &str) -> String {
    let s = input.trim();
    if s.len() >= 4 && s.starts_with("$$") && s.ends_with("$$") {
        return s[2..s.len() - 2].trim().to_string();
    }
    if s.len() >= 4 && s.starts_with("\\[") && s.ends_with("\\]") {
        return s[2..s.len() - 2].trim().to_string();
    }
    s.to_string()
}

fn read_balanced(chars: &[char], idx: usize, open: char, close: char) -> ReadResult {
    let mut depth: i32 = 1;
    let mut cursor = idx + 1;
    while cursor < chars.len() {
        let ch = chars[cursor];
        if ch == '\\' {
            cursor += 2;
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
        }
        if depth == 0 {
            let value: String = chars[idx + 1..cursor].iter().collect();
            return ReadResult { value, next: cursor + 1 };
        }
        cursor += 1;
    }
    let value: String = chars.get(idx + 1..).unwrap_or(&[]).iter().collect();
    ReadResult { value, next: chars.len() }
}

fn read_group_or_token(chars: &[char], idx: usize, depth: usize) -> ReadResult {
    let start = skip_spaces(chars, idx);
    // Nesting cap exceeded — fall back to the remaining input as a literal
    // (stack-overflow guard against malformed input).
    if depth > MAX_GROUP_DEPTH {
        let value: String = chars.get(start..).unwrap_or(&[]).iter().collect();
        return ReadResult { value, next: chars.len() };
    }
    if start < chars.len() && chars[start] == '{' {
        let group = read_balanced(chars, start, '{', '}');
        let value = convert_latex_fragment(&group.value, depth + 1);
        return ReadResult { value, next: group.next };
    }
    if start < chars.len() && chars[start] == '\\' {
        return read_command(chars, start, depth + 1);
    }
    if start < chars.len() {
        ReadResult { value: chars[start].to_string(), next: start + 1 }
    } else {
        ReadResult { value: String::new(), next: chars.len() }
    }
}

fn read_command_name(chars: &[char], idx: usize) -> ReadResult {
    if idx + 1 < chars.len() && chars[idx + 1] == '\\' {
        return ReadResult { value: "\\".to_string(), next: idx + 2 };
    }
    let mut end = idx + 1;
    while end < chars.len() && chars[end].is_ascii_alphabetic() {
        end += 1;
    }
    if end > idx + 1 {
        let value: String = chars[idx + 1..end].iter().collect();
        ReadResult { value, next: end }
    } else {
        let value = chars.get(idx + 1).map(|c| c.to_string()).unwrap_or_default();
        ReadResult { value, next: (idx + 2).min(chars.len()) }
    }
}

fn env_token(env: &str) -> Option<&'static str> {
    match env {
        "matrix" => Some("matrix"),
        "pmatrix" => Some("pmatrix"),
        "bmatrix" => Some("bmatrix"),
        "vmatrix" => Some("dmatrix"),
        "cases" => Some("cases"),
        "align" | "align*" | "aligned" => Some("eqalign"),
        _ => None,
    }
}

fn read_command(chars: &[char], idx: usize, depth: usize) -> ReadResult {
    let name = read_command_name(chars, idx);
    let command = name.value.clone();

    if command == "\\" {
        return ReadResult { value: "#".to_string(), next: name.next };
    }

    if command == "frac" {
        let num = read_group_or_token(chars, name.next, depth);
        let den = read_group_or_token(chars, num.next, depth);
        return ReadResult { value: format!("{{{}}} over {{{}}}", num.value, den.value), next: den.next };
    }

    if command == "sqrt" {
        let mut cursor = skip_spaces(chars, name.next);
        let mut root: Option<String> = None;
        if cursor < chars.len() && chars[cursor] == '[' {
            let opt = read_balanced(chars, cursor, '[', ']');
            root = Some(convert_latex_fragment(&opt.value, depth + 1));
            cursor = opt.next;
        }
        let body = read_group_or_token(chars, cursor, depth);
        return match root {
            Some(r) => ReadResult { value: format!("root {{{}}} of {{{}}}", r, body.value), next: body.next },
            None => ReadResult { value: format!("sqrt{{{}}}", body.value), next: body.next },
        };
    }

    if command == "begin" {
        let env = read_group_or_token(chars, name.next, depth);
        let end_tag = format!("\\end{{{}}}", env.value);
        let Some(end_idx) = find_substring(chars, &end_tag, env.next) else {
            return ReadResult { value: env.value, next: env.next };
        };
        let body_raw: String = chars[env.next..end_idx].iter().collect();
        let body = convert_latex_fragment(&body_raw, depth + 1);
        let next = end_idx + end_tag.chars().count();

        // Environment -> EqEdit native token whitelist — more accurate
        // round-trip than LEFT (/RIGHT ) composition. Fixed points:
        // matrix/pmatrix/bmatrix restore the original environment,
        // cases -> HULKCASE -> \begin{cases}, vmatrix -> dmatrix ->
        // \begin{vmatrix}. align family renders via eqalign (plain-TeX
        // vocabulary, so not a fixed point — but lossless render/content).
        if let Some(tok) = env_token(&env.value) {
            return ReadResult { value: format!("{{{}{{{}}}}}", tok, body), next };
        }
        // Bmatrix (braced matrix) — no dedicated token, composed via
        // LEFT {/RIGHT } (round-trips through
        // \left\{\begin{matrix}...\end{matrix}\right\} — render/content preserved).
        if env.value == "Bmatrix" {
            return ReadResult { value: format!("LEFT {{ {{matrix{{{}}}}} RIGHT }}", body), next };
        }
        // Unsupported environment (vmatrix* etc) — strip the wrapper only,
        // keep the body.
        ReadResult { value: body, next }
    } else if command == "left" || command == "right" {
        let kw = if command == "left" { "LEFT" } else { "RIGHT" };
        let cursor = skip_spaces(chars, name.next);
        let mut delimiter = String::new();
        let mut next = cursor;
        if let Some(&ch0) = chars.get(cursor) {
            delimiter.push(ch0);
            next = cursor + 1;
        }
        if delimiter == "\\" {
            // Escaped delimiters like \{ \} \| — no leftover backslash
            // ("LEFT {" is the vocabulary hulk_to_latex's replace_bracket
            // restores to \left \{).
            let escaped = read_command_name(chars, cursor);
            delimiter = if escaped.value == "\\" {
                "\\".to_string()
            } else {
                CONVERT_MAP.get(escaped.value.as_str()).map(|s| s.to_string()).unwrap_or(escaped.value.clone())
            };
            next = escaped.next;
        }
        let value = if delimiter.is_empty() { kw.to_string() } else { format!("{} {}", kw, delimiter) };
        ReadResult { value, next }
    } else if let Some(&accent) = ACCENT_COMMANDS.get(command.as_str()) {
        let body = read_group_or_token(chars, name.next, depth);
        ReadResult { value: format!("{}{{{}}}", accent, body.value), next: body.next }
    } else if command == "," {
        ReadResult { value: "`".to_string(), next: name.next }
    } else if command == ";" || command == ":" {
        ReadResult { value: "~".to_string(), next: name.next }
    } else if command == "!" {
        ReadResult { value: String::new(), next: name.next }
    } else if command == "mathrm" || command == "text" {
        // Literal text — not converted, protected with EqEdit quotes (keeps
        // `int` from rendering as an integral). hulk_to_latex reads a
        // single-token quote back as \text{...}, forming a fixed point.
        let start = skip_spaces(chars, name.next);
        if start < chars.len() && chars[start] == '{' {
            let group = read_balanced(chars, start, '{', '}');
            ReadResult { value: format!("\"{}\"", group.value), next: group.next }
        } else {
            let tok = read_group_or_token(chars, start, depth);
            ReadResult { value: format!("\"{}\"", tok.value), next: tok.next }
        }
    } else if let Some(mapped) = COMMAND_MAP.get(command.as_str()) {
        ReadResult { value: mapped.clone(), next: name.next }
    } else if EQEDIT_FUNCTIONS_SET.contains(command.as_str()) {
        ReadResult { value: command.clone(), next: name.next }
    } else if command.chars().count() >= 2 && command.chars().all(|c| c.is_ascii_alphabetic()) {
        // Unsupported command — a bare "alphabet-stripped" name renders as
        // an italicized variable sequence in EqEdit, a silent corruption.
        // Protect with a literal quote to make the loss visible and stable
        // (hulk_to_latex reads "..." back as \text{...}). Single characters
        // (\, delimiters, etc) pass through unchanged — don't corrupt
        // structural vocabulary.
        ReadResult { value: format!("\"{}\"", command), next: name.next }
    } else {
        ReadResult { value: command.clone(), next: name.next }
    }
}

fn convert_latex_fragment(input: &str, depth: usize) -> String {
    // Brace-bomb etc malformed input — fall back to a literal past the cap.
    if depth > MAX_GROUP_DEPTH {
        return normalize_eq_edit(input);
    }

    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut idx = 0;

    while idx < chars.len() {
        let ch = chars[idx];
        if ch == '\\' {
            let cmd = read_command(&chars, idx, depth + 1);
            out.push(' ');
            out.push_str(&cmd.value);
            out.push(' ');
            idx = cmd.next;
            continue;
        }
        if ch == '{' {
            let group = read_balanced(&chars, idx, '{', '}');
            out.push('{');
            out.push_str(&convert_latex_fragment(&group.value, depth + 1));
            out.push('}');
            idx = group.next;
            continue;
        }
        if ch == '_' || ch == '^' {
            let script = read_group_or_token(&chars, idx + 1, depth);
            out.push(' ');
            out.push(ch);
            out.push('{');
            out.push_str(&script.value);
            out.push('}');
            idx = script.next;
            continue;
        }
        if ch == '&' {
            out.push_str(" & ");
            idx += 1;
            continue;
        }
        out.push(ch);
        idx += 1;
    }

    normalize_eq_edit(&out)
}

/// LaTeX source stage only: quote reserved-word literal sub/superscripts.
/// Applied before conversion only — output (\pi -> pi etc) is not re-quoted.
pub fn quote_reserved_keywords(latex: &str) -> String {
    RESERVED_SCRIPT_RE
        .replace_all(latex, |caps: &regex::Captures| {
            let op = &caps[1];
            let word = &caps[2];
            if RESERVED_WORDS.contains(word) {
                format!("{}{{\"{}\"}}", op, word)
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

/// Convert a LaTeX-like source string (Markdown display math) to a Hancom
/// EqEdit script — the reverse of `hulk_to_latex`.
pub fn latex_to_hulk(input: &str) -> String {
    let src = strip_math_delimiters(input);
    if src.chars().count() > MAX_EQUATION_SOURCE {
        return normalize_eq_edit(&src);
    }
    convert_latex_fragment(&quote_reserved_keywords(&src), 0)
}
