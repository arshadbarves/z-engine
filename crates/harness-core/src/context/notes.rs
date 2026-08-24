//! L1 context notes (spec §6.1): the model's meta-output that survives all
//! compaction verbatim.
//!
//! The model reports progress/decisions/needs_later through the
//! `update_context_notes` pseudo-tool each turn. `droppable` entries are
//! honored eagerly: a `droppable` entry quoting a tool-output id
//! (`[harness:tool-output id=abcd1234]`) elides that transcript entry on
//! the next request, pressure or not.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContextNotes {
    #[serde(default)]
    pub progress: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub needs_later: Vec<String>,
    /// Summaries of demoted old turns (written by the compactor).
    #[serde(default, skip_deserializing)]
    pub summaries: Vec<String>,
}

#[derive(Debug, Default)]
pub struct NotesStore {
    notes: ContextNotes,
    /// tool-output ids explicitly marked droppable by the model.
    droppable_ids: BTreeSet<String>,
}

impl NotesStore {
    pub fn merge(&mut self, progress: &[String], decisions: &[String], needs_later: &[String]) {
        push_unique(&mut self.notes.progress, progress);
        push_unique(&mut self.notes.decisions, decisions);
        push_unique(&mut self.notes.needs_later, needs_later);
    }

    pub fn mark_droppable(&mut self, entries: &[String]) {
        for d in entries {
            if let Some(id) = extract_tool_output_id(d) {
                self.droppable_ids.insert(id.to_string());
            }
        }
    }

    pub fn get(&self) -> &ContextNotes {
        &self.notes
    }

    pub fn add_summary(&mut self, summary: String) {
        if !summary.trim().is_empty() {
            self.notes.summaries.push(summary.trim().to_string());
        }
    }

    pub fn droppable_ids(&self) -> &BTreeSet<String> {
        &self.droppable_ids
    }

    pub fn take_droppable_ids(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.droppable_ids)
    }

    pub fn is_empty(&self) -> bool {
        self.notes.progress.is_empty()
            && self.notes.decisions.is_empty()
            && self.notes.needs_later.is_empty()
            && self.notes.summaries.is_empty()
    }

    /// Render the L1 block injected after the system prompt.
    pub fn render_block(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut out =
            String::from("# Session context notes (authoritative; survives compaction)\n");
        for (title, items) in [
            ("Progress", &self.notes.progress),
            ("Decisions", &self.notes.decisions),
            ("Needs later", &self.notes.needs_later),
            ("Earlier-session summary", &self.notes.summaries),
        ] {
            if items.is_empty() {
                continue;
            }
            out.push_str(&format!("## {title}\n"));
            for it in items {
                out.push_str(&format!("- {it}\n"));
            }
        }
        Some(out)
    }
}

fn push_unique(dst: &mut Vec<String>, src: &[String]) {
    for s in src {
        let s = s.trim();
        if !s.is_empty() && !dst.iter().any(|d| d == s) {
            dst.push(s.to_string());
        }
    }
}

/// Recognize `[harness:tool-output id=xxxx]` references in droppable text.
pub fn extract_tool_output_id(text: &str) -> Option<&str> {
    const TAG: &str = "[harness:tool-output id=";
    let start = text.find(TAG)? + TAG.len();
    let rest = &text[start..];
    let end = rest.find(']')?;
    Some(&rest[..end])
}

#[derive(Debug, Deserialize)]
pub struct NotesInput {
    #[serde(default)]
    pub progress: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub needs_later: Vec<String>,
    #[serde(default)]
    pub droppable: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_dedupes_and_preserves_order() {
        let mut s = NotesStore::default();
        s.merge(
            &["rewrote lexer".to_string()],
            &["no regex".to_string()],
            &[],
        );
        s.merge(
            &["rewrote lexer".to_string(), "tests green".to_string()],
            &[],
            &["error enum shape".to_string()],
        );
        assert_eq!(s.get().progress, ["rewrote lexer", "tests green"]);
        assert_eq!(s.get().decisions, ["no regex"]);
        assert_eq!(s.get().needs_later.len(), 1);
    }

    #[test]
    fn renders_only_nonempty_sections() {
        let mut s = NotesStore::default();
        assert!(s.render_block().is_none());
        s.merge(&["p1".to_string()], &[], &[]);
        let block = s.render_block().unwrap();
        assert!(block.contains("## Progress\n- p1\n"));
        assert!(!block.contains("Decisions"));
    }

    #[test]
    fn summaries_append_and_render() {
        let mut s = NotesStore::default();
        s.add_summary("facts: uses tokio".into());
        s.add_summary(String::new()); // ignored
        assert_eq!(s.get().summaries.len(), 1);
        assert!(
            s.render_block()
                .unwrap()
                .contains("Earlier-session summary")
        );
    }

    #[test]
    fn droppable_id_extraction() {
        assert_eq!(
            extract_tool_output_id(
                "the grep output above [harness:tool-output id=ab12cd34] can go"
            ),
            Some("ab12cd34")
        );
        assert_eq!(extract_tool_output_id("no marker"), None);
    }
}
