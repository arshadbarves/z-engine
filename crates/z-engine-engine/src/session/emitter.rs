//! The one path from a session to the GUI: every event gets the next
//! per-session sequence number and is delivered in that order.

use std::fmt;
use std::sync::Mutex;

use z_engine_protocol::{Event, EventEnvelope, NoticeLevel, SessionId};

use crate::options::EventSink;
use crate::sync::lock;

pub(crate) struct Emitter {
    session_id: SessionId,
    sink: EventSink,
    /// Held while the sink runs so delivery order matches `seq`.
    seq: Mutex<u64>,
}

impl fmt::Debug for Emitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("session_id", &self.session_id)
            .field("seq", &*lock(&self.seq))
            .finish_non_exhaustive()
    }
}

impl Emitter {
    pub(crate) fn new(session_id: SessionId, sink: EventSink) -> Self {
        Self {
            session_id,
            sink,
            seq: Mutex::new(0),
        }
    }

    pub(crate) fn emit(&self, event: Event) {
        let mut seq = lock(&self.seq);
        *seq += 1;
        (self.sink)(EventEnvelope {
            session_id: self.session_id.clone(),
            seq: *seq,
            event,
        });
    }

    pub(crate) fn notice(&self, level: NoticeLevel, text: impl Into<String>) {
        self.emit(Event::notice(level, text));
    }

    pub(crate) fn error(&self, message: impl Into<String>) {
        self.emit(Event::Error {
            message: message.into(),
        });
    }
}
