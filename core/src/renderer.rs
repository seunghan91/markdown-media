//! SVG Renderer module

use serde::{Deserialize, Serialize};

/// Media render options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub border_radius: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            background: "#f0f0f0".to_string(),
            border_radius: 8,
        }
    }
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
}
