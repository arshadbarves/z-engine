//! Document symbols: the semantic answer a mutation gate can rely on.
//!
//! A tree-sitter outline says what the *text* looks like; rust-analyzer's
//! `textDocument/documentSymbol` says what the *compiler's* view of this
//! file contains. Only the latter can authorize a change to a symbol, so
//! this module keeps three answers apart and never collapses them:
//! resolved, not-indexed, and answered-about-something-else. The last two
//! are refusals, not empty successes.

use std::path::Path;

use serde_json::{Value, json};

use super::{LspClient, percent_encode_path};

/// What the semantic provider could say about one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolAnswer {
    /// The server answered for this document; these are its declarations
    /// (flattened, nested items included).
    Resolved(Vec<String>),
    /// The server is reachable but has nothing for this file: not indexed
    /// yet, outside the workspace, request failed, or an empty answer.
    Unindexed(String),
    /// The server answered about a different document, or in a shape this
    /// client cannot verify. Never trusted.
    Mismatched(String),
}

impl LspClient {
    /// Ask the server which symbols `text` declares in `abs_path`.
    ///
    /// `text` is the exact image being judged (the pre-edit bytes), pushed
    /// with `didOpen`/`didChange` first so the answer describes those
    /// bytes rather than whatever the server last saw on disk.
    pub async fn document_symbols(&self, abs_path: &Path, text: &str) -> SymbolAnswer {
        if let Err(e) = self.open_document(abs_path, text).await {
            return SymbolAnswer::Unindexed(format!("could not open the document: {e}"));
        }
        let uri = percent_encode_path(abs_path);
        match self
            .request(
                "textDocument/documentSymbol",
                json!({"textDocument": {"uri": uri}}),
            )
            .await
        {
            Ok(result) => parse_symbols(&result, &uri),
            Err(e) => SymbolAnswer::Unindexed(e),
        }
    }
}

/// Flatten a `documentSymbol` result into declaration names.
///
/// Pure, so the shapes that matter — hierarchical `DocumentSymbol[]`, flat
/// `SymbolInformation[]`, a foreign document, an empty index, garbage —
/// are all testable without a language server.
pub fn parse_symbols(result: &Value, uri: &str) -> SymbolAnswer {
    let Some(entries) = result.as_array() else {
        return match result {
            Value::Null => SymbolAnswer::Unindexed("the server reported no symbols".into()),
            _ => SymbolAnswer::Mismatched("documentSymbol did not return a list".into()),
        };
    };
    if entries.is_empty() {
        return SymbolAnswer::Unindexed("the server reported no symbols for this file".into());
    }
    let mut names = Vec::new();
    for entry in entries {
        if let Some(found) = entry.get("location").and_then(|l| l.get("uri")) {
            let Some(other) = found.as_str() else {
                return SymbolAnswer::Mismatched("a symbol carried an unreadable uri".into());
            };
            if !uris_match(other, uri) {
                return SymbolAnswer::Mismatched(format!("symbols were reported for {other}"));
            }
        }
        if !collect(entry, &mut names) {
            return SymbolAnswer::Mismatched("a symbol carried no name".into());
        }
    }
    SymbolAnswer::Resolved(names)
}

/// Append `entry`'s name and every nested name; `false` when the shape is
/// not a symbol at all.
fn collect(entry: &Value, out: &mut Vec<String>) -> bool {
    let Some(name) = entry.get("name").and_then(Value::as_str) else {
        return false;
    };
    out.push(name.to_string());
    match entry.get("children").and_then(Value::as_array) {
        None => true,
        Some(children) => children.iter().all(|child| collect(child, out)),
    }
}

/// Compare two `file:` uris by the path they denote, so a server that
/// escapes a character differently is not mistaken for a foreign answer.
fn uris_match(a: &str, b: &str) -> bool {
    percent_decode(a) == percent_decode(b)
}

fn percent_decode(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(&uri[i + 1..i + 3], 16) {
                    Ok(byte) => out.push(byte),
                    Err(_) => out.push(b'%'),
                }
                i += 3;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///repo/src/lib.rs";

    #[test]
    fn hierarchical_symbols_flatten_to_every_declaration() {
        let result = json!([
            {"name": "parse", "kind": 12},
            {"name": "Parser", "kind": 23, "children": [
                {"name": "new", "kind": 6},
                {"name": "run", "kind": 6}
            ]}
        ]);
        assert_eq!(
            parse_symbols(&result, URI),
            SymbolAnswer::Resolved(vec![
                "parse".into(),
                "Parser".into(),
                "new".into(),
                "run".into()
            ])
        );
    }

    #[test]
    fn flat_symbols_are_accepted_only_for_the_document_asked_about() {
        let ours = json!([{"name": "parse", "kind": 12,
            "location": {"uri": URI, "range": {}}}]);
        assert_eq!(
            parse_symbols(&ours, URI),
            SymbolAnswer::Resolved(vec!["parse".into()])
        );

        let theirs = json!([{"name": "parse", "kind": 12,
            "location": {"uri": "file:///repo/src/other.rs", "range": {}}}]);
        assert!(
            matches!(parse_symbols(&theirs, URI), SymbolAnswer::Mismatched(why)
                if why.contains("other.rs")),
            "a foreign document must never authorize"
        );
    }

    #[test]
    fn differently_escaped_uris_denote_the_same_document() {
        let escaped = json!([{"name": "parse",
            "location": {"uri": "file:///repo/src/lib%2Ers"}}]);
        assert_eq!(
            parse_symbols(&escaped, URI),
            SymbolAnswer::Resolved(vec!["parse".into()])
        );
    }

    #[test]
    fn an_empty_or_absent_index_is_not_an_answer() {
        assert!(matches!(
            parse_symbols(&json!([]), URI),
            SymbolAnswer::Unindexed(_)
        ));
        assert!(matches!(
            parse_symbols(&Value::Null, URI),
            SymbolAnswer::Unindexed(_)
        ));
    }

    #[test]
    fn unreadable_shapes_are_mismatches_not_empty_successes() {
        assert!(matches!(
            parse_symbols(&json!({"symbols": []}), URI),
            SymbolAnswer::Mismatched(_)
        ));
        assert!(matches!(
            parse_symbols(&json!([{"kind": 12}]), URI),
            SymbolAnswer::Mismatched(_)
        ));
        assert!(matches!(
            parse_symbols(&json!([{"name": "x", "location": {"uri": 7}}]), URI),
            SymbolAnswer::Mismatched(_)
        ));
    }

    #[tokio::test]
    async fn a_server_that_cannot_spawn_reports_unindexed_rather_than_symbols() {
        let tmp = tempfile::tempdir().unwrap();
        let client = LspClient::new(
            tmp.path(),
            std::path::PathBuf::from("not-a-language-server"),
        );
        let answer = client
            .document_symbols(&tmp.path().join("lib.rs"), "pub fn parse() {}\n")
            .await;
        assert!(matches!(answer, SymbolAnswer::Unindexed(_)), "{answer:?}");
    }
}
