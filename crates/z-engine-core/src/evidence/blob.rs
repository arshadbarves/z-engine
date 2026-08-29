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
    /// Open a blob store rooted at `root`, creating the directory if
    /// needed. Construction fails closed: if `root` cannot be created
    /// (e.g. a file already occupies that path, or permissions deny it),
    /// this returns a typed [`EvidenceError::Init`] instead of silently
    /// producing a store that will only fail later on first use.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, EvidenceError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|source| EvidenceError::Init {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
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
            // Content-addressed: a file already at this path is *expected*
            // to hold these exact bytes, but that must be verified rather
            // than assumed — an on-disk blob can be corrupted or tampered
            // with independently of this store. Re-hash it before trusting
            // it as a match.
            let existing = std::fs::read(&path).map_err(|source| EvidenceError::BlobRead {
                handle: handle.clone(),
                path: path.clone(),
                source,
            })?;
            let actual = BlobHandle::of(&existing);
            if actual != handle {
                return Err(EvidenceError::HashMismatch {
                    handle: handle.clone(),
                    actual,
                });
            }
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
///
/// `FsBlobStore::new` already validated/created `root` once at
/// construction, so the common case here does not redundantly call
/// `create_dir_all` on every write. If the directory has since
/// disappeared (e.g. deleted out-of-band between construction and this
/// call), creating the temp file fails with `NotFound`; only then is the
/// directory (re)created and the write retried once, preserving
/// correctness without paying the extra syscall on the hot path.
fn atomic_write(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| std::io::Error::other("target has no parent directory"))?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "blob".into()),
        ulid::Ulid::new()
    ));
    let mut file = match std::fs::File::create(&tmp) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(dir)?;
            std::fs::File::create(&tmp)?
        }
        Err(e) => return Err(e),
    };
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()
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
        let store = FsBlobStore::new(dir.path()).unwrap();
        assert_eq!(store.put(b"same").unwrap(), store.put(b"same").unwrap());
    }

    #[test]
    fn distinct_bytes_get_distinct_handles() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        assert_ne!(store.put(b"a").unwrap(), store.put(b"b").unwrap());
    }

    #[test]
    fn put_then_get_roundtrips_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        let handle = store.put(b"payload").unwrap();
        assert_eq!(store.get(&handle).unwrap(), b"payload");
    }

    #[test]
    fn get_of_unknown_handle_is_missing_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        let handle = BlobHandle::of(b"never written");
        let err = store.get(&handle).unwrap_err();
        assert!(matches!(err, EvidenceError::BlobMissing { .. }));
    }

    #[test]
    fn get_detects_on_disk_corruption_via_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        let handle = store.put(b"original").unwrap();
        // Simulate on-disk tampering/corruption after the write.
        std::fs::write(dir.path().join(handle.as_str()), b"tampered").unwrap();

        let err = store.get(&handle).unwrap_err();
        assert!(matches!(err, EvidenceError::HashMismatch { .. }));
    }

    #[test]
    fn put_detects_pre_corrupted_existing_blob() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        let handle = store.put(b"original").unwrap();
        // Tamper with the on-disk blob out-of-band, then ask the store to
        // `put` the same original bytes again. Trusting the existing file
        // just because it exists at the content-addressed path would
        // silently paper over the corruption; `put` must re-hash and fail.
        std::fs::write(dir.path().join(handle.as_str()), b"tampered").unwrap();

        let err = store.put(b"original").unwrap_err();
        assert!(matches!(err, EvidenceError::HashMismatch { .. }));
    }

    #[test]
    fn new_fails_closed_when_root_cannot_be_created() {
        let dir = tempfile::tempdir().unwrap();
        // Occupy the intended root path with a plain file, so creating a
        // directory there is guaranteed to fail.
        let blocked_root = dir.path().join("blocked-root");
        std::fs::write(&blocked_root, b"not a directory").unwrap();

        let err = FsBlobStore::new(&blocked_root).unwrap_err();
        assert!(matches!(err, EvidenceError::Init { .. }));
    }

    #[test]
    fn put_recreates_root_directory_if_it_disappears_after_construction() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsBlobStore::new(dir.path()).unwrap();
        // Simulate the root directory vanishing after construction (e.g.
        // deleted out-of-band). `put` must self-heal rather than assume
        // the directory validated at construction still exists.
        std::fs::remove_dir_all(dir.path()).unwrap();

        let handle = store.put(b"payload").unwrap();
        assert_eq!(store.get(&handle).unwrap(), b"payload");
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
