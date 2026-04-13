//! MDM Manifest v2 -- asset index with content-addressable storage
//!
//! Provides a structured manifest for tracking source documents and their
//! extracted assets (images, tables, charts, equations) using SHA-256 hashes
//! for content deduplication and addressable storage paths.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Top-level manifest describing a converted document and its assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestV2 {
    /// Manifest schema version, always "2.0"
    pub version: String,
    /// Information about the source document
    pub source: SourceInfo,
    /// All extracted assets with content-addressable paths
    pub assets: Vec<Asset>,
    /// Aggregate conversion statistics
    pub stats: ConversionStats,
}

/// Metadata about the original source document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Original filename (e.g. "report.hwp")
    pub filename: String,
    /// Document format: "hwp", "pdf", "docx", "hwpx"
    pub format: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// SHA-256 hex digest of the source file
    pub hash: String,
    /// Document title extracted from metadata
    pub title: Option<String>,
    /// Document author extracted from metadata
    pub author: Option<String>,
    /// Number of pages (if applicable)
    pub pages: Option<usize>,
}

/// A single extracted asset with its content hash and storage path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// Unique identifier within the manifest (e.g. "image_001")
    pub id: String,
    /// The kind of media this asset represents
    pub media_type: MediaType,
    /// Relative path from bundle root (e.g. "assets/images/a1b2c3d4e5f6.png")
    pub src: String,
    /// SHA-256 hex digest of the asset content
    pub content_hash: String,
    /// Original filename before content-addressing, if known
    pub original_name: Option<String>,
    /// Additional metadata about the asset
    pub metadata: AssetMetadata,
}

/// Classification of extracted media assets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Table,
    Chart,
    Equation,
    Video,
    Audio,
    Embed,
}

/// Optional metadata attached to an asset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Source page number (1-indexed)
    pub page: Option<usize>,
    /// Width in pixels
    pub width: Option<u32>,
    /// Height in pixels
    pub height: Option<u32>,
    /// File format extension (e.g. "png", "svg", "jpg")
    pub format: Option<String>,
    /// Caption text associated with the asset
    pub caption: Option<String>,
    /// Alt text for accessibility
    pub alt_text: Option<String>,
    /// Position within the source page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// Coordinates of an asset within its source page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Aggregate statistics from the conversion process.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionStats {
    pub total_assets: usize,
    pub images: usize,
    pub tables: usize,
    pub charts: usize,
    pub equations: usize,
    pub markdown_lines: usize,
    pub markdown_chars: usize,
    pub conversion_ms: u64,
}

impl ManifestV2 {
    /// Create a new manifest for a source file.
    ///
    /// Reads the file to compute its size and SHA-256 hash.
    /// If the file cannot be read, size defaults to 0 and hash to "unknown".
    pub fn new(source_path: &Path, format: &str) -> Self {
        let size = std::fs::metadata(source_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let hash = compute_file_hash(source_path);

        Self {
            version: "2.0".to_string(),
            source: SourceInfo {
                filename: source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                format: format.to_string(),
                size_bytes: size,
                hash,
                title: None,
                author: None,
                pages: None,
            },
            assets: Vec::new(),
            stats: ConversionStats::default(),
        }
    }

    /// Add an asset with content-addressable naming.
    ///
    /// Uses the first 12 hex characters of the SHA-256 hash as the filename.
    /// If an asset with the same content hash already exists, it is not added
    /// again (deduplication), but the filename is still returned.
    ///
    /// Returns the content-addressed filename (e.g. "a1b2c3d4e5f6.png").
    pub fn add_asset(
        &mut self,
        data: &[u8],
        media_type: MediaType,
        ext: &str,
        metadata: AssetMetadata,
    ) -> String {
        let hash = compute_hash(data);
        let short_hash = &hash[..12];
        let filename = format!("{short_hash}.{ext}");

        // Deduplicate: skip if this exact content already tracked
        if self.assets.iter().any(|a| a.content_hash == hash) {
            return filename;
        }

        let id = match media_type {
            MediaType::Image => format!("image_{:03}", self.stats.images + 1),
            MediaType::Table => format!("table_{:03}", self.stats.tables + 1),
            MediaType::Chart => format!("chart_{:03}", self.stats.charts + 1),
            MediaType::Equation => format!("eq_{:03}", self.stats.equations + 1),
            MediaType::Video | MediaType::Audio | MediaType::Embed => {
                format!("asset_{:03}", self.stats.total_assets + 1)
            }
        };

        let subdir = match media_type {
            MediaType::Image => "images",
            MediaType::Table | MediaType::Chart => "tables",
            MediaType::Equation => "equations",
            MediaType::Video | MediaType::Audio | MediaType::Embed => "other",
        };

        self.assets.push(Asset {
            id,
            media_type: media_type.clone(),
            src: format!("assets/{subdir}/{filename}"),
            content_hash: hash,
            original_name: None,
            metadata,
        });

        self.stats.total_assets += 1;
        match media_type {
            MediaType::Image => self.stats.images += 1,
            MediaType::Table => self.stats.tables += 1,
            MediaType::Chart => self.stats.charts += 1,
            MediaType::Equation => self.stats.equations += 1,
            MediaType::Video | MediaType::Audio | MediaType::Embed => {}
        }

        filename
    }

    /// Look up an asset by its content hash.
    pub fn find_by_hash(&self, hash: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.content_hash == hash)
    }

    /// Serialize the manifest to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a manifest from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Compute the SHA-256 hex digest of a byte slice.
fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute the SHA-256 hex digest of a file's contents.
///
/// Returns "unknown" if the file cannot be read.
fn compute_file_hash(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(data) => compute_hash(&data),
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let data = b"hello world";
        let h1 = compute_hash(data);
        let h2 = compute_hash(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn test_compute_hash_differs_for_different_input() {
        let h1 = compute_hash(b"aaa");
        let h2 = compute_hash(b"bbb");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_add_asset_returns_content_addressed_filename() {
        let mut m = make_test_manifest();
        let fname = m.add_asset(b"img data", MediaType::Image, "png", AssetMetadata::default());
        assert!(fname.ends_with(".png"));
        assert_eq!(fname.len(), 12 + 1 + 3); // 12-char hash + '.' + ext
    }

    #[test]
    fn test_add_asset_dedup() {
        let mut m = make_test_manifest();
        let data = b"duplicate image bytes";

        let f1 = m.add_asset(data, MediaType::Image, "png", AssetMetadata::default());
        let f2 = m.add_asset(data, MediaType::Image, "png", AssetMetadata::default());

        assert_eq!(f1, f2);
        assert_eq!(m.assets.len(), 1);
        assert_eq!(m.stats.images, 1);
        assert_eq!(m.stats.total_assets, 1);
    }

    #[test]
    fn test_add_different_assets_increments_stats() {
        let mut m = make_test_manifest();

        m.add_asset(b"img1", MediaType::Image, "png", AssetMetadata::default());
        m.add_asset(b"img2", MediaType::Image, "jpg", AssetMetadata::default());
        m.add_asset(b"tbl1", MediaType::Table, "html", AssetMetadata::default());
        m.add_asset(b"eq1", MediaType::Equation, "svg", AssetMetadata::default());

        assert_eq!(m.stats.total_assets, 4);
        assert_eq!(m.stats.images, 2);
        assert_eq!(m.stats.tables, 1);
        assert_eq!(m.stats.equations, 1);
        assert_eq!(m.assets.len(), 4);
    }

    #[test]
    fn test_asset_ids_are_sequential() {
        let mut m = make_test_manifest();

        m.add_asset(b"a", MediaType::Image, "png", AssetMetadata::default());
        m.add_asset(b"b", MediaType::Image, "png", AssetMetadata::default());

        assert_eq!(m.assets[0].id, "image_001");
        assert_eq!(m.assets[1].id, "image_002");
    }

    #[test]
    fn test_asset_src_paths() {
        let mut m = make_test_manifest();

        m.add_asset(b"img", MediaType::Image, "png", AssetMetadata::default());
        assert!(m.assets[0].src.starts_with("assets/images/"));

        m.add_asset(b"tbl", MediaType::Table, "html", AssetMetadata::default());
        assert!(m.assets[1].src.starts_with("assets/tables/"));

        m.add_asset(b"eq", MediaType::Equation, "svg", AssetMetadata::default());
        assert!(m.assets[2].src.starts_with("assets/equations/"));

        m.add_asset(b"vid", MediaType::Video, "mp4", AssetMetadata::default());
        assert!(m.assets[3].src.starts_with("assets/other/"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut m = make_test_manifest();
        m.add_asset(
            b"roundtrip test",
            MediaType::Image,
            "png",
            AssetMetadata {
                page: Some(1),
                width: Some(800),
                height: Some(600),
                format: Some("png".to_string()),
                caption: Some("Test image".to_string()),
                alt_text: None,
                position: Some(Position { x: 10.0, y: 20.0 }),
            },
        );

        let json = m.to_json().expect("serialize");
        let restored = ManifestV2::from_json(&json).expect("deserialize");

        assert_eq!(restored.version, "2.0");
        assert_eq!(restored.assets.len(), 1);
        assert_eq!(restored.assets[0].media_type, MediaType::Image);
        assert_eq!(restored.assets[0].metadata.page, Some(1));
        assert_eq!(restored.stats.images, 1);
    }

    #[test]
    fn test_find_by_hash() {
        let mut m = make_test_manifest();
        let data = b"searchable";
        let hash = compute_hash(data);
        m.add_asset(data, MediaType::Image, "png", AssetMetadata::default());

        assert!(m.find_by_hash(&hash).is_some());
        assert!(m.find_by_hash("nonexistent").is_none());
    }

    #[test]
    fn test_new_with_nonexistent_file() {
        let m = ManifestV2::new(Path::new("/nonexistent/file.pdf"), "pdf");
        assert_eq!(m.version, "2.0");
        assert_eq!(m.source.format, "pdf");
        assert_eq!(m.source.size_bytes, 0);
        assert_eq!(m.source.hash, "unknown");
    }

    #[test]
    fn test_media_type_serde() {
        let json = serde_json::to_string(&MediaType::Image).unwrap();
        assert_eq!(json, "\"image\"");

        let restored: MediaType = serde_json::from_str("\"equation\"").unwrap();
        assert_eq!(restored, MediaType::Equation);
    }

    /// Helper to create a minimal manifest for testing.
    fn make_test_manifest() -> ManifestV2 {
        ManifestV2 {
            version: "2.0".to_string(),
            source: SourceInfo {
                filename: "test.pdf".to_string(),
                format: "pdf".to_string(),
                size_bytes: 1000,
                hash: "abc123".to_string(),
                title: None,
                author: None,
                pages: None,
            },
            assets: Vec::new(),
            stats: ConversionStats::default(),
        }
    }
}
