//! SVG Renderer module
//!
//! Provides rendering capabilities for various image formats including:
//! - SVG generation and inline embedding
//! - SVG to PNG/WebP conversion
//! - Responsive image generation with srcset
//! - WebP encoding and optimization

use image::{DynamicImage, ImageFormat, ImageEncoder, ImageBuffer, Rgba};
use image::codecs::png::PngEncoder;
use image::codecs::jpeg::JpegEncoder;
use resvg::tiny_skia;
use resvg::usvg;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;

/// Render errors
#[derive(Error, Debug)]
pub enum RenderError {
    #[error("SVG parsing error: {0}")]
    SvgParseError(String),
    #[error("Image encoding error: {0}")]
    EncodingError(String),
    #[error("Invalid dimensions: {0}")]
    InvalidDimensions(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
}

/// Output format for rendered images
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Png,
    Jpeg,
    WebP,
    Svg,
}

impl OutputFormat {
    /// Get file extension for format
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::WebP => "webp",
            OutputFormat::Svg => "svg",
        }
    }

    /// Get MIME type for format
    pub fn mime_type(&self) -> &'static str {
        match self {
            OutputFormat::Png => "image/png",
            OutputFormat::Jpeg => "image/jpeg",
            OutputFormat::WebP => "image/webp",
            OutputFormat::Svg => "image/svg+xml",
        }
    }

    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(OutputFormat::Png),
            "jpg" | "jpeg" => Some(OutputFormat::Jpeg),
            "webp" => Some(OutputFormat::WebP),
            "svg" => Some(OutputFormat::Svg),
            _ => None,
        }
    }
}

/// Responsive image size preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveSize {
    pub width: u32,
    pub suffix: String,
    pub descriptor: String,  // e.g., "1x", "2x", "320w"
}

impl Default for ResponsiveSize {
    fn default() -> Self {
        Self {
            width: 800,
            suffix: "".to_string(),
            descriptor: "1x".to_string(),
        }
    }
}

/// Responsive image presets
pub struct ResponsivePresets;

impl ResponsivePresets {
    /// Standard web sizes
    pub fn web() -> Vec<ResponsiveSize> {
        vec![
            ResponsiveSize { width: 320, suffix: "-sm".to_string(), descriptor: "320w".to_string() },
            ResponsiveSize { width: 640, suffix: "-md".to_string(), descriptor: "640w".to_string() },
            ResponsiveSize { width: 1024, suffix: "-lg".to_string(), descriptor: "1024w".to_string() },
            ResponsiveSize { width: 1920, suffix: "-xl".to_string(), descriptor: "1920w".to_string() },
        ]
    }

    /// Retina display sizes
    pub fn retina() -> Vec<ResponsiveSize> {
        vec![
            ResponsiveSize { width: 400, suffix: "".to_string(), descriptor: "1x".to_string() },
            ResponsiveSize { width: 800, suffix: "@2x".to_string(), descriptor: "2x".to_string() },
            ResponsiveSize { width: 1200, suffix: "@3x".to_string(), descriptor: "3x".to_string() },
        ]
    }

    /// Thumbnail preset
    pub fn thumbnails() -> Vec<ResponsiveSize> {
        vec![
            ResponsiveSize { width: 100, suffix: "-thumb".to_string(), descriptor: "100w".to_string() },
            ResponsiveSize { width: 300, suffix: "-small".to_string(), descriptor: "300w".to_string() },
            ResponsiveSize { width: 600, suffix: "-medium".to_string(), descriptor: "600w".to_string() },
        ]
    }
}

/// Media render options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub border_radius: u32,
    pub quality: u8,
    pub format: OutputFormat,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            background: "#f0f0f0".to_string(),
            border_radius: 8,
            quality: 85,
            format: OutputFormat::Png,
        }
    }
}

/// Responsive image result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveImageResult {
    pub base_path: String,
    pub images: Vec<ResponsiveImageVariant>,
    pub srcset: String,
    pub sizes: String,
}

/// A single responsive image variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveImageVariant {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub format: OutputFormat,
    pub size_bytes: usize,
}

/// Core renderer for media content
pub struct Renderer {
    options: RenderOptions,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            options: RenderOptions::default(),
        }
    }

    pub fn with_options(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Get current options
    pub fn options(&self) -> &RenderOptions {
        &self.options
    }

    /// Render media to SVG string
    pub fn render_svg(&self, alt_text: &str, media_type: &str) -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
  <rect width="100%" height="100%" fill="{}" rx="{}"/>
  <text x="50%" y="45%" text-anchor="middle" fill="#666" font-family="sans-serif" font-size="14">{}</text>
  <text x="50%" y="55%" text-anchor="middle" fill="#999" font-family="sans-serif" font-size="12">{}</text>
</svg>"##,
            self.options.width,
            self.options.height,
            self.options.width,
            self.options.height,
            self.options.background,
            self.options.border_radius,
            alt_text,
            media_type
        )
    }

    /// Render SVG string to PNG bytes
    pub fn render_svg_to_png(&self, svg_content: &str) -> Result<Vec<u8>, RenderError> {
        self.render_svg_to_format(svg_content, OutputFormat::Png)
    }

    /// Render SVG string to WebP bytes
    pub fn render_svg_to_webp(&self, svg_content: &str) -> Result<Vec<u8>, RenderError> {
        self.render_svg_to_format(svg_content, OutputFormat::WebP)
    }

    /// Render SVG string to specified format
    pub fn render_svg_to_format(&self, svg_content: &str, format: OutputFormat) -> Result<Vec<u8>, RenderError> {
        if format == OutputFormat::Svg {
            return Ok(svg_content.as_bytes().to_vec());
        }

        // Parse SVG using usvg
        let options = usvg::Options::default();
        let tree = usvg::Tree::from_str(svg_content, &options)
            .map_err(|e| RenderError::SvgParseError(e.to_string()))?;

        let size = tree.size();
        let scale_x = self.options.width as f32 / size.width();
        let scale_y = self.options.height as f32 / size.height();
        let scale = scale_x.min(scale_y);

        let width = (size.width() * scale) as u32;
        let height = (size.height() * scale) as u32;

        if width == 0 || height == 0 {
            return Err(RenderError::InvalidDimensions(
                format!("Invalid output dimensions: {}x{}", width, height)
            ));
        }

        // Create pixmap and render
        let mut pixmap = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| RenderError::InvalidDimensions("Failed to create pixmap".to_string()))?;

        let transform = tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Convert to DynamicImage
        let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            width,
            height,
            pixmap.data().to_vec()
        ).ok_or_else(|| RenderError::EncodingError("Failed to create image buffer".to_string()))?;

        let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

        // Encode to target format
        self.encode_image(&dynamic_image, format)
    }

    /// Encode DynamicImage to bytes in specified format
    fn encode_image(&self, image: &DynamicImage, format: OutputFormat) -> Result<Vec<u8>, RenderError> {
        let mut output = Cursor::new(Vec::new());

        match format {
            OutputFormat::Png => {
                let encoder = PngEncoder::new(&mut output);
                encoder.write_image(
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image.color().into(),
                )?;
            }
            OutputFormat::Jpeg => {
                let encoder = JpegEncoder::new_with_quality(&mut output, self.options.quality);
                encoder.write_image(
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image.color().into(),
                )?;
            }
            OutputFormat::WebP => {
                // WebP encoding using image crate
                image.write_to(&mut output, ImageFormat::WebP)?;
            }
            OutputFormat::Svg => {
                return Err(RenderError::EncodingError(
                    "Cannot encode raster image to SVG".to_string()
                ));
            }
        }

        Ok(output.into_inner())
    }

    /// Render image to WebP format with optional resizing
    pub fn render_to_webp(&self, image_data: &[u8], target_width: Option<u32>) -> Result<Vec<u8>, RenderError> {
        let img = image::load_from_memory(image_data)?;

        let processed = if let Some(width) = target_width {
            if img.width() > width {
                let ratio = width as f64 / img.width() as f64;
                let height = (img.height() as f64 * ratio) as u32;
                img.resize(width, height, image::imageops::FilterType::Lanczos3)
            } else {
                img
            }
        } else {
            img
        };

        self.encode_image(&processed, OutputFormat::WebP)
    }

    /// Generate responsive images at multiple sizes
    pub fn render_responsive(
        &self,
        image_data: &[u8],
        base_path: &Path,
        sizes: &[ResponsiveSize],
        format: OutputFormat,
    ) -> Result<ResponsiveImageResult, RenderError> {
        let original = image::load_from_memory(image_data)?;
        let base_name = base_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image");
        let parent = base_path.parent().unwrap_or(Path::new("."));

        let mut images = Vec::new();
        let mut srcset_parts = Vec::new();

        for size in sizes {
            if size.width > original.width() {
                continue; // Skip sizes larger than original
            }

            let ratio = size.width as f64 / original.width() as f64;
            let height = (original.height() as f64 * ratio) as u32;

            let resized = original.resize(
                size.width,
                height,
                image::imageops::FilterType::Lanczos3,
            );

            let encoded = self.encode_image(&resized, format)?;
            let file_name = format!("{}{}.{}", base_name, size.suffix, format.extension());
            let file_path = parent.join(&file_name);

            // Write file
            std::fs::write(&file_path, &encoded)?;

            let variant = ResponsiveImageVariant {
                path: file_path.to_string_lossy().to_string(),
                width: size.width,
                height,
                format,
                size_bytes: encoded.len(),
            };

            srcset_parts.push(format!("{} {}", file_name, size.descriptor));
            images.push(variant);
        }

        Ok(ResponsiveImageResult {
            base_path: base_path.to_string_lossy().to_string(),
            images,
            srcset: srcset_parts.join(", "),
            sizes: "(max-width: 320px) 280px, (max-width: 640px) 600px, 1024px".to_string(),
        })
    }

    /// Generate HTML picture element with responsive sources
    pub fn render_responsive_html(
        &self,
        result: &ResponsiveImageResult,
        alt_text: &str,
        class: Option<&str>,
    ) -> String {
        let class_attr = class.map(|c| format!(" class=\"{}\"", c)).unwrap_or_default();

        // Group images by format for source elements
        let webp_sources: Vec<_> = result.images.iter()
            .filter(|img| img.format == OutputFormat::WebP)
            .collect();

        let fallback = result.images.iter()
            .find(|img| img.format != OutputFormat::WebP)
            .or_else(|| result.images.first());

        let mut html = String::from("<picture>\n");

        // WebP sources
        if !webp_sources.is_empty() {
            let webp_srcset: Vec<String> = webp_sources.iter()
                .map(|img| format!("{} {}w", img.path, img.width))
                .collect();
            html.push_str(&format!(
                "  <source type=\"image/webp\" srcset=\"{}\" sizes=\"{}\" />\n",
                webp_srcset.join(", "),
                result.sizes
            ));
        }

        // Fallback img element
        if let Some(img) = fallback {
            html.push_str(&format!(
                "  <img src=\"{}\" alt=\"{}\" width=\"{}\" height=\"{}\"{} loading=\"lazy\" />\n",
                img.path, alt_text, img.width, img.height, class_attr
            ));
        }

        html.push_str("</picture>");
        html
    }

    /// Render SVG for inline embedding (data URI)
    pub fn render_svg_inline(&self, svg_content: &str) -> String {
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            svg_content.as_bytes()
        );
        format!("data:image/svg+xml;base64,{}", encoded)
    }

    /// Render image as data URI
    pub fn render_data_uri(&self, image_data: &[u8], format: OutputFormat) -> Result<String, RenderError> {
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            image_data
        );
        Ok(format!("data:{};base64,{}", format.mime_type(), encoded))
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_svg() {
        let renderer = Renderer::new();
        let svg = renderer.render_svg("Test Image", "image/png");
        assert!(svg.contains("Test Image"));
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_output_format() {
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::WebP.mime_type(), "image/webp");
        assert_eq!(OutputFormat::from_extension("jpg"), Some(OutputFormat::Jpeg));
    }

    #[test]
    fn test_svg_inline() {
        let renderer = Renderer::new();
        let svg = "<svg><rect/></svg>";
        let data_uri = renderer.render_svg_inline(svg);
        assert!(data_uri.starts_with("data:image/svg+xml;base64,"));
    }

    #[test]
    fn test_responsive_presets() {
        let web = ResponsivePresets::web();
        assert_eq!(web.len(), 4);
        assert_eq!(web[0].width, 320);

        let retina = ResponsivePresets::retina();
        assert_eq!(retina.len(), 3);
        assert_eq!(retina[1].descriptor, "2x");
    }
}
