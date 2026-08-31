use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::options::ScanCacheOptions;
use super::size::{apparent_size, disk_usage};

const CACHE_VERSION: u32 = 1;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScanCacheStats {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileSignature {
    device: u64,
    inode: u64,
    length: u64,
    blocks: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSignature {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            blocks: metadata.blocks(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedFile {
    path: PathBuf,
    signature: FileSignature,
    apparent_size: u64,
    disk_usage: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    entries: Vec<CachedFile>,
}

struct CacheState {
    path: PathBuf,
    previous: HashMap<PathBuf, CachedFile>,
    current: HashMap<PathBuf, CachedFile>,
    stats: ScanCacheStats,
}

/// Thread-safe cache session shared by parallel scanner workers.
#[derive(Clone, Default)]
pub(crate) struct MetadataCache(Option<Arc<Mutex<CacheState>>>);

impl MetadataCache {
    pub(crate) fn load(options: Option<&ScanCacheOptions>) -> Self {
        let Some(options) = options else {
            return Self::default();
        };

        let fresh = fs::metadata(&options.path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age <= options.ttl);

        let previous = if fresh {
            fs::read(&options.path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<CacheFile>(&bytes).ok())
                .filter(|cache| cache.version == CACHE_VERSION)
                .map(|cache| {
                    cache
                        .entries
                        .into_iter()
                        .map(|entry| (entry.path.clone(), entry))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        Self(Some(Arc::new(Mutex::new(CacheState {
            path: options.path.clone(),
            previous,
            current: HashMap::new(),
            stats: ScanCacheStats::default(),
        }))))
    }

    pub(crate) fn sizes(&self, path: &Path, metadata: &fs::Metadata) -> (u64, u64) {
        let Some(state) = &self.0 else {
            return (apparent_size(metadata), disk_usage(metadata));
        };
        let signature = FileSignature::from_metadata(metadata);
        let mut state = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let cached_sizes = state
            .previous
            .get(path)
            .filter(|cached| cached.signature == signature)
            .map(|cached| (cached.apparent_size, cached.disk_usage));
        let (apparent, disk) = match cached_sizes {
            Some(sizes) => {
                state.stats.hits += 1;
                sizes
            }
            _ => {
                state.stats.misses += 1;
                (apparent_size(metadata), disk_usage(metadata))
            }
        };
        state.current.insert(
            path.to_path_buf(),
            CachedFile {
                path: path.to_path_buf(),
                signature,
                apparent_size: apparent,
                disk_usage: disk,
            },
        );
        (apparent, disk)
    }

    pub(crate) fn stats(&self) -> ScanCacheStats {
        self.0
            .as_ref()
            .map(|state| {
                state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .stats
            })
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) {
        let Some(state) = &self.0 else {
            return;
        };
        let (path, entries) = {
            let state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                state.path.clone(),
                state.current.values().cloned().collect::<Vec<_>>(),
            )
        };
        let cache = CacheFile {
            version: CACHE_VERSION,
            entries,
        };
        let Ok(bytes) = serde_json::to_vec(&cache) else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }

        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".scan-metadata.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        if fs::write(&temporary, bytes).is_ok() && fs::rename(&temporary, &path).is_err() {
            let _ = fs::remove_file(temporary);
        }
    }
}
