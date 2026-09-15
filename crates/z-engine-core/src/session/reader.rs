use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use crate::verification::{TASK_REPORT_SCHEMA_VERSION, TaskStatus};

use super::SessionEvent;
use super::storage::{lock, path_state};

pub(super) struct ReadTranscript {
    pub events: Vec<SessionEvent>,
    pub torn_tail: bool,
    pub missing_newline: bool,
}

/// Read a legacy or versioned transcript. Only a JSON value truncated at EOF
/// is recoverable; corruption or an unsupported event/schema is an error.
pub fn read_events(path: &Path) -> io::Result<Vec<SessionEvent>> {
    let shared = path_state(path)?;
    let state = lock(&shared)?;
    state.ensure_healthy()?;
    Ok(read_transcript(path)?.events)
}

pub(super) fn read_transcript(path: &Path) -> io::Result<ReadTranscript> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut events = Vec::new();
    let mut line = Vec::new();
    let mut number = 0;
    let mut torn_tail = false;
    let mut missing_newline = false;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        number += 1;
        missing_newline = line.last() != Some(&b'\n');
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        // Parse syntax first: semantic EOF errors (e.g. a missing report field)
        // are corruption, not a known torn final write.
        let value: serde_json::Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(error) if missing_newline && error.is_eof() => {
                torn_tail = true;
                break;
            }
            Err(error) => return Err(invalid_line(number, error)),
        };
        let event: SessionEvent =
            serde_json::from_value(value).map_err(|error| invalid_line(number, error))?;
        if let SessionEvent::TaskUpdated { report } = &event {
            if report.schema_version != TASK_REPORT_SCHEMA_VERSION {
                return Err(invalid_line(
                    number,
                    format!("unsupported task report schema {}", report.schema_version),
                ));
            }
        }
        events.push(event);
    }
    if torn_tail {
        for event in &mut events {
            if let SessionEvent::TaskUpdated { report } = event {
                if matches!(
                    report.status,
                    TaskStatus::Complete | TaskStatus::Running | TaskStatus::NeedsVerification
                ) {
                    report.status = TaskStatus::Interrupted;
                    report.blockers.push("Session ended with a truncated record; later effects or task updates may be missing.".into());
                }
            }
        }
    }
    Ok(ReadTranscript {
        events,
        torn_tail,
        missing_newline,
    })
}

fn invalid_line(number: usize, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid session record on line {number}: {error}"),
    )
}
