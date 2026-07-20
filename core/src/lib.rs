//! MDM Core Engine - High-performance media rendering for Markdown Media
//!
//! This crate provides core functionality for:
//! - HWP document parsing (HWP 5.0 and HWPX)
//! - SVG generation from media descriptions

// Allow dead code and unused imports during active development.
// These will be cleaned up before v1.0 release.
#![allow(dead_code, unused_imports, unused_variables, unreachable_patterns, unused_assignments)]
//! - Image optimization and caching
//! - Placeholder generation

pub mod hwp;
pub mod hwp3;
pub mod hwpx;
pub mod hwpx_gen;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod docx;
pub mod equation;
pub mod xlsx;
#[cfg(feature = "xls")]
pub mod xls;
#[cfg(feature = "rtf")]
pub mod rtf;
#[cfg(feature = "epub")]
pub mod epub;
pub mod pptx;
#[cfg(feature = "url-fetch")]
pub mod url_fetch;
pub mod doc97;
pub mod heic;
#[cfg(feature = "docx-out")]
pub mod gen_docx;
#[cfg(feature = "pdf-out")]
pub mod gen_pdf;
pub mod html;
pub mod csv_parser;
pub mod txt_parser;
pub mod plugin;
pub mod ir;
pub mod chunker;
pub mod ocr;
#[cfg(feature = "image-processing")]
pub mod renderer;
#[cfg(feature = "image-processing")]
pub mod optimizer;
pub mod cache;
pub mod legal;
pub mod form;
pub mod manifest;
pub mod utils;
pub mod pii;
pub mod lint;
#[cfg(feature = "watch")]
pub mod watch;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use hwp::HwpParser;
pub use hwpx::HwpxParser;
pub use docx::DocxParser;
pub use docx::DocxDocument;
#[cfg(feature = "image-processing")]
pub use renderer::Renderer;
#[cfg(feature = "image-processing")]
pub use optimizer::Optimizer;
pub use cache::Cache;

/// Core configuration for the MDM engine
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// Output format (svg, png, webp)
    pub format: String,
    /// Quality level (0-100)
    pub quality: u8,
    /// Enable caching
    pub cache_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            format: "svg".to_string(),
            quality: 85,
            cache_enabled: true,
        }
    }
}
