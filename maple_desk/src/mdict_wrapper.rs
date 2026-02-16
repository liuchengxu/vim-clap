//! Thin wrapper around the `mdict-parser` crate for MDict (`.mdx`) support.
//!
//! Provides the same lookup interface as [`crate::stardict::StarDictionary`].
//! Loading is wrapped in `catch_unwind` because the upstream crate panics
//! on malformed, encrypted, or unsupported MDX files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A loaded MDict dictionary with HashMap-based O(1) lookup.
pub struct MdictDictionary {
    pub name: String,
    pub word_count: usize,
    /// Lowercased key -> HTML definition.
    entries: HashMap<String, String>,
}

impl MdictDictionary {
    /// Load a `.mdx` file, building a HashMap for instant lookups.
    ///
    /// Uses `catch_unwind` because `mdict-parser` panics on malformed input.
    pub fn load(mdx_path: &Path) -> Result<Self, String> {
        let data = std::fs::read(mdx_path)
            .map_err(|error| format!("Failed to read .mdx file: {error}"))?;

        let name = mdx_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        // Wrap in catch_unwind since mdict-parser panics on bad input
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mdx = mdict_parser::parser::parse(&data);
            let mut entries = HashMap::new();
            for record in mdx.items() {
                let key = record.key.to_lowercase();
                entries.insert(key, record.definition);
            }
            entries
        }));

        match result {
            Ok(entries) => {
                let word_count = entries.len();
                Ok(Self {
                    name,
                    word_count,
                    entries,
                })
            }
            Err(_) => Err(format!(
                "MDict parser panicked loading {} (possibly malformed, encrypted, or v3 format)",
                mdx_path.display()
            )),
        }
    }

    /// Case-insensitive lookup.
    pub fn lookup(&self, word: &str) -> Option<&str> {
        self.entries.get(&word.to_lowercase()).map(|s| s.as_str())
    }
}

/// Find all `.mdx` files in a directory (non-recursive).
pub fn scan_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mdx"))
        .collect()
}
