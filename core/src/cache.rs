//! Caching module for rendered media

use std::collections::HashMap;

/// Simple in-memory cache for rendered media
pub struct Cache {
    store: HashMap<String, Vec<u8>>,
    max_size: usize,
}

impl Cache {
    pub fn new(max_size: usize) -> Self {
        Self {
            store: HashMap::new(),
            max_size,
        }
    }

    /// Get cached item by key
    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.store.get(key)
    }

    /// Store item in cache
    pub fn set(&mut self, key: String, value: Vec<u8>) -> bool {
        if self.store.len() >= self.max_size {
            if let Some(first_key) = self.store.keys().next().cloned() {
                self.store.remove(&first_key);
            }
        }
        self.store.insert(key, value);
        true
    }

    /// Clear all cached items
    pub fn clear(&mut self) {
        self.store.clear();
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_operations() {
        let mut cache = Cache::new(10);
        cache.set("key1".to_string(), vec![1, 2, 3]);
        assert!(cache.get("key1").is_some());
        assert_eq!(cache.len(), 1);
    }
}
