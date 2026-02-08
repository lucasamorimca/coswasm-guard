use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ast::contract_info::{
    EntryPoint, FunctionInfo, MessageEnum, StateItem,
};
use crate::ir::types::{ContractIr, FunctionIr};

/// Schema version — bump when cached struct layouts change
const SCHEMA_VERSION: u32 = 1;

/// Per-file cached artifact: visitor output + IR functions for one source file
#[derive(Serialize, Deserialize)]
pub struct CachedFileArtifact {
    pub entry_points: Vec<EntryPoint>,
    pub message_enums: Vec<MessageEnum>,
    pub state_items: Vec<StateItem>,
    pub functions: Vec<FunctionInfo>,
    pub ir_functions: Vec<FunctionIr>,
    pub ir_entry_points: Vec<String>,
}

/// Cache manifest tracking file hashes and artifact locations
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    files: HashMap<PathBuf, FileEntry>,
}

#[derive(Serialize, Deserialize)]
struct FileEntry {
    hash: String,
    artifact_file: String,
}

/// Manages file-level caching of parsed AST data and IR
pub struct CacheManager {
    cache_dir: PathBuf,
    manifest: Manifest,
}

impl CacheManager {
    /// Open or create a cache in the given directory
    pub fn open(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache dir: {}", cache_dir.display()))?;

        let artifacts_dir = cache_dir.join("artifacts");
        fs::create_dir_all(&artifacts_dir)?;

        let manifest_path = cache_dir.join("manifest.json");
        let manifest = if manifest_path.exists() {
            let data = fs::read_to_string(&manifest_path)?;
            let m: Manifest = serde_json::from_str(&data).unwrap_or_else(|_| Manifest {
                schema_version: SCHEMA_VERSION,
                files: HashMap::new(),
            });
            // Invalidate if schema version changed
            if m.schema_version != SCHEMA_VERSION {
                Manifest {
                    schema_version: SCHEMA_VERSION,
                    files: HashMap::new(),
                }
            } else {
                m
            }
        } else {
            Manifest {
                schema_version: SCHEMA_VERSION,
                files: HashMap::new(),
            }
        };

        Ok(Self {
            cache_dir,
            manifest,
        })
    }

    /// Compute SHA256 hash of file contents
    pub fn hash_contents(contents: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(contents.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Look up a cached artifact for a file. Returns None on miss or hash mismatch.
    pub fn lookup(&self, file_path: &Path, current_hash: &str) -> Option<CachedFileArtifact> {
        let entry = self.manifest.files.get(file_path)?;
        if entry.hash != current_hash {
            return None;
        }
        let artifact_path = self.cache_dir.join("artifacts").join(&entry.artifact_file);
        let data = fs::read(&artifact_path).ok()?;
        bincode::deserialize(&data).ok()
    }

    /// Store a cached artifact for a file
    pub fn store(
        &mut self,
        file_path: &Path,
        hash: &str,
        artifact: &CachedFileArtifact,
    ) -> Result<()> {
        let artifact_name = format!("{}.bin", &hash[..16]);
        let artifact_path = self.cache_dir.join("artifacts").join(&artifact_name);
        let data = bincode::serialize(artifact)?;
        fs::write(&artifact_path, data)?;

        self.manifest.files.insert(
            file_path.to_path_buf(),
            FileEntry {
                hash: hash.to_string(),
                artifact_file: artifact_name,
            },
        );
        Ok(())
    }

    /// Flush manifest to disk
    pub fn flush(&self) -> Result<()> {
        let manifest_path = self.cache_dir.join("manifest.json");
        let data = serde_json::to_string_pretty(&self.manifest)?;
        fs::write(manifest_path, data)?;
        Ok(())
    }

    /// Clear all cached artifacts
    pub fn clear(&mut self) -> Result<()> {
        let artifacts_dir = self.cache_dir.join("artifacts");
        if artifacts_dir.exists() {
            fs::remove_dir_all(&artifacts_dir)?;
            fs::create_dir_all(&artifacts_dir)?;
        }
        self.manifest.files.clear();
        self.flush()
    }

    /// Merge a cached artifact into ContractInfo and ContractIr
    pub fn merge_cached_into(
        artifact: &CachedFileArtifact,
        contract: &mut crate::ast::ContractInfo,
        ir: &mut ContractIr,
        file_path: PathBuf,
    ) {
        contract.source_files.push(file_path);
        contract
            .entry_points
            .extend(artifact.entry_points.clone());
        contract
            .message_enums
            .extend(artifact.message_enums.clone());
        contract.state_items.extend(artifact.state_items.clone());
        contract.functions.extend(artifact.functions.clone());

        ir.functions.extend(artifact.ir_functions.clone());
        for ep in &artifact.ir_entry_points {
            if !ir.entry_points.contains(ep) {
                ir.entry_points.push(ep.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_hash_contents() {
        let h1 = CacheManager::hash_contents("hello");
        let h2 = CacheManager::hash_contents("hello");
        let h3 = CacheManager::hash_contents("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 64); // SHA256 hex
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = std::env::temp_dir().join("cosmwasm-guard-test-cache");
        let _ = fs::remove_dir_all(&dir);

        let mut cache = CacheManager::open(dir.clone()).unwrap();

        let artifact = CachedFileArtifact {
            entry_points: vec![],
            message_enums: vec![],
            state_items: vec![],
            functions: vec![],
            ir_functions: vec![],
            ir_entry_points: vec!["execute".to_string()],
        };

        let file = PathBuf::from("src/lib.rs");
        let hash = CacheManager::hash_contents("test source code");

        cache.store(&file, &hash, &artifact).unwrap();
        cache.flush().unwrap();

        // Lookup should hit
        let hit = cache.lookup(&file, &hash);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().ir_entry_points, vec!["execute".to_string()]);

        // Different hash should miss
        let different = CacheManager::hash_contents("different source");
        let miss = cache.lookup(&file, &different);
        assert!(miss.is_none());

        // Clear should remove everything
        cache.clear().unwrap();
        let miss = cache.lookup(&file, &hash);
        assert!(miss.is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
