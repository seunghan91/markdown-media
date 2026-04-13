//! Plugin architecture for format-extensible document conversion.
//!
//! Provides a `FormatPlugin` trait and a compile-time `PluginRegistry` that
//! allows new format parsers to be added without modifying core conversion
//! logic. This is a **static** plugin system (no dynamic loading).
//!
//! # Example
//!
//! ```rust
//! use mdm_core::plugin::PluginRegistry;
//!
//! let registry = PluginRegistry::with_builtins();
//! let plugins = registry.list();
//! assert!(plugins.iter().any(|(name, _)| *name == "CSV/TSV"));
//! ```

use crate::manifest::{AssetMetadata, MediaType};
use std::io;
use std::path::Path;

/// Result of converting a document into Markdown and extracted assets.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Rendered Markdown content.
    pub markdown: String,
    /// Media assets extracted during conversion (images, charts, etc.).
    pub assets: Vec<ExtractedAsset>,
    /// Document title, if available from metadata.
    pub title: Option<String>,
    /// Document author, if available from metadata.
    pub author: Option<String>,
    /// Page count, if applicable.
    pub pages: Option<usize>,
}

/// A media asset extracted from a source document.
#[derive(Debug, Clone)]
pub struct ExtractedAsset {
    /// Raw asset bytes.
    pub data: Vec<u8>,
    /// Classification of this asset.
    pub media_type: MediaType,
    /// File extension without leading dot (e.g. `"png"`).
    pub extension: String,
    /// Additional metadata (dimensions, page number, etc.).
    pub metadata: AssetMetadata,
}

/// Trait that all format plugins must implement.
///
/// Provides a uniform interface for converting various document formats into
/// Markdown with optional extracted media assets.
pub trait FormatPlugin: Send + Sync {
    /// File extensions this plugin handles, lowercase without dots.
    ///
    /// Example: `&["pdf"]` or `&["xlsx", "xls"]`.
    fn extensions(&self) -> &[&str];

    /// Human-readable format name (e.g. `"PDF"`, `"CSV/TSV"`).
    fn name(&self) -> &str;

    /// Convert raw file bytes into Markdown and extracted assets.
    ///
    /// `filename` is provided for format hinting (e.g. distinguishing `.csv`
    /// from `.tsv` when the same plugin handles both).
    fn convert_bytes(&self, data: &[u8], filename: &str) -> io::Result<ConversionResult>;

    /// Convert a file on disk. The default implementation reads the file then
    /// delegates to [`convert_bytes`](FormatPlugin::convert_bytes).
    fn convert_file(&self, path: &Path) -> io::Result<ConversionResult> {
        let data = std::fs::read(path)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        self.convert_bytes(&data, filename)
    }
}

// ---------------------------------------------------------------------------
// Built-in plugin implementations
// ---------------------------------------------------------------------------

/// Plugin adapter for the CSV/TSV parser.
pub struct CsvPlugin;

impl FormatPlugin for CsvPlugin {
    fn extensions(&self) -> &[&str] {
        &["csv", "tsv"]
    }

    fn name(&self) -> &str {
        "CSV/TSV"
    }

    fn convert_bytes(&self, data: &[u8], _filename: &str) -> io::Result<ConversionResult> {
        let parser = crate::csv_parser::CsvParser::from_bytes(data.to_vec())?;
        let doc = parser.parse()?;
        Ok(ConversionResult {
            markdown: doc.to_markdown(),
            assets: vec![],
            title: None,
            author: None,
            pages: None,
        })
    }
}

/// Plugin adapter for the plain text parser.
pub struct TxtPlugin;

impl FormatPlugin for TxtPlugin {
    fn extensions(&self) -> &[&str] {
        &["txt", "text", "log"]
    }

    fn name(&self) -> &str {
        "Plain Text"
    }

    fn convert_bytes(&self, data: &[u8], _filename: &str) -> io::Result<ConversionResult> {
        let parser = crate::txt_parser::TxtParser::from_bytes(data.to_vec())?;
        Ok(ConversionResult {
            markdown: parser.to_markdown(),
            assets: vec![],
            title: None,
            author: None,
            pages: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Registry of available format plugins.
///
/// Holds a collection of [`FormatPlugin`] trait objects and provides lookup by
/// file extension.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn FormatPlugin>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Create a registry pre-populated with all built-in format plugins.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(CsvPlugin));
        reg.register(Box::new(TxtPlugin));
        reg
    }

    /// Register a new format plugin.
    pub fn register(&mut self, plugin: Box<dyn FormatPlugin>) {
        self.plugins.push(plugin);
    }

    /// Find a plugin that handles the given file extension.
    ///
    /// The lookup is case-insensitive. Returns `None` if no plugin matches.
    pub fn find_by_extension(&self, ext: &str) -> Option<&dyn FormatPlugin> {
        let ext_lower = ext.to_lowercase();
        self.plugins
            .iter()
            .find(|p| {
                p.extensions()
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(&ext_lower))
            })
            .map(|p| p.as_ref())
    }

    /// List all registered plugins as `(name, extensions)` pairs.
    pub fn list(&self) -> Vec<(&str, &[&str])> {
        self.plugins
            .iter()
            .map(|p| (p.name(), p.extensions()))
            .collect()
    }

    /// Return the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_with_builtins() {
        let reg = PluginRegistry::with_builtins();
        assert!(reg.len() >= 2);

        let names: Vec<&str> = reg.list().iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"CSV/TSV"));
        assert!(names.contains(&"Plain Text"));
    }

    #[test]
    fn test_find_by_extension() {
        let reg = PluginRegistry::with_builtins();

        assert!(reg.find_by_extension("csv").is_some());
        assert!(reg.find_by_extension("CSV").is_some());
        assert!(reg.find_by_extension("tsv").is_some());
        assert!(reg.find_by_extension("txt").is_some());
        assert!(reg.find_by_extension("log").is_some());
        assert!(reg.find_by_extension("unknown").is_none());
    }

    #[test]
    fn test_csv_plugin_convert() {
        let plugin = CsvPlugin;
        let data = b"Name,Age\nAlice,30\nBob,25";
        let result = plugin.convert_bytes(data, "test.csv").unwrap();
        assert!(result.markdown.contains("Alice"));
        assert!(result.markdown.contains("| Name"));
        assert!(result.assets.is_empty());
    }

    #[test]
    fn test_txt_plugin_convert() {
        let plugin = TxtPlugin;
        let data = b"Hello, world!\nLine two.";
        let result = plugin.convert_bytes(data, "test.txt").unwrap();
        assert!(result.markdown.contains("Hello, world!"));
        assert!(result.assets.is_empty());
    }

    #[test]
    fn test_empty_registry() {
        let reg = PluginRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.find_by_extension("csv").is_none());
    }

    #[test]
    fn test_plugin_name_from_extension() {
        let reg = PluginRegistry::with_builtins();
        let plugin = reg.find_by_extension("txt").unwrap();
        assert_eq!(plugin.name(), "Plain Text");
    }
}
