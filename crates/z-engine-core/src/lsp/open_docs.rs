//! Which documents the *current* server connection has been told about.
//!
//! The distinction that matters is "this connection", not "this run". A
//! `didChange` is only meaningful to a server that received the matching
//! `didOpen`; send one to a freshly spawned rust-analyzer and it is
//! dropped, the document is never analyzed, and the mutation gate then
//! sees no symbols — blocking every Rust edit for the rest of the run
//! with a refusal no amount of re-reading can clear.
//!
//! So the registry is keyed by a connection *generation*. A respawn bumps
//! the generation, which makes every earlier belief stale in one step,
//! and the next edit re-opens the document instead of assuming it is
//! already known. Kept apart from the client so the rule can be tested
//! without a server, a process, or a socket.

use std::collections::HashSet;

/// What to send for a document the caller is about to synchronize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenAction {
    /// This connection has never seen the document.
    DidOpen,
    /// This connection opened it already; send the new text as a change.
    DidChange,
}

impl OpenAction {
    pub(super) fn method(self) -> &'static str {
        match self {
            Self::DidOpen => "textDocument/didOpen",
            Self::DidChange => "textDocument/didChange",
        }
    }
}

/// The documents one connection has been told about.
#[derive(Debug, Default)]
pub(super) struct OpenDocs {
    generation: u64,
    open: HashSet<String>,
}

impl OpenDocs {
    /// Decide what to send for `uri` on connection `generation`.
    ///
    /// Adopting a newer generation clears everything first: whatever the
    /// previous server was told died with it. Callers must hold this
    /// registry across the send (see [`OpenDocs::confirm`]), which is
    /// what serializes two concurrent opens of the same document into one
    /// `didOpen` followed by one `didChange`.
    pub(super) fn action_for(&mut self, uri: &str, generation: u64) -> OpenAction {
        if generation != self.generation {
            self.generation = generation;
            self.open.clear();
        }
        if self.open.contains(uri) {
            OpenAction::DidChange
        } else {
            OpenAction::DidOpen
        }
    }

    /// Record that the notification for `uri` reached the server.
    ///
    /// Only a *successful* send registers the document: a write that
    /// failed leaves the server without the `didOpen`, and remembering it
    /// anyway would send a `didChange` next time that nothing can apply.
    /// A send that raced a respawn is discarded for the same reason.
    pub(super) fn confirm(&mut self, uri: &str, generation: u64) {
        if generation == self.generation {
            self.open.insert(uri.to_string());
        }
    }

    #[cfg(test)]
    pub(super) fn known(&self) -> usize {
        self.open.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "file:///repo/src/lib.rs";
    const B: &str = "file:///repo/src/main.rs";

    #[test]
    fn the_first_sight_of_a_document_opens_it_and_later_ones_change_it() {
        let mut docs = OpenDocs::default();
        assert_eq!(docs.action_for(A, 1), OpenAction::DidOpen);
        docs.confirm(A, 1);
        assert_eq!(docs.action_for(A, 1), OpenAction::DidChange);
        // A second document is its own question.
        assert_eq!(docs.action_for(B, 1), OpenAction::DidOpen);
    }

    /// The finding this module exists for: after a respawn the server
    /// knows nothing, so the next edit must re-open rather than send a
    /// change the new server will silently drop.
    #[test]
    fn a_respawn_makes_every_document_unknown_again() {
        let mut docs = OpenDocs::default();
        docs.action_for(A, 1);
        docs.confirm(A, 1);
        docs.action_for(B, 1);
        docs.confirm(B, 1);
        assert_eq!(docs.known(), 2);

        assert_eq!(
            docs.action_for(A, 2),
            OpenAction::DidOpen,
            "a new connection has been told nothing"
        );
        assert_eq!(docs.known(), 0, "the old beliefs are dropped, not kept");
        docs.confirm(A, 2);
        assert_eq!(docs.action_for(B, 2), OpenAction::DidOpen);
    }

    /// A failed write must not leave the document looking open: the next
    /// attempt has to be the `didOpen` that never landed.
    #[test]
    fn an_unconfirmed_send_leaves_the_document_unopened() {
        let mut docs = OpenDocs::default();
        assert_eq!(docs.action_for(A, 1), OpenAction::DidOpen);
        // …no confirm: the write failed.
        assert_eq!(docs.action_for(A, 1), OpenAction::DidOpen);
    }

    /// A send that was decided against one connection and confirmed after
    /// a respawn describes a server that no longer exists.
    #[test]
    fn a_confirmation_from_a_dead_connection_is_discarded() {
        let mut docs = OpenDocs::default();
        docs.action_for(A, 1);
        docs.action_for(B, 2); // respawn observed while A was in flight
        docs.confirm(A, 1);
        assert_eq!(
            docs.action_for(A, 2),
            OpenAction::DidOpen,
            "the new server was never told about A"
        );
    }

    #[test]
    fn the_notification_names_match_the_protocol() {
        assert_eq!(OpenAction::DidOpen.method(), "textDocument/didOpen");
        assert_eq!(OpenAction::DidChange.method(), "textDocument/didChange");
    }
}
