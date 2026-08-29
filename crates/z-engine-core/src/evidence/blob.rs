//! Content-addressed blob storage: bytes are named by their own SHA-256
//! digest, so identical content always resolves to the same
//! [`BlobHandle`] and is written to disk at most once.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::EvidenceError;

/// A validated, lowercase 64-character hex SHA-256 digest identifying one
/// piece of immutable content. The only way to build one is either
/// hashing bytes ([`BlobHandle::of`]) or parsing an already-hex string
/// ([`BlobHandle::parse`]), so a `BlobHandle` in memory is always
/// well-formed; malformed strings are only possible when deserializing
/// untrusted data (e.g. a hand-edited or corrupted ledger line), where
/// [`Deserialize`] rejects them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct BlobHandle(String);

impl BlobHandle {
    /// Hash `bytes` and return the resulting handle.
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(to_hex(&hasher.finalize()))
    }

    /// Validate `hex` as a 64-character hex SHA-256 digest.
    pub fn parse(hex: &str) -> Result<Self, EvidenceError> {
        let is_valid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        if is_valid {
            Ok(Self(hex.to_ascii_lowercase()))
        } else {
            Err(EvidenceError::MalformedHandle(hex.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlobHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for BlobHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        BlobHandle::parse(&raw).map_err(serde::de::Error::custom)
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Content-addressed storage for artifact bytes. Implementations must
/// make `put` idempotent for identical content and must detect on-disk
/// corruption in `get` rather than returning tampered bytes.
pub trait BlobStore {
    /// Store `bytes`, returning its content handle. Writing the same
    /// bytes twice is a no-op the second time.
    fn put(&self, bytes: &[u8]) -> Result<BlobHandle, EvidenceError>;

    /// Retrieve the bytes for `handle`, verifying they still hash to it.
    fn get(&self, handle: &BlobHandle) -> Result<Vec<u8>, EvidenceError>;
}

/// Filesystem-backed [`BlobStore`]: one file per blob, named by its hash,
/// under `root` (e.g. `.z-engine/runs/<run-id>/blobs`).
#[derive(Debug, Clone)]
pub struct FsBlobStore {
    root: PathBuf,
}

impl FsBlobStore {
    /// Open (creating if needed) a blob store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let _ = std::fs::create_dir_all(&root);
        Self { root }
    }

    fn path_for(&self, handle: &BlobHandle) -> PathBuf {
        self.root.join(handle.as_str())
    }
}

impl BlobStore for FsBlobStore {
    fn put(&self, bytes: &[u8]) -> Result<BlobHandle, EvidenceError> {
        let handle = BlobHandle::of(bytes);
        let path = self.path_for(&handle);
        if path.exists() {
            // Content-addressed: an existing file at this path already
            // holds these exact bytes, so there is nothing left to do.
            return Ok(handle);
        }
        atomic_write(&path, bytes).map_err(|source| EvidenceError::BlobWrite {
            handle: handle.clone(),
            path: path.clone(),
            source,
        })?;
        Ok(handle)
    }

    fn get(&self, handle: &BlobHandle) -> Result<Vec<u8>, EvidenceError> {
        let path = self.path_for(handle);
        let bytes = std::fs::read(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                EvidenceError::BlobMissing {
                    handle: handle.clone(),
                    path: path.clone(),
                }
            } else {
                EvidenceError::BlobRead {
                    handle: handle.clone(),
                    path: path.clone(),
                    source,
                }
            }
        })?;
        let actual = BlobHandle::of(&bytes);
        if actual != *handle {
            return Err(EvidenceError::HashMismatch {
                handle: handle.clone(),
                actual,
            });
        }
        Ok(bytes)
    }
}

/// Crash-safe file creation: write to a temp sibling, flush it, then
/// atomically rename over the target. Mirrors `tools::fsutil::atomic_write`
/// but stays synchronous and dependency-free of the tools module, since
/// evidence storage must not couple to tool/agent/UI layers.
fn atomic_write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| std::io::Error::other("target has no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "blob".into()),
        ulid::Ulid::new()
    ));
    let result = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    match std::fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_bytes_share_one_blob() {
        // `dir` must stay bound for the whole test: `tempdir().path()`
        // used inline would drop (and delete) the TempDir at the end of
        // the `let store = ...` statement, leaving `store` pointed at a
        // directory that no longer exists.
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path());
        assert_eq!(store.put(b"same").unwrap(), store.put(b"same").unwrap());
    }

    #[test]
    fn distinct_bytes_get_distinct_handles() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path());
        assert_ne!(store.put(b"a").unwrap(), store.put(b"b").unwrap());
    }

    #[test]
    fn put_then_get_roundtrips_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path());
        let handle = store.put(b"payload").unwrap();
        assert_eq!(store.get(&handle).unwrap(), b"payload");
    }

    #[test]
    fn get_of_unknown_handle_is_missing_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path());
        let handle = BlobHandle::of(b"never written");
        let err = store.get(&handle).unwrap_err();
        assert!(matches!(err, EvidenceError::BlobMissing { .. }));
    }

    #[test]
    fn get_detects_on_disk_corruption_via_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path());
        let handle = store.put(b"original").unwrap();
        // Simulate on-disk tampering/corruption after the write.
        std::fs::write(dir.path().join(handle.as_str()), b"tampered").unwrap();

        let err = store.get(&handle).unwrap_err();
        assert!(matches!(err, EvidenceError::HashMismatch { .. }));
    }

    #[test]
    fn parse_accepts_valid_hex_and_lowercases_it() {
        let hex = "A".repeat(64);
        let handle = BlobHandle::parse(&hex).unwrap();
        assert_eq!(handle.as_str(), "a".repeat(64));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!(matches!(
            BlobHandle::parse("abc"),
            Err(EvidenceError::MalformedHandle(_))
        ));
    }

    #[test]
    fn parse_rejects_non_hex_characters() {
        let bad = format!("{}zz", "a".repeat(62));
        assert!(matches!(
            BlobHandle::parse(&bad),
            Err(EvidenceError::MalformedHandle(_))
        ));
    }

    #[test]
    fn deserialize_rejects_malformed_handle_json() {
        let err = serde_json::from_str::<BlobHandle>("\"not-a-hash\"").unwrap_err();
        assert!(err.to_string().contains("malformed blob handle"));
    }
}
