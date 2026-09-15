use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::SessionEvent;
use super::reader::read_transcript;
use super::storage::{SharedPathState, event_line, lock, path_state, replace_events, sync_parent};

/// Append handle for one session. Failures are sticky across handles for its path.
#[derive(Debug)]
pub struct SessionWriter {
    file: File,
    pub path: PathBuf,
    shared: SharedPathState,
    generation: u64,
}

impl SessionWriter {
    #[cfg(test)]
    pub(crate) fn from_file_for_test(file: File, path: PathBuf) -> Self {
        let shared = path_state(&path).expect("test session path must be accessible");
        let generation = lock(&shared)
            .expect("test session lock must be healthy")
            .generation;
        Self {
            file,
            path,
            shared,
            generation,
        }
    }

    pub fn create(sessions_dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(sessions_dir)?;
        let path = sessions_dir.join(format!("{}.jsonl", ulid::Ulid::new()));
        Self::append_to(&path)
    }

    pub fn append_to(path: &Path) -> io::Result<Self> {
        let shared = path_state(path)?;
        let mut state = lock(&shared)?;
        state.ensure_healthy()?;
        let result = (|| {
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            if !file.metadata()?.is_file() {
                return Err(io::Error::other(
                    "session transcript must be a regular file",
                ));
            }
            let transcript = read_transcript(path)?;
            if transcript.torn_tail {
                // Recovery must persist the downgraded reports atomically: merely
                // dropping the tail would resurrect the old completed evidence.
                replace_events(path, &transcript.events, &mut state)?;
                file = OpenOptions::new().append(true).open(path)?;
            } else if transcript.missing_newline {
                file.write_all(b"\n")?;
                file.flush()?;
                file.sync_all()?;
            }
            Ok(file)
        })();
        let file = state.remember(result)?;
        let generation = state.generation;
        drop(state);
        Ok(Self {
            file,
            path: path.to_path_buf(),
            shared,
            generation,
        })
    }

    /// Append one serialized, flushed JSONL line under the shared path lock.
    pub fn record(&mut self, event: &SessionEvent) -> io::Result<()> {
        self.write_event(event, false)
    }

    /// Acknowledge only after flush and sync, including the parent on Unix.
    pub fn record_durable(&mut self, event: &SessionEvent) -> io::Result<()> {
        self.write_event(event, true)
    }

    fn write_event(&mut self, event: &SessionEvent, durable: bool) -> io::Result<()> {
        let mut state = lock(&self.shared)?;
        state.ensure_healthy()?;
        let result = (|| {
            if self.generation != state.generation {
                self.file = OpenOptions::new().append(true).open(&self.path)?;
                self.generation = state.generation;
            }
            Self::ensure_current_file(&self.file, &self.path)?;
            self.file.write_all(&event_line(event)?)?;
            self.file.flush()?;
            if durable {
                self.file.sync_all()?;
                sync_parent(&self.path)?;
            }
            Ok(())
        })();
        state.remember(result)
    }

    fn ensure_current_file(file: &File, path: &Path) -> io::Result<()> {
        let current = std::fs::metadata(path)?;
        if !current.is_file() {
            return Err(io::Error::other(
                "session transcript must be a regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let opened = file.metadata()?;
            if current.dev() != opened.dev() || current.ino() != opened.ino() {
                return Err(io::Error::other(
                    "session transcript was replaced outside the session store",
                ));
            }
        }
        #[cfg(not(unix))]
        let _ = file;
        Ok(())
    }

    /// Reopening a replaced file never clears a prior persistence failure.
    pub fn reopen(&mut self) -> io::Result<()> {
        let mut state = lock(&self.shared)?;
        state.ensure_healthy()?;
        let file = state.remember(OpenOptions::new().append(true).open(&self.path))?;
        self.file = file;
        self.generation = state.generation;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_normal_record_blocks_durable_ack_across_handles() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let writer = SessionWriter::create(dir.path()).unwrap();
        let mut other = SessionWriter::append_to(&writer.path).unwrap();
        let mut writer = SessionWriter::from_file_for_test(
            File::open(&writer.path).unwrap(),
            writer.path.clone(),
        );
        let event = SessionEvent::Ack;
        assert!(writer.record(&event).is_err());
        assert!(writer.record_durable(&event).is_err());
        assert!(other.record_durable(&event).is_err());
        assert!(writer.reopen().is_err());
        assert!(SessionWriter::append_to(&writer.path).is_err());
        assert!(super::super::read_events(&writer.path).is_err());
    }

    #[test]
    fn concurrent_append_handles_do_not_interleave_records() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let writer = SessionWriter::create(dir.path()).unwrap();
        let path = writer.path.clone();
        let threads: Vec<_> = (0..4)
            .map(|thread| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut writer = SessionWriter::append_to(&path).unwrap();
                    for index in 0..30 {
                        writer
                            .record(&SessionEvent::Note {
                                text: format!("{thread}:{index}:{}", "x".repeat(8192)),
                            })
                            .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(super::super::read_events(&path).unwrap().len(), 120);
    }

    #[test]
    fn removed_transcript_cannot_acknowledge_durable_event() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let mut writer = SessionWriter::create(dir.path()).unwrap();
        std::fs::remove_file(&writer.path).unwrap();
        assert!(writer.record_durable(&SessionEvent::Ack).is_err());
        assert!(writer.record(&SessionEvent::Ack).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn alias_append_handle_shares_sticky_failure() {
        let dir = tempfile::tempdir_in(".").unwrap();
        let mut writer = SessionWriter::create(dir.path()).unwrap();
        let alias = dir.path().join("alias.jsonl");
        std::os::unix::fs::symlink(std::fs::canonicalize(&writer.path).unwrap(), &alias).unwrap();
        let mut other = SessionWriter::append_to(&alias).unwrap();
        other.file = File::open(&writer.path).unwrap();
        assert!(other.record(&SessionEvent::Ack).is_err());
        assert!(writer.record_durable(&SessionEvent::Ack).is_err());
    }
}
