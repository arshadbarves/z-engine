//! Shared serialization and sticky failures for every in-process path writer.

use std::collections::HashMap;
#[cfg(unix)]
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use super::SessionEvent;

#[derive(Debug, Default)]
pub(super) struct PathState {
    pub generation: u64,
    failure: Option<(io::ErrorKind, String)>,
}

impl PathState {
    pub fn ensure_healthy(&self) -> io::Result<()> {
        match &self.failure {
            Some((kind, message)) => Err(io::Error::new(*kind, message.clone())),
            None => Ok(()),
        }
    }

    pub fn remember<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        if let Err(error) = &result {
            self.failure.get_or_insert_with(|| {
                (error.kind(), format!("session persistence failed: {error}"))
            });
        }
        result
    }
}

pub(super) type SharedPathState = Arc<Mutex<PathState>>;

pub(super) fn path_state(path: &Path) -> io::Result<SharedPathState> {
    static PATHS: OnceLock<Mutex<HashMap<PathBuf, SharedPathState>>> = OnceLock::new();
    let key = match std::fs::canonicalize(path) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            std::fs::canonicalize(parent)?.join(
                path.file_name()
                    .ok_or_else(|| io::Error::other("session path has no filename"))?,
            )
        }
        Err(error) => return Err(error),
    };
    let mut paths = lock(PATHS.get_or_init(Mutex::default))?;
    Ok(paths.entry(key).or_default().clone())
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> io::Result<MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| io::Error::other("session storage lock poisoned"))
}

pub(super) fn event_line(event: &SessionEvent) -> io::Result<Vec<u8>> {
    let mut line = serde_json::to_vec(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    line.push(b'\n');
    Ok(line)
}

pub(super) fn sync_parent(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        File::open(parent)?.sync_all()
    }
    // Directory handles cannot be opened/flushed portably on Windows.
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Caller holds the path lock; all append handles track `generation`.
pub(super) fn replace_events(
    path: &Path,
    events: &[SessionEvent],
    state: &mut PathState,
) -> io::Result<()> {
    let path = std::fs::canonicalize(path)?;
    let replacement = path.with_extension(format!("rewrite-{}", ulid::Ulid::new()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&replacement)?;
        for event in events {
            file.write_all(&event_line(event)?)?;
        }
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&replacement, &path)?;
        state.generation += 1;
        sync_parent(&path)
    })();
    if result.is_err() {
        if let Err(error) = std::fs::remove_file(&replacement) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(%error, "failed to remove session replacement");
            }
        }
    }
    state.remember(result)
}
