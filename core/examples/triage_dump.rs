//! Dump triage results for a PDF — use to calibrate thresholds.
//!
//! cargo run --example triage_dump --features pdf -- <path-to-pdf>

use mdm_core::pdf::PdfParser;

fn main() {
    let path = std::env::args().nth(1).expect("usage: triage_dump <pdf>");
    let parser = PdfParser::open(&path).expect("open failed");
    println!(
        "{:<4} {:<12} {:<5} {:<7} {:<7} {:<5} {:<7} {:<7} {:<5} {:<5}",
        "pg", "category", "conf", "text%", "image%", "#img",
        "fontR", "underL", "invis", "cjk"
    );
    for t in parser.triage() {
        let fmt_opt_f = |v: Option<f32>| v.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "-".into());
        let fmt_opt_b = |v: Option<bool>| match v {
            Some(true) => "yes".to_string(),
            Some(false) => "no".to_string(),
            None => "-".to_string(),
        };
        println!(
            "{:<4} {:<12?} {:<5.2} {:<7.3} {:<7.3} {:<5} {:<7} {:<7} {:<5} {:<5}",
            t.page,
            t.category,
            t.confidence,
            t.text_coverage,
            t.image_coverage,
            t.image_count,
            fmt_opt_f(t.font_reliability),
            fmt_opt_f(t.ocr_underlay_ratio),
            fmt_opt_b(t.has_invisible_text),
            fmt_opt_b(t.contains_cjk),
        );
    }
}
