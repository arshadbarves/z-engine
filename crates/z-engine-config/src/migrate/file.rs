//! In-place migration of one settings file: the original is copied to a
//! backup that is never overwritten, then the converted document replaces
//! the file atomically.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Table;
use ts_rs::TS;

use super::convert_v1;
use crate::document::{self, Schema};
use crate::error::ConfigError;
use crate::files::write_atomic;

const MAX_BACKUPS: usize = 100;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct MigrationOutcome {
    pub migrated: bool,
    /// Copy of the v1 file, e.g. `config.v1.toml` beside `config.toml`.
    pub backup: Option<PathBuf>,
    /// What changed meaning or was dropped.
    pub notes: Vec<String>,
}

/// Migrates `path` when it is a v1 file; a missing or current file is left
/// alone. A file that does not parse is an error and stays untouched.
pub fn migrate_file(path: &Path) -> Result<MigrationOutcome, ConfigError> {
    let _guard = document::lock();
    let Some(raw) = document::read(path)? else {
        return Ok(MigrationOutcome::default());
    };
    if document::schema_of(&raw.table) != Schema::V1 {
        return Ok(MigrationOutcome::default());
    }
    let (converted, notes) = convert_v1(raw.table);
    let backup = persist(path, &raw.text, &converted)?;
    Ok(MigrationOutcome {
        migrated: true,
        backup: Some(backup),
        notes,
    })
}

/// Backs up `original` and replaces `path` with `converted`. Callers hold
/// [`document::lock`].
pub(crate) fn persist(
    path: &Path,
    original: &str,
    converted: &Table,
) -> Result<PathBuf, ConfigError> {
    let backup = write_backup(path, original)?;
    write_atomic(path, document::render(converted)?.as_bytes(), false)?;
    Ok(backup)
}

/// `config.v1.toml`, else `config.v1.1.toml`, ...; an existing backup with
/// the same content is reused.
fn write_backup(path: &Path, original: &str) -> Result<PathBuf, ConfigError> {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string());
    for attempt in 0..MAX_BACKUPS {
        let name = match attempt {
            0 => format!("{stem}.v1.toml"),
            n => format!("{stem}.v1.{n}.toml"),
        };
        let candidate = path.with_file_name(name);
        match create_new(&candidate, original, path) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if fs::read_to_string(&candidate).is_ok_and(|existing| existing == original) {
                    return Ok(candidate);
                }
            }
            Err(error) => return Err(ConfigError::io(&candidate, error)),
        }
    }
    let error = io::Error::new(io::ErrorKind::AlreadyExists, "too many v1 backups");
    Err(ConfigError::io(path, error))
}

fn create_new(backup: &Path, text: &str, original: &Path) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup)?;
    let result = fill(&mut file, text, original);
    if result.is_err() {
        drop(file);
        if let Err(cleanup) = fs::remove_file(backup) {
            tracing::warn!(path = %backup.display(), error = %cleanup, "could not remove partial backup");
        }
    }
    result
}

fn fill(file: &mut fs::File, text: &str, original: &Path) -> io::Result<()> {
    if let Ok(metadata) = fs::metadata(original) {
        file.set_permissions(metadata.permissions())?;
    }
    file.write_all(text.as_bytes())?;
    file.sync_all()
}
