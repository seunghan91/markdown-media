// Test cases ported/adapted from kkdoc (MIT):
//   tests/hwpx-equation.test.ts               (hmlToLatex / hulk_to_latex)
//   tests/hwpx-equation-generation.test.ts    (latexLikeToEqEdit / latex_to_hulk)
//
// hmlToLatex's raw output carries irregular inter-token spacing (it's a
// space-joined token stream, not a pretty-printer) — the upstream tests
// normalize with `.replace(/\s+/g, "")` or collapse-to-single-space before
// comparing. Same approach here via `nospace`/`norm`.

use super::{hulk_to_latex, latex_to_hulk};

fn nospace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ─── hulk_to_latex — single token convert map ─────────────────────────────

#[test]
fn to_latex_left_lfloor_right_rfloor() {
    let out = norm(&hulk_to_latex("LEFT \u{230a} a+b RIGHT \u{230b}"));
    assert_eq!(out, "\\left \\lfloor a+b \\right \\rfloor");
}

#[test]
fn to_latex_pm_neq_leq() {
    let out = norm(&hulk_to_latex("a \u{00b1} b != c LEQ d"));
    assert_eq!(out, "a \\pm b \\neq c \\leq d");
}

#[test]
fn to_latex_greek_letters() {
    let out = norm(&hulk_to_latex("alpha + PHI"));
    assert_eq!(out, "\\alpha + \\Phi");
}

#[test]
fn to_latex_left_right_brace() {
    let out = norm(&hulk_to_latex("LEFT { x RIGHT }"));
    assert_eq!(out, "\\left \\{ x \\right \\}");
}

// ─── hulk_to_latex — frac (over) ───────────────────────────────────────────

#[test]
fn to_latex_frac_simple() {
    assert_eq!(nospace(&hulk_to_latex("{a} over {b}")), "\\frac{a}{b}");
}

#[test]
fn to_latex_frac_nested() {
    let out = nospace(&hulk_to_latex("{ { 1 } over { x } } over { y }"));
    assert_eq!(out, "\\frac{\\frac{1}{x}}{y}");
}

// ─── hulk_to_latex — root of ────────────────────────────────────────────────

#[test]
fn to_latex_root_of() {
    assert_eq!(nospace(&hulk_to_latex("root {3} of {x+1}")), "\\sqrt[3]{x+1}");
}

// ─── hulk_to_latex — reserved-word literal misparse guards ─────────────────

#[test]
fn to_latex_quoted_over_literal_is_not_a_frac_trigger() {
    let out = nospace(&hulk_to_latex("{a} over {b} + x _ {\"over\"}"));
    assert_eq!(out, "\\frac{a}{b}+x_{\\text{over}}");
}

#[test]
fn to_latex_multiword_quote_containing_over_untouched() {
    let out = hulk_to_latex("{ \"sum over items\" } + {a} over {b}");
    assert!(out.contains("\"sum over items\""), "{out}");
    assert!(nospace(&out).contains("\\frac{a}{b}"), "{out}");
}

#[test]
fn to_latex_leading_literal_of_substring_not_mistaken_for_root_of() {
    let out = nospace(&hulk_to_latex("\"profit\" + root {3} of {x}"));
    assert_eq!(out, "\\text{profit}+\\sqrt[3]{x}");
}

#[test]
fn to_latex_word_internal_substrings_are_not_operators() {
    let out = norm(&hulk_to_latex("groot + cover"));
    assert_eq!(out, "groot + cover");
}

// ─── hulk_to_latex — vec / bar ──────────────────────────────────────────────

#[test]
fn to_latex_vec_accent() {
    assert_eq!(nospace(&hulk_to_latex("{ vec {AB} }")), "\\overrightarrow{AB}");
}

#[test]
fn to_latex_hat_accent() {
    assert_eq!(nospace(&hulk_to_latex("{ hat {x} }")), "\\widehat{x}");
}

// ─── hulk_to_latex — matrix / cases ─────────────────────────────────────────

#[test]
fn to_latex_matrix() {
    let out = hulk_to_latex("{ matrix {a & b # c & d} }");
    assert!(out.contains("\\begin{matrix}"));
    assert!(out.contains("\\end{matrix}"));
    assert!(out.contains("\\\\"));
}

#[test]
fn to_latex_cases() {
    let out = hulk_to_latex("{ cases { 1 & x>0 # 0 & x<=0 } }");
    assert!(out.contains("\\begin{cases}"));
    assert!(out.contains("\\end{cases}"));
}

// ─── hulk_to_latex — brace (overbrace/underbrace) ──────────────────────────

#[test]
fn to_latex_overbrace() {
    assert_eq!(nospace(&hulk_to_latex("OVERBRACE {x+y} {n}")), "\\overbrace{x+y}^{n}");
}

// ─── hulk_to_latex — misc ───────────────────────────────────────────────────

#[test]
fn to_latex_empty_and_blank() {
    assert_eq!(hulk_to_latex(""), "");
    assert_eq!(hulk_to_latex("   ").trim(), "");
}

#[test]
fn to_latex_backtick_becomes_space() {
    let out = norm(&hulk_to_latex("a`+`b"));
    assert_eq!(out, "a + b");
}

// ─── latex_to_hulk — basic structures ──────────────────────────────────────

#[test]
fn to_hulk_frac_sqrt_nthroot() {
    assert_eq!(latex_to_hulk("\\frac{a}{b}"), "{a} over {b}");
    assert_eq!(latex_to_hulk("\\sqrt{x}"), "sqrt{x}");
    assert_eq!(latex_to_hulk("\\sqrt[n]{x}"), "root {n} of {x}");
}

#[test]
fn to_hulk_greek_ops_arrows() {
    assert_eq!(latex_to_hulk("\\alpha + \\beta = \\gamma"), "alpha + beta = gamma");
    assert_eq!(latex_to_hulk("A \\rightarrow B"), "A -> B");
    assert_eq!(latex_to_hulk("x \\le y \\ne z \\ge w"), "x LEQ y != z GEQ w");
}

#[test]
fn to_hulk_matrix_env() {
    let out = latex_to_hulk("\\begin{matrix} a & b \\\\ c & d \\end{matrix}");
    assert_eq!(out, "{matrix{a & b # c & d}}");
}

#[test]
fn to_hulk_pmatrix_bmatrix_native_tokens() {
    assert_eq!(
        latex_to_hulk("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}"),
        "{pmatrix{a & b # c & d}}"
    );
    assert_eq!(latex_to_hulk("\\begin{bmatrix} 1 \\\\ 2 \\end{bmatrix}"), "{bmatrix{1 # 2}}");
}

// ─── round trip: every written token must read back to the same command ───

#[test]
fn roundtrip_pm_cdot_ast_leftarrow() {
    let script = latex_to_hulk("a \\pm b \\cdot c \\ast d \\leftarrow e");
    assert_eq!(script, "a +- b cdot c AST d larrow e");
    assert_eq!(nospace(&hulk_to_latex(&script)), "a\\pmb\\cdotc\\astd\\leftarrowe");
}

#[test]
fn roundtrip_left_right_parens() {
    let script = latex_to_hulk("\\left( x \\right)");
    assert_eq!(script, "LEFT ( x RIGHT )");
    assert_eq!(nospace(&hulk_to_latex(&script)), "\\left(x\\right)");
}

#[test]
fn roundtrip_left_right_braces_no_leftover_backslash() {
    let script = latex_to_hulk("\\left\\{ x + y \\right\\}");
    assert_eq!(script, "LEFT { x + y RIGHT }");
    assert_eq!(nospace(&hulk_to_latex(&script)), "\\left\\{x+y\\right\\}");
}

#[test]
fn roundtrip_pmatrix_preserves_environment() {
    let script = latex_to_hulk("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}");
    assert_eq!(nospace(&hulk_to_latex(&script)), "\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}");
}

#[test]
fn roundtrip_cases_vmatrix_native_tokens() {
    let rt = |s: &str| nospace(&hulk_to_latex(&latex_to_hulk(s)));
    assert_eq!(rt("\\begin{cases} a & b \\\\ c & d \\end{cases}"), "\\begin{cases}a&b\\\\c&d\\end{cases}");
    assert_eq!(rt("\\begin{vmatrix} a \\\\ b \\end{vmatrix}"), "\\begin{vmatrix}a\\\\b\\end{vmatrix}");
}

#[test]
fn roundtrip_bmatrix_align_content_preserved() {
    let rt = |s: &str| nospace(&hulk_to_latex(&latex_to_hulk(s)));
    assert_eq!(
        rt("\\begin{Bmatrix} a \\\\ b \\end{Bmatrix}"),
        "\\left\\{\\begin{matrix}a\\\\b\\end{matrix}\\right\\}"
    );
    assert!(rt("\\begin{align} a &= b \\\\ c &= d \\end{align}").contains("eqalign"));
}

#[test]
fn roundtrip_all_command_map_entries_are_fixed_points() {
    // Ported from "COMMAND_MAP 전 항목이 고정점": writing `cmd` then reading the
    // token back doesn't have to reproduce the same command name (aliases
    // like `to`/`rightarrow` both write "->"), but re-writing whatever name
    // it *does* read back to must reproduce the exact same token.
    for cmd in ["alpha", "beta", "leq", "geq", "pm", "mp", "times", "cdot", "in", "notin",
                "subset", "cup", "cap", "int", "sum", "prod", "lim", "to", "rightarrow",
                "leftrightarrow", "cdots", "ldots", "div", "approx", "therefore", "because",
                "oplus", "uparrow", "propto", "cong", "equiv", "sim", "angle", "mapsto",
                "ll", "gg", "dagger", "models", "coprod"] {
        let script = latex_to_hulk(&format!("\\{cmd}"));
        let latex_back = norm(&hulk_to_latex(&script));
        assert!(
            latex_back.starts_with('\\') && latex_back[1..].chars().all(|c| c.is_ascii_alphabetic()),
            "\\{cmd} -> {script:?} -> {latex_back:?} — not read back as a single command"
        );
        let script2 = latex_to_hulk(&latex_back);
        assert_eq!(script2, script, "\\{cmd} -> {script:?} -> {latex_back:?} -> {script2:?} (fixed point broken)");
    }
}

#[test]
fn roundtrip_accent_commands_are_fixed_points() {
    for cmd in ["bar", "overline", "vec", "overrightarrow", "hat", "widehat",
                "tilde", "widetilde", "dot", "ddot", "underline"] {
        let script = latex_to_hulk(&format!("\\{cmd}{{x}}"));
        let latex_back = norm(&hulk_to_latex(&script));
        let m = latex_back.strip_prefix('\\').and_then(|s| s.split('{').next()).unwrap_or("").trim();
        assert!(!m.is_empty(), "\\{cmd} -> {script:?} -> {latex_back:?} — not read back as an accent command");
        let script2 = latex_to_hulk(&format!("\\{m}{{x}}"));
        assert_eq!(script2, script, "\\{cmd} -> {script:?} -> \\{m} (fixed point broken)");
    }
}

// ─── reserved-word subscript protection ────────────────────────────────────

#[test]
fn to_hulk_reserved_subscript_quoted() {
    assert_eq!(latex_to_hulk("T_{int}"), "T _{\"int\"}");
    assert_eq!(latex_to_hulk("x_{rel}"), "x _{rel}");
}

#[test]
fn to_hulk_text_literal_roundtrips_through_text_command() {
    assert_eq!(latex_to_hulk("\\text{int}"), "\"int\"");
    assert_eq!(nospace(&hulk_to_latex("T _{\"int\"}")), "T_{\\text{int}}");
}

// ─── malformed-input guards ─────────────────────────────────────────────────

#[test]
fn to_hulk_brace_bomb_does_not_overflow() {
    // Must not stack-overflow / panic on pathological nesting — that's the
    // whole assertion (MAX_GROUP_DEPTH guard).
    let _out = latex_to_hulk(&"{".repeat(5000));
}

#[test]
fn to_hulk_long_source_passes_through_unconverted() {
    // MAX_EQUATION_SOURCE guard — over-length input is returned
    // (whitespace-normalized) without attempting conversion.
    let long = "x + ".repeat(4000);
    assert!(!latex_to_hulk(&long).is_empty());
}
