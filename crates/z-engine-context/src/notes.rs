use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelNote {
    pub kind: NoteKind,
    pub source: NoteSource,
    pub trust: NoteTrust,
    pub text: String,
}

impl ModelNote {
    pub fn new(kind: NoteKind, text: String) -> Self {
        Self {
            kind,
            source: if kind == NoteKind::Summary {
                NoteSource::ModelCompactionOrLegacyNote
            } else {
                NoteSource::ModelContextNotes
            },
            trust: NoteTrust::Unverified,
            text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Progress,
    Decision,
    NeedsLater,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteSource {
    ModelContextNotes,
    ModelCompactionOrLegacyNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteTrust {
    Unverified,
}
