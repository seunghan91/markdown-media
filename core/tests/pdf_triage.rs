//! Integration tests: PDF triage against the repo's existing test corpus.
//!
//! These don't assert a specific category (real-world docs vary wildly);
//! they assert that:
//!   1. Triage returns a result for every page
//!   2. Coverage signals are in [0, 1]
//!   3. The signature of public API stays stable

use mdm_core::pdf::{PdfParser, PdfCategory};
use std::path::PathBuf;

fn corpus_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.push("tests");
    p
}

fn load(rel: &str) -> PdfParser {
    let mut p = corpus_root();
    p.push(rel);
    PdfParser::open(&p).unwrap_or_else(|e| panic!("open {}: {}", rel, e))
}

#[test]
fn triage_returns_one_result_per_page() {
    let parser = load("pdf_benchmark/test_comprehensive.pdf");
    let results = parser.triage();
    assert!(!results.is_empty(), "triage must return at least one page");
    for t in &results {
        assert!(t.page >= 1, "page numbers are 1-indexed");
        assert!(
            (0.0..=1.0).contains(&t.text_coverage),
            "text_coverage {} out of [0,1] on page {}", t.text_coverage, t.page
        );
        assert!(
            (0.0..=1.0).contains(&t.image_coverage),
            "image_coverage {} out of [0,1] on page {}", t.image_coverage, t.page
        );
        assert!(
            (0.0..=1.0).contains(&t.confidence),
            "confidence {} out of [0,1] on page {}", t.confidence, t.page
        );
    }
}

#[test]
fn triage_classifies_digital_text_pdfs() {
    // Synthetic benchmark PDFs are digital-born → should classify as
    // TextNative or Unknown (if coverage heuristic underestimates).
    // Must NOT classify as Scanned (we know there's no image-only content).
    let parser = load("pdf_benchmark/test_headings.pdf");
    let results = parser.triage();
    for t in &results {
        assert_ne!(
            t.category,
            PdfCategory::Scanned,
            "digital-born PDF page {} classified as Scanned (text_cov={}, image_cov={})",
            t.page, t.text_coverage, t.image_coverage
        );
    }
}

#[test]
fn triage_mixed_page_yields_ocr_regions() {
    // Iterate all corpus PDFs; if we find a Mixed page, its ocr_regions
    // should be non-empty. Lack of Mixed pages is fine (not all docs have them).
    let patterns = [
        "pdf_benchmark/test_comprehensive.pdf",
        "pdf_benchmark/test_twocolumn.pdf",
        "pdf_benchmark/test_headers_footers.pdf",
        "pdf_benchmark/test_headings.pdf",
    ];
    for path in patterns {
        let parser = load(path);
        for t in parser.triage() {
            if t.category == PdfCategory::Mixed {
                assert!(
                    !t.ocr_regions.is_empty(),
                    "Mixed page {} in {} has no ocr_regions", t.page, path
                );
            }
        }
    }
}

#[test]
fn triage_is_deterministic() {
    let parser = load("pdf_benchmark/test_comprehensive.pdf");
    let r1 = parser.triage();
    let r2 = parser.triage();
    assert_eq!(r1.len(), r2.len());
    for (a, b) in r1.iter().zip(r2.iter()) {
        assert_eq!(a.category, b.category, "page {} category diverged", a.page);
        assert!((a.text_coverage - b.text_coverage).abs() < 1e-6);
        assert!((a.image_coverage - b.image_coverage).abs() < 1e-6);
    }
}
