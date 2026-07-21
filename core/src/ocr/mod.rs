// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/engine.ts, models.ts, pdf-ocr.ts
//
//! Built-in text OCR engine — PP-OCRv5 korean (det DBNet + rec SVTR/CTC) ONNX inference.
//!
//! Pipeline: RGBA page → det (line detection) → line crop → rec (CTC decode) → [`OcrLine`].
//! Coordinates are in input-pixel space (top-left origin, y down); callers map to
//! PDF coordinates (see reference `pdf-ocr.ts::ocrItemsToBlocks`).
//!
//! Pre/post-processing follow the official `inference.yml` spec verbatim:
//!  - det: BGR, long side 960 (multiple of 32), mean/std [0.485,0.456,0.406]/[0.229,0.224,0.225],
//!    DBPostProcess thresh 0.3 / box_thresh 0.6 / unclip_ratio 1.5
//!  - rec: BGR, height 48 aspect-preserving resize + right zero-pad, (x/255-0.5)/0.5,
//!    CTC decode (blank=0, 1..N=dict, N+1=space), text_score 0.5
//!
//! DB post-processing's contour+minAreaRect is approximated by axis-aligned
//! connected-component bboxes (scanned government docs are dominated by horizontal
//! text; rotated text is out of v1 scope).
//!
//! ## Feature gating
//! The pure numeric core (dictionary parsing, normalization math, connected-component
//! box extraction, CTC decode) is always compiled and unit-tested. The ONNX Runtime
//! engine ([`engine`]) is behind the optional `ocr` feature (pulls the `ort` crate).
//! Build with `--features ocr` and download models via `scripts/download-ocr-models.sh`.

pub mod detect;
pub mod models;
pub mod preprocess;
pub mod recognize;

#[cfg(feature = "ocr")]
pub mod engine;

#[cfg(feature = "ocr")]
pub use engine::OcrEngine;

/// One recognized text line. Coordinates are input-image pixels (top-left origin).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OcrLine {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Mean CTC confidence, 0..1.
    pub confidence: f32,
}

/// Recognition threshold — lines below this confidence are dropped (`text_score`).
pub const TEXT_SCORE: f32 = 0.5;

#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("OCR feature not enabled (rebuild with --features ocr)")]
    FeatureDisabled,
    #[error("OCR models not found in {0} — run scripts/download-ocr-models.sh")]
    ModelsMissing(String),
    #[error("OCR dictionary parse failed (delete model cache and re-download)")]
    DictParse,
    #[error("image decode failed: {0}")]
    ImageDecode(String),
    #[error("onnx runtime error: {0}")]
    Runtime(String),
}

pub type Result<T> = std::result::Result<T, OcrError>;

/// True when OCR can actually run: the `ocr` feature is compiled in **and** the
/// model files are present in the cache. This is the signal the PDF pipeline
/// should gate on before routing Scanned/Mixed pages (see `pdf::triage`).
pub fn ocr_available() -> bool {
    #[cfg(feature = "ocr")]
    {
        models::models_present()
    }
    #[cfg(not(feature = "ocr"))]
    {
        false
    }
}

/// Recognize text lines from an encoded image (PNG/JPEG/…) byte buffer.
///
/// Without the `ocr` feature this returns [`OcrError::FeatureDisabled`] so callers
/// can degrade gracefully. With the feature it decodes to RGBA and runs the engine.
pub fn ocr_image(bytes: &[u8]) -> Result<Vec<OcrLine>> {
    #[cfg(feature = "ocr")]
    {
        engine::ocr_image_impl(bytes)
    }
    #[cfg(not(feature = "ocr"))]
    {
        let _ = bytes;
        Err(OcrError::FeatureDisabled)
    }
}

/// Recognize text lines directly from decoded RGBA pixels (top-left origin,
/// row-major, `len == w * h * 4`). Used by the PDF OCR pipeline
/// (`pdf::pdf_ocr`), which rasterizes pages itself and has no encoded image
/// bytes to hand to [`ocr_image`].
///
/// Without the `ocr` feature this returns [`OcrError::FeatureDisabled`].
pub fn ocr_rgba(rgba: &[u8], w: u32, h: u32) -> Result<Vec<OcrLine>> {
    #[cfg(feature = "ocr")]
    {
        engine::ocr_rgba_impl(rgba, w, h)
    }
    #[cfg(not(feature = "ocr"))]
    {
        let _ = (rgba, w, h);
        Err(OcrError::FeatureDisabled)
    }
}
