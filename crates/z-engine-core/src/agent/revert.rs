//! Rewind command handlers: restore checkpointed files and report what
//! happened as a status note.

use std::path::Path;
use std::sync::atomic::Ordering;

use tokio::sync::mpsc::UnboundedSender;

use super::events::Event;
use crate::tools::ToolCtx;
use z_engine_provider::ChatMessage;

pub(super) fn revert_last_turn(ctx: &ToolCtx, root: &Path, ev_tx: &UnboundedSender<Event>) {
    let out = ctx.checkpoints.revert_last_turn();
    ctx.repo_map_dirty.store(true, Ordering::Relaxed);
    let names: Vec<String> = out
        .restored
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let note = if out.restored.is_empty() && out.errors.is_empty() {
        "rewind: nothing to revert".to_string()
    } else {
        let mut s = format!("rewound {} file(s)", out.restored.len());
        if !names.is_empty() {
            let shown: Vec<String> = names.iter().take(3).cloned().collect();
            s.push_str(": ");
            s.push_str(&shown.join(", "));
            if names.len() > 3 {
                s.push_str(&format!(" +{}", names.len() - 3));
            }
        }
        if !out.errors.is_empty() {
            s.push_str(&format!(" ({} failed)", out.errors.len()));
        }
        s
    };
    for e in &out.errors {
        tracing::warn!(error = %e, "revert restore failed");
    }
    let _ = ev_tx.send(Event::StatusNote(note));
}

pub(super) fn revert_to_turn(
    ctx: &ToolCtx,
    root: &Path,
    keep: u64,
    ev_tx: &UnboundedSender<Event>,
) {
    let out = ctx.checkpoints.revert_to_turn(keep);
    ctx.repo_map_dirty.store(true, Ordering::Relaxed);
    let names: Vec<String> = out
        .restored
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let note = if out.restored.is_empty() && out.errors.is_empty() {
        format!(
            "rewind: no file changes recorded at or after turn {keep} \
             (checkpoints do not survive an app restart)"
        )
    } else {
        let mut s = format!(
            "rewound {} file(s) to before turn {keep}",
            out.restored.len()
        );
        if out.evicted_gaps {
            s.push_str(" (warning: some older checkpoints were evicted and cannot be restored)");
        }
        if !names.is_empty() {
            let shown: Vec<String> = names.iter().take(3).cloned().collect();
            s.push_str(": ");
            s.push_str(&shown.join(", "));
            if names.len() > 3 {
                s.push_str(&format!(" +{}", names.len() - 3));
            }
        }
        if !out.errors.is_empty() {
            s.push_str(&format!(" ({} failed)", out.errors.len()));
        }
        s
    };
    for e in &out.errors {
        tracing::warn!(error = %e, "revert-to-turn restore failed");
    }
    let _ = ev_tx.send(Event::StatusNote(note));
}

/// Drop the `keep`-th user message (0-based) and everything after it
/// from the in-memory working set.
pub(super) fn trim_working_before_user_turn(working: &mut Vec<ChatMessage>, keep: u64) {
    let mut seen = 0u64;
    let mut cut = working.len();
    for (i, m) in working.iter().enumerate() {
        if matches!(m, ChatMessage::User { .. } | ChatMessage::UserMulti { .. }) {
            if seen == keep {
                cut = i;
                break;
            }
            seen += 1;
        }
    }
    working.truncate(cut);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_working_drops_from_kept_user_onward() {
        let mut working = vec![
            ChatMessage::user("one"),
            ChatMessage::Assistant {
                content: Some("a".into()),
                tool_calls: vec![],
            },
            ChatMessage::user("two"),
            ChatMessage::Assistant {
                content: Some("b".into()),
                tool_calls: vec![],
            },
        ];
        trim_working_before_user_turn(&mut working, 1);
        assert_eq!(working.len(), 2);
        assert!(matches!(&working[0], ChatMessage::User { content } if content == "one"));
    }

    #[test]
    fn trim_working_keep_zero_clears_all() {
        let mut working = vec![ChatMessage::user("one")];
        trim_working_before_user_turn(&mut working, 0);
        assert!(working.is_empty());
    }
}
