//! MDM Core Engine - High-performance media rendering for Markdown Media
//!
//! This crate provides core functionality for:
//! - HWP document parsing (HWP 5.0 and HWPX)
//! - SVG generation from media descriptions
//! - Image optimization and caching
//! - Placeholder generation

pub mod hwp;
pub mod hwpx;
pub mod pdf;
pub mod renderer;
pub mod optimizer;
pub mod cache;

pub use hwp::HwpParser;
pub use hwpx::HwpxParser;
pub use renderer::Renderer;
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
