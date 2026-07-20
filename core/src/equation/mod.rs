//! HULK (Hancom's HWP/HWPX equation mini-language) <-> LaTeX conversion.
//!
//! Ported from kkdoc (MIT):
//!   - `src/hwpx/equation.ts`          (HULK -> LaTeX, `hmlToLatex`)
//!   - `src/hwpx/equation-generate.ts` (LaTeX -> HULK, `latexLikeToEqEdit`)
//!
//! kkdoc's `hmlToLatex` is itself a direct port of hml-equation-parser
//! (Python, Apache 2.0) — see
//! `reference/kkdoc/THIRD_PARTY/hml-equation-parser.txt` for the original
//! license. HWPX's `<hp:script>` and HWP5's `HWPTAG_EQEDIT` record both carry
//! the same near-LaTeX HULK mini-language, so this module serves both parsers.

mod tables;
mod to_hulk;
mod to_latex;

pub use to_hulk::latex_to_hulk;
pub use to_latex::hulk_to_latex;

/// `haystack[from..]`'s first occurrence of `needle`, char-indexed. Rust
/// equivalent of JS `string.indexOf(needle, from)`, operating on `Vec<char>`
/// throughout this module so a multibyte (Korean) span inside `\text{...}`
/// or a quoted EqEdit literal can't desync byte/char index bookkeeping.
pub(crate) fn find_substring(haystack: &[char], needle: &str, from: usize) -> Option<usize> {
    let needle_chars: Vec<char> = needle.chars().collect();
    let nlen = needle_chars.len();
    if nlen == 0 {
        return Some(from.min(haystack.len()));
    }
    if from > haystack.len() || haystack.len() - from < nlen {
        return None;
    }
    for i in from..=haystack.len() - nlen {
        if haystack[i..i + nlen] == needle_chars[..] {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests;
