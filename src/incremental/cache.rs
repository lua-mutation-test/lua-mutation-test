//! Persistent cache for incremental mutation testing.

use crate::config::Config;
use crate::mutant::Mutant;
use crate::result::MutantResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const CACHE_VERSION: &str = "1";
const CACHE_DIR: &str = ".lua-mutation-test/cache";
const CACHE_FILE: &str = "mutation-cache.json";

/// On-disk cache format for incremental mutation runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    /// Schema version of the cache file.
    pub version: String,
    /// Hash of the configuration that produced the cached entries.
    pub config_hash: String,
    /// Git HEAD commit at the time the cache was written, if available.
    pub git_head: Option<String>,
    /// SHA-256 hashes of source files at the time the cache was written.
    pub file_hashes: HashMap<String, String>,
    /// Cached mutant results keyed by composite cache key.
    pub entries: HashMap<String, CacheEntry>,
}

impl Default for CacheFile {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION.to_string(),
            config_hash: String::new(),
            git_head: None,
            file_hashes: HashMap::new(),
            entries: HashMap::new(),
        }
    }
}

impl CacheFile {
    /// Loads the cache from disk, returning an empty cache if it does not exist
    /// or cannot be read.
    pub fn load(project_root: &Path) -> Self {
        let path = cache_path(project_root);
        if !path.exists() {
            return Self::default();
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        match serde_json::from_str::<CacheFile>(&contents) {
            Ok(cache) if cache.version == CACHE_VERSION => cache,
            _ => Self::default(),
        }
    }

    /// Writes the cache to disk atomically.
    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let cache_dir = project_root.join(CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        let path = cache_path(project_root);
        let tmp = cache_dir.join("mutation-cache.json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Computes a stable composite cache key.
    pub fn key(file: &Path, mutant_id: &str, config_hash: &str) -> String {
        format!("{}#{}#{}", file.to_string_lossy(), mutant_id, config_hash)
    }
}

/// A single cached mutant result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Source file path for the mutant.
    pub file: String,
    /// Stable mutant identifier.
    pub mutant_id: String,
    /// Configuration hash that produced this result.
    pub config_hash: String,
    /// Result category: killed, survived, timed_out, or error.
    pub result_category: String,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Cached stdout snippet.
    pub stdout_snippet: String,
    /// Cached stderr snippet.
    pub stderr_snippet: String,
    /// Error reason when `result_category` is `error`.
    pub reason: Option<String>,
}

impl CacheEntry {
    /// Builds a cache entry from a mutant result.
    pub fn from_result(result: &MutantResult, config_hash: &str) -> Self {
        let mutant = result.mutant();
        let (result_category, duration_ms, stdout_snippet, stderr_snippet, reason) = match result {
            MutantResult::Killed {
                duration_ms,
                stdout_snippet,
                stderr_snippet,
                ..
            } => (
                "killed".to_string(),
                *duration_ms,
                stdout_snippet.clone(),
                stderr_snippet.clone(),
                None,
            ),
            MutantResult::Survived {
                duration_ms,
                stdout_snippet,
                stderr_snippet,
                ..
            } => (
                "survived".to_string(),
                *duration_ms,
                stdout_snippet.clone(),
                stderr_snippet.clone(),
                None,
            ),
            MutantResult::TimedOut {
                duration_ms,
                stdout_snippet,
                stderr_snippet,
                ..
            } => (
                "timed_out".to_string(),
                *duration_ms,
                stdout_snippet.clone(),
                stderr_snippet.clone(),
                None,
            ),
            MutantResult::Error {
                duration_ms,
                reason,
                stdout_snippet,
                stderr_snippet,
                ..
            } => (
                "error".to_string(),
                *duration_ms,
                stdout_snippet.clone(),
                stderr_snippet.clone(),
                Some(reason.clone()),
            ),
            MutantResult::Equivalent { reason, .. } => (
                "equivalent".to_string(),
                0,
                String::new(),
                String::new(),
                Some(reason.clone()),
            ),
        };

        Self {
            file: mutant.file.to_string_lossy().to_string(),
            mutant_id: mutant.id.clone(),
            config_hash: config_hash.to_string(),
            result_category,
            duration_ms,
            stdout_snippet,
            stderr_snippet,
            reason,
        }
    }
}

/// Reconstructs a `MutantResult` from a cache entry.
pub fn result_from_entry(mutant: Mutant, entry: &CacheEntry) -> MutantResult {
    match entry.result_category.as_str() {
        "killed" => MutantResult::Killed {
            mutant,
            duration_ms: entry.duration_ms,
            stdout_snippet: entry.stdout_snippet.clone(),
            stderr_snippet: entry.stderr_snippet.clone(),
        },
        "survived" => MutantResult::Survived {
            mutant,
            duration_ms: entry.duration_ms,
            stdout_snippet: entry.stdout_snippet.clone(),
            stderr_snippet: entry.stderr_snippet.clone(),
        },
        "timed_out" => MutantResult::TimedOut {
            mutant,
            duration_ms: entry.duration_ms,
            stdout_snippet: entry.stdout_snippet.clone(),
            stderr_snippet: entry.stderr_snippet.clone(),
        },
        "equivalent" => MutantResult::Equivalent {
            mutant,
            reason: entry
                .reason
                .clone()
                .unwrap_or_else(|| "likely equivalent".to_string()),
        },
        _ => MutantResult::Error {
            mutant,
            duration_ms: entry.duration_ms,
            reason: entry
                .reason
                .clone()
                .unwrap_or_else(|| "unknown cached category".to_string()),
            stdout_snippet: entry.stdout_snippet.clone(),
            stderr_snippet: entry.stderr_snippet.clone(),
        },
    }
}

/// Returns the path to the cache file.
pub fn cache_path(project_root: &Path) -> PathBuf {
    project_root.join(CACHE_DIR).join(CACHE_FILE)
}

/// Computes a stable SHA-256 hash of the effective configuration.
pub fn config_hash(config: &Config) -> String {
    let json = serde_json::to_string(config).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    format!("{:x}", hash)
}

/// Computes the SHA-256 hash of a file's contents.
pub fn file_hash(path: &Path) -> Result<String, String> {
    let contents = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = Sha256::digest(&contents);
    Ok(format!("{:x}", hash))
}

/// Computes SHA-256 hashes for a set of source files.
pub fn compute_file_hashes(paths: &[PathBuf]) -> HashMap<String, String> {
    let mut hashes = HashMap::new();
    for path in paths {
        if let Ok(hash) = file_hash(path) {
            hashes.insert(path.to_string_lossy().to_string(), hash);
        }
    }
    hashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::{CandidateMutant, Mutant};
    use std::path::PathBuf;

    fn dummy_result(category: &str) -> (MutantResult, Mutant) {
        let mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            PathBuf::from("src/foo.lua"),
            "local x = 1",
        );
        let result = match category {
            "killed" => MutantResult::Killed {
                mutant: mutant.clone(),
                duration_ms: 10,
                stdout_snippet: "out".to_string(),
                stderr_snippet: "err".to_string(),
            },
            "survived" => MutantResult::Survived {
                mutant: mutant.clone(),
                duration_ms: 10,
                stdout_snippet: "out".to_string(),
                stderr_snippet: "err".to_string(),
            },
            "timed_out" => MutantResult::TimedOut {
                mutant: mutant.clone(),
                duration_ms: 10,
                stdout_snippet: "out".to_string(),
                stderr_snippet: "err".to_string(),
            },
            "equivalent" => MutantResult::Equivalent {
                mutant: mutant.clone(),
                reason: "likely equivalent".to_string(),
            },
            _ => MutantResult::Error {
                mutant: mutant.clone(),
                duration_ms: 0,
                reason: "boom".to_string(),
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
        };
        (result, mutant)
    }

    #[test]
    fn cache_entry_round_trips_result() {
        let (result, _) = dummy_result("killed");
        let entry = CacheEntry::from_result(&result, "cfg");
        assert_eq!(entry.result_category, "killed");
        assert_eq!(entry.stdout_snippet, "out");
        assert_eq!(entry.stderr_snippet, "err");
    }

    #[test]
    fn reconstructs_killed_result() {
        let (result, mutant) = dummy_result("killed");
        let entry = CacheEntry::from_result(&result, "cfg");
        let reconstructed = result_from_entry(mutant, &entry);
        assert!(matches!(reconstructed, MutantResult::Killed { .. }));
    }

    #[test]
    fn roundtrip_survived_timed_out_equivalent() {
        for category in ["survived", "timed_out", "equivalent"] {
            let (result, mutant) = dummy_result(category);
            assert_eq!(result.category(), category);
            let entry = CacheEntry::from_result(&result, "cfg");
            assert_eq!(entry.result_category, category);
            let restored = result_from_entry(mutant, &entry);
            assert_eq!(restored.category(), category);
            match category {
                "survived" => {
                    assert!(matches!(restored, MutantResult::Survived { .. }));
                    if let MutantResult::Survived {
                        duration_ms,
                        stdout_snippet,
                        stderr_snippet,
                        ..
                    } = restored
                    {
                        assert_eq!(duration_ms, 10);
                        assert_eq!(stdout_snippet, "out");
                        assert_eq!(stderr_snippet, "err");
                    }
                }
                "timed_out" => {
                    assert!(matches!(restored, MutantResult::TimedOut { .. }));
                    if let MutantResult::TimedOut {
                        duration_ms,
                        stdout_snippet,
                        stderr_snippet,
                        ..
                    } = restored
                    {
                        assert_eq!(duration_ms, 10);
                        assert_eq!(stdout_snippet, "out");
                        assert_eq!(stderr_snippet, "err");
                    }
                }
                "equivalent" => {
                    assert!(matches!(restored, MutantResult::Equivalent { .. }));
                    if let MutantResult::Equivalent { reason, .. } = restored {
                        assert_eq!(reason, "likely equivalent");
                    }
                }
                _ => unreachable!(),
            }
        }

        // Exercise the real save -> load file path so serialization of every
        // status is covered, not just the in-memory entry conversion.
        let dir = tempfile::tempdir().unwrap();
        let mut cache = CacheFile::default();
        for category in ["survived", "timed_out", "equivalent"] {
            let (result, _) = dummy_result(category);
            let entry = CacheEntry::from_result(&result, "cfg");
            let key = CacheFile::key(Path::new("src/foo.lua"), category, "cfg");
            cache.entries.insert(key, entry);
        }
        cache.save(dir.path()).unwrap();
        let loaded = CacheFile::load(dir.path());
        assert_eq!(loaded.entries.len(), 3);
        for category in ["survived", "timed_out", "equivalent"] {
            let key = CacheFile::key(Path::new("src/foo.lua"), category, "cfg");
            let entry = loaded
                .entries
                .get(&key)
                .expect("entry must survive save/load");
            assert_eq!(entry.result_category, category);
            let (_, mutant) = dummy_result(category);
            let restored = result_from_entry(mutant, entry);
            assert_eq!(restored.category(), category);
        }
    }

    #[test]
    fn stale_version_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let (result, _) = dummy_result("survived");
        let entry = CacheEntry::from_result(&result, "cfg");
        let mut cache = CacheFile::default();
        cache
            .entries
            .insert("src/foo.lua#dummy#cfg".to_string(), entry);
        cache.save(dir.path()).unwrap();

        // Rewrite the file with a wrong schema version; load must not trust it.
        let path = cache_path(dir.path());
        let raw = std::fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        value["version"] = serde_json::Value::String("stale-version".to_string());
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let loaded = CacheFile::load(dir.path());
        assert!(
            loaded.entries.is_empty(),
            "stale cache version must be ignored"
        );
    }

    #[test]
    fn cache_key_includes_file_and_mutant_id() {
        let key = CacheFile::key(Path::new("src/a.lua"), "abc", "cfg");
        assert!(key.contains("src/a.lua"));
        assert!(key.contains("abc"));
        assert!(key.contains("cfg"));
    }

    #[test]
    fn file_hash_is_stable() {
        let tmp = std::env::temp_dir().join(format!("lmt-hash-test-{}", std::process::id()));
        std::fs::write(&tmp, "hello").unwrap();
        let first = file_hash(&tmp).unwrap();
        let second = file_hash(&tmp).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn config_hash_changes_with_config() {
        let cfg1 = Config::default();
        let cfg2 = Config {
            timeout: Some(42),
            ..Default::default()
        };
        assert_ne!(config_hash(&cfg1), config_hash(&cfg2));
    }
}
