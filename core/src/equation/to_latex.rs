// Ported from kkdoc (MIT): src/hwpx/equation.ts (`hmlToLatex`)
//
// HWPX/HWP equation script (HULK, Hancom's mini equation language) -> LaTeX.
// Upstream credits this as a direct port of hml-equation-parser (Python,
// Apache 2.0) — see reference/kkdoc/THIRD_PARTY/hml-equation-parser.txt.
//
// Entry point: `hulk_to_latex(script)`.
//
// Example: `x = { -b +- SQRT { b^2 -4ac } } over {2a}`
//       -> `x = \frac { -b +- \sqrt { b^2 -4ac } }{2a}`
//
// Same 5-pass rewrite as upstream (frac -> rootOf -> matrix -> bar -> brace),
// operating on `Vec<char>` throughout (rather than `&str`/byte slices) so
// index bookkeeping stays valid even when a literal `"..."`/`\text{...}` span
// carries multibyte Korean text.

use super::find_substring;
use super::tables::{BAR_CONVERT_MAP, BRACE_CONVERT_MAP, CONVERT_MAP, MATRIX_CONVERT_MAP, MIDDLE_CONVERT_MAP, MatrixMapping};

/// Find a matching `{...}` pair at/after `start_idx` (direction=1) or before
/// (direction=0). Returns `[start, end)` such that `chars[start..end]` is the
/// full bracketed span including the `{` and `}`. `None` when unmatched.
fn find_brackets(chars: &[char], start_idx: usize, direction: u8) -> Option<(usize, usize)> {
    if direction == 1 {
        let start_cur = start_idx + chars[start_idx..].iter().position(|&c| c == '{')?;
        let mut bracket_count: i32 = 1;
        for i in start_cur + 1..chars.len() {
            match chars[i] {
                '{' => bracket_count += 1,
                '}' => bracket_count -= 1,
                _ => {}
            }
            if bracket_count == 0 {
                return Some((start_cur, i + 1));
            }
        }
        return None;
    }

    // direction=0: reverse the string (and swap braces) then reuse dir=1 search.
    let mut reversed: Vec<char> = chars.iter().rev().copied().collect();
    for c in reversed.iter_mut() {
        if *c == '{' {
            *c = '}';
        } else if *c == '}' {
            *c = '{';
        }
    }
    let new_start_idx = reversed.len().checked_sub(start_idx + 1)?;
    let (s, e) = find_brackets(&reversed, new_start_idx, 1)?;
    Some((reversed.len() - e, reversed.len() - s))
}

/// Find the nearest `{...}` group that actually encloses `start_idx`.
/// A previous closed group such as `_ { x } HULKBAR { y }` must not be
/// treated as the outer wrapper for `HULKBAR`.
fn find_enclosing_brackets(chars: &[char], start_idx: usize) -> Option<(usize, usize)> {
    let mut depth: i32 = 0;
    let mut idx = start_idx as isize - 1;
    while idx >= 0 {
        let i = idx as usize;
        match chars[i] {
            '}' => depth += 1,
            '{' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    return match find_brackets(chars, i, 1) {
                        Some((start, end)) if start == i && end > start_idx => Some((start, end)),
                        _ => None,
                    };
                }
            }
            _ => {}
        }
        idx -= 1;
    }
    None
}

/// `"..."` literals and `\text{...}` spans get masked with a same-length
/// filler so reserved-word search (over/root/of) doesn't mistake a
/// substring inside a literal for an operator. Index alignment is preserved.
fn mask_literal_spans(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut out = chars.to_vec();

    let mut i = 0;
    while i < n {
        if chars[i] == '"' {
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == '"') {
                let end = i + 1 + rel;
                for j in i..=end {
                    out[j] = '\u{ffff}';
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    let text_prefix: [char; 6] = ['\\', 't', 'e', 'x', 't', '{'];
    let mut i = 0;
    while i < n {
        if i + text_prefix.len() <= n && chars[i..i + text_prefix.len()] == text_prefix {
            let body_start = i + text_prefix.len();
            if let Some(rel) = chars[body_start..].iter().position(|&c| c == '}') {
                let end = body_start + rel;
                for j in i..=end {
                    out[j] = '\u{ffff}';
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    out
}

/// Space-bounded standalone token search only (literal spans masked). `None`
/// when not found.
fn find_keyword_token(chars: &[char], word: &str, from: usize) -> Option<usize> {
    let masked = mask_literal_spans(chars);
    let word_chars: Vec<char> = word.chars().collect();
    let wlen = word_chars.len();
    if wlen == 0 || masked.len() < wlen {
        return None;
    }
    let mut i = from;
    while i + wlen <= masked.len() {
        if masked[i..i + wlen] == word_chars[..] {
            let ok_l = i == 0 || masked[i - 1].is_whitespace();
            let ok_r = i + wlen == masked.len() || masked[i + wlen].is_whitespace();
            if ok_l && ok_r {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// `{1} over {2}` -> `\frac{1}{2}`
fn replace_frac(mut chars: Vec<char>) -> Vec<char> {
    let hml_frac = "over";
    loop {
        let Some(cursor) = find_keyword_token(&chars, hml_frac, 0) else { break };

        // The numerator is the token immediately preceding `over` (skipping
        // whitespace) — grabbing the nearest `}` group to the left avoids
        // silently deleting unrelated content between an earlier group and
        // `over` (e.g. `sqrt {x} + 1 over 2`).
        let mut end = cursor;
        while end > 0 && chars[end - 1].is_whitespace() {
            end -= 1;
        }

        let num_start: usize;
        let num_end: usize;
        let wrapped: Vec<char>;
        if end > 0 && chars[end - 1] == '}' {
            match find_brackets(&chars, end - 1, 0) {
                Some((s, e)) => {
                    num_start = s;
                    num_end = e;
                    wrapped = chars[s..e].to_vec();
                }
                None => return chars,
            }
        } else {
            num_end = end;
            let mut start = end;
            while start > 0 && !chars[start - 1].is_whitespace() {
                start -= 1;
            }
            if start == num_end {
                // empty numerator
                return chars;
            }
            num_start = start;
            let mut w = vec!['{'];
            w.extend_from_slice(&chars[start..num_end]);
            w.push('}');
            wrapped = w;
        }

        let before_frac: Vec<char> = chars[..num_start].to_vec();
        let after_frac: Vec<char> = chars[cursor + hml_frac.chars().count()..].to_vec();

        let mut next = before_frac;
        next.extend("\\frac".chars());
        next.extend(wrapped);
        next.extend(after_frac);
        chars = next;
    }
    chars
}

/// `root {1} of {2}` -> `\sqrt[1]{2}`
fn replace_root_of(mut chars: Vec<char>) -> Vec<char> {
    loop {
        let Some(root_cursor) = find_keyword_token(&chars, "root", 0) else { break };
        let Some(elem1) = find_brackets(&chars, root_cursor, 1) else { return chars };
        // `of` is only valid after root's degree group — a global first match
        // would mistake a literal/preceding text for the keyword.
        let Some(of_cursor) = find_keyword_token(&chars, "of", elem1.1) else { return chars };
        let Some(elem2) = find_brackets(&chars, of_cursor, 1) else { return chars };

        let e1: Vec<char> = chars[elem1.0 + 1..elem1.1 - 1].to_vec();
        let e2: Vec<char> = chars[elem2.0 + 1..elem2.1 - 1].to_vec();

        let mut next: Vec<char> = chars[..root_cursor].to_vec();
        next.extend("\\sqrt[".chars());
        next.extend(e1);
        next.extend("]{".chars());
        next.extend(e2);
        next.push('}');
        next.extend(chars.get(elem2.1 + 1..).unwrap_or(&[]).iter().copied());
        chars = next;
    }
    chars
}

fn replace_elements(bracket_chars: &[char]) -> Vec<char> {
    // strip outer `{` `}`
    let inner: String = bracket_chars[1..bracket_chars.len() - 1].iter().collect();
    let inner = inner.replace('#', " \\\\ ").replace("&amp;", "&");
    inner.chars().collect()
}

fn replace_matrix(mut input: Vec<char>, mat_str: &str, mat_elem: &MatrixMapping) -> Vec<char> {
    loop {
        let Some(cursor) = find_substring(&input, mat_str, 0) else { break };
        let Some((e_start, e_end)) = find_brackets(&input, cursor, 1) else { return input };
        let elem = replace_elements(&input[e_start..e_end]);
        let outer = if mat_elem.remove_outer_brackets { find_enclosing_brackets(&input, cursor) } else { None };

        let (before, after): (Vec<char>, Vec<char>) = match outer {
            Some((b_start, b_end)) if b_end >= e_end => (input[..b_start].to_vec(), input[b_end..].to_vec()),
            _ => (input[..cursor].to_vec(), input[e_end..].to_vec()),
        };

        let mut next = before;
        next.extend(mat_elem.begin.chars());
        next.extend(elem);
        next.extend(mat_elem.end.chars());
        next.extend(after);
        input = next;
    }
    input
}

/// matrix/pmatrix/bmatrix/dmatrix/cases/eqalign expansion.
fn replace_all_matrix(mut eq: Vec<char>) -> Vec<char> {
    for (mat_key, mat_elem) in MATRIX_CONVERT_MAP {
        eq = replace_matrix(eq, mat_key, mat_elem);
    }
    eq
}

fn replace_bar(mut input: Vec<char>, bar_str: &str, bar_elem: &str) -> Vec<char> {
    loop {
        let Some(cursor) = find_substring(&input, bar_str, 0) else { break };
        let Some((e_start, e_end)) = find_brackets(&input, cursor, 1) else { return input };
        let elem: Vec<char> = input[e_start..e_end].to_vec();
        let outer = find_enclosing_brackets(&input, cursor);
        let (replace_start, replace_end) = match outer {
            Some((os, oe)) if oe >= e_end => (os, oe),
            _ => (cursor, e_end),
        };

        let mut next: Vec<char> = input[..replace_start].to_vec();
        next.extend(bar_elem.chars());
        next.extend(elem);
        next.extend(input[replace_end..].iter().copied());
        input = next;
    }
    input
}

/// vec/hat/bar/dot/ddot/tilde/... (HULK-prefixed) -> LaTeX accent.
fn replace_all_bar(mut eq: Vec<char>) -> Vec<char> {
    for (bar_key, bar_elem) in BAR_CONVERT_MAP {
        eq = replace_bar(eq, bar_key, bar_elem);
    }
    eq
}

fn replace_brace(mut input: Vec<char>, brace_str: &str, brace_elem: &str) -> Vec<char> {
    loop {
        let Some(cursor) = find_substring(&input, brace_str, 0) else { break };
        let Some((e_start1, e_end1)) = find_brackets(&input, cursor, 1) else { return input };
        let Some((e_start2, e_end2)) = find_brackets(&input, e_end1, 1) else { return input };
        let elem1: Vec<char> = input[e_start1..e_end1].to_vec();
        let elem2: Vec<char> = input[e_start2..e_end2].to_vec();

        let mut next: Vec<char> = input[..cursor].to_vec();
        next.extend(brace_elem.chars());
        next.extend(elem1);
        next.push('^');
        next.extend(elem2);
        next.extend(input[e_end2..].iter().copied());
        input = next;
    }
    input
}

/// overbrace/underbrace: `BRACE {body} {label}` -> `\overbrace{body}^{label}`
fn replace_all_brace(mut eq: Vec<char>) -> Vec<char> {
    for (brace_key, brace_elem) in BRACE_CONVERT_MAP {
        eq = replace_brace(eq, brace_key, brace_elem);
    }
    eq
}

/// After single-token pass, fix `\left {` -> `\left \{` and `\right }` -> `\right \}`.
fn replace_bracket(tokens: &mut [String]) {
    for i in 0..tokens.len() {
        if tokens[i] == "{" && i > 0 && tokens[i - 1] == "\\left" {
            tokens[i] = "\\{".to_string();
        }
        if tokens[i] == "}" && i > 0 && tokens[i - 1] == "\\right" {
            tokens[i] = "\\}".to_string();
        }
    }
}

/// Convert an HWPX/HWP equation script (HULK, Hancom's mini equation
/// language) to LaTeX. Returns the converted LaTeX body (without `$` delimiters).
pub fn hulk_to_latex(hml_eq_str: &str) -> String {
    if hml_eq_str.is_empty() {
        return String::new();
    }

    let mut s = hml_eq_str.replace('`', " ");
    s = s.replace('{', " { ").replace('}', " } ").replace('&', " & ");

    let mut tokens: Vec<String> = s.split(' ').map(|t| t.to_string()).collect();
    for t in tokens.iter_mut() {
        if let Some(v) = CONVERT_MAP.get(t.as_str()) {
            *t = (*v).to_string();
        } else if let Some(v) = MIDDLE_CONVERT_MAP.get(t.as_str()) {
            *t = (*v).to_string();
        } else if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
            // EqEdit literal quotes ("int" etc — blocks command interpretation)
            // -> restored to \text{...}. Forms a fixed point with the
            // generator's (equation-generate.ts) \text -> "..." output.
            let inner = &t[1..t.len() - 1];
            if !inner.is_empty() {
                *t = format!("\\text{{{}}}", inner);
            }
        }
    }
    tokens.retain(|tok| !tok.is_empty());
    replace_bracket(&mut tokens);

    let out = tokens.join(" ");
    let mut chars: Vec<char> = out.chars().collect();
    chars = replace_frac(chars);
    chars = replace_root_of(chars);
    chars = replace_all_matrix(chars);
    chars = replace_all_bar(chars);
    chars = replace_all_brace(chars);

    chars.into_iter().collect()
}
