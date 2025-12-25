//! Image optimizer and cache layer for MDM
//! 
//! Provides WebP conversion, image compression, and caching support

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// Image optimization settings
#[derive(Debug, Clone)]
pub struct OptimizeSettings {
    /// Target format (webp, jpeg, png)
    pub format: String,
    /// Quality (1-100)
    pub quality: u8,
    /// Max width (0 = no limit)
    pub max_width: u32,
    /// Max height (0 = no limit)
    pub max_height: u32,
    /// Enable lossless compression
    pub lossless: bool,
}

impl Default for OptimizeSettings {
    fn default() -> Self {
        OptimizeSettings {
            format: "webp".to_string(),
            quality: 85,
            max_width: 0,
            max_height: 0,
            lossless: false,
        }
    }
}

/// Image optimizer
pub struct ImageOptimizer {
    settings: OptimizeSettings,
}

impl ImageOptimizer {
    pub fn new(settings: OptimizeSettings) -> Self {
        ImageOptimizer { settings }
    }

    /// Optimize image data
    /// 
    /// Currently returns original data with format conversion placeholder.
    /// Full implementation would use image crate for actual conversion.
    pub fn optimize(&self, image_data: &[u8], original_format: &str) -> io::Result<OptimizedImage> {
        // Detect input format
        let input_format = if !original_format.is_empty() {
            original_format.to_string()
        } else {
            detect_format(image_data)
        };

        // For now, return original data with metadata
        // Full implementation would convert using `image` crate
        let output_format = if self.settings.format == "auto" {
            // Keep original format or convert to webp
            if input_format == "gif" {
                "gif".to_string() // Preserve animations
            } else {
                "webp".to_string()
            }
        } else {
            self.settings.format.clone()
        };

        Ok(OptimizedImage {
            data: image_data.to_vec(),
            format: output_format,
            original_size: image_data.len(),
            optimized_size: image_data.len(), // Same for now
            width: 0,
            height: 0,
        })
    }
}

/// Optimized image result
#[derive(Debug)]
pub struct OptimizedImage {
    pub data: Vec<u8>,
    pub format: String,
    pub original_size: usize,
    pub optimized_size: usize,
    pub width: u32,
    pub height: u32,
}

impl OptimizedImage {
    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            1.0
        } else {
            self.optimized_size as f64 / self.original_size as f64
        }
    }

    /// Size savings in bytes
    pub fn bytes_saved(&self) -> i64 {
        self.original_size as i64 - self.optimized_size as i64
    }
}

/// Detect image format from magic bytes
fn detect_format(data: &[u8]) -> String {
    if data.len() < 4 {
        return "unknown".to_string();
    }

    if data[0] == 0xFF && data[1] == 0xD8 {
        "jpeg".to_string()
    } else if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        "png".to_string()
    } else if data[0] == 0x47 && data[1] == 0x49 && data[2] == 0x46 {
        "gif".to_string()
    } else if data[0] == 0x42 && data[1] == 0x4D {
        "bmp".to_string()
    } else if &data[0..4] == b"RIFF" && data.len() >= 12 && &data[8..12] == b"WEBP" {
        "webp".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Simple file-based cache
pub struct FileCache {
    cache_dir: PathBuf,
    index: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    path: PathBuf,
    size: usize,
    hash: String,
}

impl FileCache {
    /// Create new cache at specified directory
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> io::Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        
        Ok(FileCache {
            cache_dir,
            index: HashMap::new(),
        })
    }

    /// Generate cache key from content
    fn make_key(data: &[u8]) -> String {
        // Simple hash using first/last bytes and length
        let len = data.len();
        if len == 0 {
            return "empty".to_string();
        }
        
        let first = data[0] as u32;
        let last = data[len - 1] as u32;
        let mid = if len > 1 { data[len / 2] as u32 } else { 0 };
        
        format!("{:08x}-{:08x}-{}", first * 31 + mid * 17 + last, len, len % 1000)
    }

    /// Get cached item
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        if let Some(entry) = self.index.get(key) {
            if entry.path.exists() {
                return fs::read(&entry.path).ok();
            }
        }
        None
    }

    /// Store item in cache
    pub fn put(&mut self, key: &str, data: &[u8]) -> io::Result<()> {
        let path = self.cache_dir.join(format!("{}.cache", key));
        
        let mut file = fs::File::create(&path)?;
        file.write_all(data)?;
        
        self.index.insert(key.to_string(), CacheEntry {
            path,
            size: data.len(),
            hash: key.to_string(),
        });
        
        Ok(())
    }

    /// Get or compute with cache
    pub fn get_or_compute<F>(&mut self, data: &[u8], compute: F) -> io::Result<Vec<u8>>
    where
        F: FnOnce(&[u8]) -> io::Result<Vec<u8>>,
    {
        let key = Self::make_key(data);
        
        if let Some(cached) = self.get(&key) {
            return Ok(cached);
        }
        
        let result = compute(data)?;
        self.put(&key, &result)?;
        
        Ok(result)
    }

    /// Clear all cached items
    pub fn clear(&mut self) -> io::Result<()> {
        for (_, entry) in self.index.drain() {
            let _ = fs::remove_file(&entry.path);
        }
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_size: usize = self.index.values().map(|e| e.size).sum();
        
        CacheStats {
            item_count: self.index.len(),
            total_size,
            cache_dir: self.cache_dir.clone(),
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub item_count: usize,
    pub total_size: usize,
    pub cache_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimize_settings_default() {
        let settings = OptimizeSettings::default();
        assert_eq!(settings.format, "webp");
        assert_eq!(settings.quality, 85);
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpeg");
        assert_eq!(detect_format(&[0x89, 0x50, 0x4E, 0x47]), "png");
        assert_eq!(detect_format(&[0x47, 0x49, 0x46, 0x38]), "gif");
        assert_eq!(detect_format(&[0x42, 0x4D, 0x00, 0x00]), "bmp");
    }

    #[test]
    fn test_cache_key_generation() {
        let data1 = b"test data 1";
        let data2 = b"test data 2";
        
        let key1 = FileCache::make_key(data1);
        let key2 = FileCache::make_key(data2);
        
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_optimized_image_stats() {
        let img = OptimizedImage {
            data: vec![0; 100],
            format: "webp".to_string(),
            original_size: 200,
            optimized_size: 100,
            width: 100,
            height: 100,
        };
        
        assert_eq!(img.compression_ratio(), 0.5);
        assert_eq!(img.bytes_saved(), 100);
    }
}
