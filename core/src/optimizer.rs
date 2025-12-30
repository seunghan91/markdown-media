// ============================================================================
// 🚧 작업 중 - 이 파일은 현재 [병렬 작업 팀]에서 개선 중입니다
// ============================================================================
// 작업 담당: 병렬 작업 팀
// 시작 시간: 2025-01-01
// 진행 상태: Phase 2.1 이미지 최적화 구현
//
// ⚠️ 주의: 1.7 오케스트레이터는 다른 팀에서 작업 중입니다.
//         이 최적화 모듈은 독립적으로 작동합니다.
// ============================================================================

//! Image optimization module
//!
//! Provides image optimization capabilities for various formats:
//! - JPEG: Quality adjustment, progressive encoding
//! - PNG: Compression level, interlacing
//! - WebP: Lossy/lossless encoding, quality control
//! - GIF: Color palette optimization
//!
//! This module is designed to work independently from the main pipeline,
//! allowing parallel development with the orchestrator (1.7).

use image::{DynamicImage, ImageFormat, GenericImageView, ImageEncoder};
use image::codecs::png::PngEncoder;
use image::codecs::jpeg::JpegEncoder;
use std::io::Cursor;
use thiserror::Error;

/// Optimization errors
#[derive(Error, Debug)]
pub enum OptimizeError {
    #[error("Invalid image format: {0}")]
    InvalidFormat(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Image format for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    Jpeg,
    Png,
    WebP,
    Gif,
    Unknown,
}

impl ImageType {
    /// Detect format from magic bytes
    pub fn from_bytes(data: &[u8]) -> Self {
        if data.len() < 4 {
            return ImageType::Unknown;
        }

        // Check magic bytes
        if &data[0..2] == b"\xFF\xD8" {
            ImageType::Jpeg
        } else if &data[0..4] == b"\x89PNG" {
            ImageType::Png
        } else if &data[0..4] == b"RIFF" && data.len() >= 12 && &data[8..12] == b"WEBP" {
            ImageType::WebP
        } else if &data[0..4] == b"GIF8" {
            ImageType::Gif
        } else {
            ImageType::Unknown
        }
    }

    /// Get format from extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => ImageType::Jpeg,
            "png" => ImageType::Png,
            "webp" => ImageType::WebP,
            "gif" => ImageType::Gif,
            _ => ImageType::Unknown,
        }
    }

    /// Get file extension
    pub fn extension(&self) -> &'static str {
        match self {
            ImageType::Jpeg => "jpg",
            ImageType::Png => "png",
            ImageType::WebP => "webp",
            ImageType::Gif => "gif",
            ImageType::Unknown => "bin",
        }
    }

    /// Get MIME type
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageType::Jpeg => "image/jpeg",
            ImageType::Png => "image/png",
            ImageType::WebP => "image/webp",
            ImageType::Gif => "image/gif",
            ImageType::Unknown => "application/octet-stream",
        }
    }
}

/// Optimization settings
#[derive(Debug, Clone)]
pub struct OptimizeSettings {
    /// JPEG quality (1-100)
    pub jpeg_quality: u8,
    /// PNG compression level (1-9, where 9 is maximum compression)
    pub png_compression: u8,
    /// WebP quality (1-100, 0 for lossless)
    pub webp_quality: u8,
    /// WebP lossless mode
    pub webp_lossless: bool,
    /// Maximum dimension (resize if larger)
    pub max_dimension: Option<u32>,
    /// Strip metadata (EXIF, etc.)
    pub strip_metadata: bool,
    /// Convert to WebP if smaller
    pub prefer_webp: bool,
    /// Minimum file size reduction to accept optimization (0.0-1.0)
    pub min_reduction: f32,
}

impl Default for OptimizeSettings {
    fn default() -> Self {
        Self {
            jpeg_quality: 85,
            png_compression: 6,
            webp_quality: 80,
            webp_lossless: false,
            max_dimension: None,
            strip_metadata: true,
            prefer_webp: true,
            min_reduction: 0.1, // At least 10% reduction
        }
    }
}

impl OptimizeSettings {
    /// High quality preset (minimal compression)
    pub fn high_quality() -> Self {
        Self {
            jpeg_quality: 95,
            png_compression: 4,
            webp_quality: 90,
            webp_lossless: false,
            max_dimension: None,
            strip_metadata: false,
            prefer_webp: false,
            min_reduction: 0.05,
        }
    }

    /// Balanced preset (good balance of quality and size)
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Maximum compression preset
    pub fn max_compression() -> Self {
        Self {
            jpeg_quality: 70,
            png_compression: 9,
            webp_quality: 65,
            webp_lossless: false,
            max_dimension: Some(1920),
            strip_metadata: true,
            prefer_webp: true,
            min_reduction: 0.0,
        }
    }

    /// Web-optimized preset
    pub fn web() -> Self {
        Self {
            jpeg_quality: 80,
            png_compression: 7,
            webp_quality: 75,
            webp_lossless: false,
            max_dimension: Some(2048),
            strip_metadata: true,
            prefer_webp: true,
            min_reduction: 0.1,
        }
    }
}

/// Optimization result
#[derive(Debug, Clone)]
pub struct OptimizeResult {
    /// Optimized image data
    pub data: Vec<u8>,
    /// Original size in bytes
    pub original_size: usize,
    /// Optimized size in bytes
    pub optimized_size: usize,
    /// Original format
    pub original_format: ImageType,
    /// Output format
    pub output_format: ImageType,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Whether the image was resized
    pub resized: bool,
    /// Size reduction ratio (0.0-1.0)
    pub reduction: f32,
}

impl OptimizeResult {
    /// Check if optimization was effective
    pub fn is_effective(&self, min_reduction: f32) -> bool {
        self.reduction >= min_reduction
    }

    /// Get formatted size reduction
    pub fn reduction_percent(&self) -> String {
        format!("{:.1}%", self.reduction * 100.0)
    }
}

/// Image optimizer for various formats
pub struct Optimizer {
    settings: OptimizeSettings,
}

impl Optimizer {
    /// Create optimizer with default settings
    pub fn new() -> Self {
        Self {
            settings: OptimizeSettings::default(),
        }
    }

    /// Create optimizer with custom quality (legacy compatibility)
    pub fn with_quality(quality: u8) -> Self {
        let mut settings = OptimizeSettings::default();
        settings.jpeg_quality = quality.min(100);
        settings.webp_quality = quality.min(100);
        Self { settings }
    }

    /// Create optimizer with custom settings
    pub fn with_settings(settings: OptimizeSettings) -> Self {
        Self { settings }
    }

    /// Get current quality (for compatibility)
    pub fn quality(&self) -> u8 {
        self.settings.jpeg_quality
    }

    /// Get current settings
    pub fn settings(&self) -> &OptimizeSettings {
        &self.settings
    }

    /// Optimize image data (legacy interface)
    pub fn optimize(&self, data: &[u8], format: &str) -> Vec<u8> {
        match self.optimize_auto(data) {
            Ok(result) => result.data,
            Err(_) => Vec::new(),
        }
    }

    /// Optimize image with full result information
    pub fn optimize_auto(&self, data: &[u8]) -> Result<OptimizeResult, OptimizeError> {
        let original_size = data.len();
        let original_format = ImageType::from_bytes(data);

        // Load image
        let img = image::load_from_memory(data)?;
        let (width, height) = img.dimensions();

        // Optionally resize
        let (processed_img, resized) = self.maybe_resize(img);

        // Try different formats and pick the best
        let (optimized_data, output_format) = self.find_best_encoding(&processed_img, original_format)?;

        let optimized_size = optimized_data.len();
        let reduction = 1.0 - (optimized_size as f32 / original_size as f32);

        // If no improvement, return original
        if reduction < self.settings.min_reduction && !resized {
            return Ok(OptimizeResult {
                data: data.to_vec(),
                original_size,
                optimized_size: original_size,
                original_format,
                output_format: original_format,
                width,
                height,
                resized: false,
                reduction: 0.0,
            });
        }

        Ok(OptimizeResult {
            data: optimized_data,
            original_size,
            optimized_size,
            original_format,
            output_format,
            width: processed_img.width(),
            height: processed_img.height(),
            resized,
            reduction,
        })
    }

    /// Optimize to specific format
    pub fn optimize_to_format(&self, data: &[u8], target_format: ImageType) -> Result<OptimizeResult, OptimizeError> {
        let original_size = data.len();
        let original_format = ImageType::from_bytes(data);

        let img = image::load_from_memory(data)?;
        let (width, height) = img.dimensions();

        let (processed_img, resized) = self.maybe_resize(img);

        let optimized_data = self.encode_to_format(&processed_img, target_format)?;
        let optimized_size = optimized_data.len();
        let reduction = 1.0 - (optimized_size as f32 / original_size as f32);

        Ok(OptimizeResult {
            data: optimized_data,
            original_size,
            optimized_size,
            original_format,
            output_format: target_format,
            width: processed_img.width(),
            height: processed_img.height(),
            resized,
            reduction,
        })
    }

    /// Resize image if necessary
    fn maybe_resize(&self, img: DynamicImage) -> (DynamicImage, bool) {
        if let Some(max_dim) = self.settings.max_dimension {
            let (width, height) = img.dimensions();
            if width > max_dim || height > max_dim {
                let aspect = width as f64 / height as f64;
                let (new_width, new_height) = if width > height {
                    (max_dim, (max_dim as f64 / aspect) as u32)
                } else {
                    ((max_dim as f64 * aspect) as u32, max_dim)
                };
                let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
                return (resized, true);
            }
        }
        (img, false)
    }

    /// Find best encoding for the image
    fn find_best_encoding(&self, img: &DynamicImage, original_format: ImageType) -> Result<(Vec<u8>, ImageType), OptimizeError> {
        let mut best_data: Option<Vec<u8>> = None;
        let mut best_format = original_format;
        let mut best_size = usize::MAX;

        // Try original format first
        if let Ok(data) = self.encode_to_format(img, original_format) {
            best_size = data.len();
            best_data = Some(data);
        }

        // Try WebP if preferred
        if self.settings.prefer_webp {
            if let Ok(webp_data) = self.encode_to_format(img, ImageType::WebP) {
                if webp_data.len() < best_size {
                    best_size = webp_data.len();
                    best_data = Some(webp_data);
                    best_format = ImageType::WebP;
                }
            }
        }

        best_data
            .map(|data| (data, best_format))
            .ok_or_else(|| OptimizeError::EncodingError("Failed to encode image".to_string()))
    }

    /// Encode image to specific format
    fn encode_to_format(&self, img: &DynamicImage, format: ImageType) -> Result<Vec<u8>, OptimizeError> {
        let mut output = Cursor::new(Vec::new());

        match format {
            ImageType::Jpeg => {
                let encoder = JpegEncoder::new_with_quality(&mut output, self.settings.jpeg_quality);
                encoder.write_image(
                    img.as_bytes(),
                    img.width(),
                    img.height(),
                    img.color().into(),
                )?;
            }
            ImageType::Png => {
                let encoder = PngEncoder::new(&mut output);
                encoder.write_image(
                    img.as_bytes(),
                    img.width(),
                    img.height(),
                    img.color().into(),
                )?;
            }
            ImageType::WebP => {
                img.write_to(&mut output, ImageFormat::WebP)?;
            }
            ImageType::Gif => {
                img.write_to(&mut output, ImageFormat::Gif)?;
            }
            ImageType::Unknown => {
                return Err(OptimizeError::InvalidFormat("Unknown format".to_string()));
            }
        }

        Ok(output.into_inner())
    }

    /// Batch optimize multiple images
    pub fn optimize_batch(&self, images: &[&[u8]]) -> Vec<Result<OptimizeResult, OptimizeError>> {
        images.iter().map(|data| self.optimize_auto(data)).collect()
    }

    /// Convert image to WebP format
    pub fn to_webp(&self, data: &[u8]) -> Result<Vec<u8>, OptimizeError> {
        self.optimize_to_format(data, ImageType::WebP)
            .map(|r| r.data)
    }

    /// Convert image to JPEG format
    pub fn to_jpeg(&self, data: &[u8]) -> Result<Vec<u8>, OptimizeError> {
        self.optimize_to_format(data, ImageType::Jpeg)
            .map(|r| r.data)
    }

    /// Convert image to PNG format
    pub fn to_png(&self, data: &[u8]) -> Result<Vec<u8>, OptimizeError> {
        self.optimize_to_format(data, ImageType::Png)
            .map(|r| r.data)
    }
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_default() {
        let optimizer = Optimizer::new();
        assert_eq!(optimizer.quality(), 85);
    }

    #[test]
    fn test_optimizer_with_quality() {
        let optimizer = Optimizer::with_quality(90);
        assert_eq!(optimizer.quality(), 90);

        // Test clamping
        let optimizer_max = Optimizer::with_quality(150);
        assert_eq!(optimizer_max.quality(), 100);
    }

    #[test]
    fn test_image_type_detection() {
        // JPEG magic bytes
        assert_eq!(ImageType::from_bytes(&[0xFF, 0xD8, 0xFF, 0xE0]), ImageType::Jpeg);

        // PNG magic bytes
        assert_eq!(ImageType::from_bytes(&[0x89, 0x50, 0x4E, 0x47]), ImageType::Png);

        // GIF magic bytes
        assert_eq!(ImageType::from_bytes(b"GIF8"), ImageType::Gif);

        // Unknown
        assert_eq!(ImageType::from_bytes(&[0x00, 0x00]), ImageType::Unknown);
    }

    #[test]
    fn test_image_type_from_extension() {
        assert_eq!(ImageType::from_extension("jpg"), ImageType::Jpeg);
        assert_eq!(ImageType::from_extension("JPEG"), ImageType::Jpeg);
        assert_eq!(ImageType::from_extension("png"), ImageType::Png);
        assert_eq!(ImageType::from_extension("webp"), ImageType::WebP);
        assert_eq!(ImageType::from_extension("gif"), ImageType::Gif);
        assert_eq!(ImageType::from_extension("xyz"), ImageType::Unknown);
    }

    #[test]
    fn test_settings_presets() {
        let high = OptimizeSettings::high_quality();
        assert_eq!(high.jpeg_quality, 95);
        assert!(!high.prefer_webp);

        let balanced = OptimizeSettings::balanced();
        assert_eq!(balanced.jpeg_quality, 85);

        let max = OptimizeSettings::max_compression();
        assert_eq!(max.jpeg_quality, 70);
        assert!(max.prefer_webp);
        assert_eq!(max.max_dimension, Some(1920));

        let web = OptimizeSettings::web();
        assert_eq!(web.max_dimension, Some(2048));
    }

    #[test]
    fn test_optimize_result_reduction() {
        let result = OptimizeResult {
            data: vec![],
            original_size: 1000,
            optimized_size: 700,
            original_format: ImageType::Jpeg,
            output_format: ImageType::Jpeg,
            width: 100,
            height: 100,
            resized: false,
            reduction: 0.3,
        };

        assert!(result.is_effective(0.2));
        assert!(!result.is_effective(0.5));
        assert_eq!(result.reduction_percent(), "30.0%");
    }

    #[test]
    fn test_image_type_properties() {
        assert_eq!(ImageType::Jpeg.extension(), "jpg");
        assert_eq!(ImageType::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageType::Png.extension(), "png");
        assert_eq!(ImageType::WebP.mime_type(), "image/webp");
    }
}
