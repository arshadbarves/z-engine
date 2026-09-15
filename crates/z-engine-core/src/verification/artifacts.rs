use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{EvidenceArtifact, VerificationError};

pub(super) const MAX_ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn digest_file(path: &Path, limit: u64) -> Result<(String, u64), VerificationError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| VerificationError::io(path, error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(VerificationError::Unsupported(format!(
            "not a regular, non-symlink file: {}",
            path.display()
        )));
    }
    let mut file = std::fs::File::open(path).map_err(|e| VerificationError::io(path, e))?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16384];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| VerificationError::io(path, e))?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > limit {
            return Err(VerificationError::ScanLimit(path.display().to_string()));
        }
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

pub(super) fn validate_artifact(artifact: &EvidenceArtifact) -> Result<(), VerificationError> {
    let path = Path::new(&artifact.path);
    if !path.is_absolute()
        || path
            .canonicalize()
            .map_err(|e| VerificationError::io(path, e))?
            != path
    {
        return Err(VerificationError::InvalidInput(
            "evidence path must be canonical and absolute".into(),
        ));
    }
    if digest_file(path, MAX_ARTIFACT_BYTES)?.0 != artifact.digest {
        return Err(VerificationError::InvalidInput(format!(
            "artifact checksum mismatch: {}",
            path.display()
        )));
    }
    Ok(())
}
