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

use tokio::time::{Duration, Instant, sleep};

use super::{LspClient, percent_encode_path};

/// How long to keep asking while the server says it has nothing.
///
/// A freshly spawned rust-analyzer answers from the opened document almost
/// at once, then reports nothing for a beat while it loads the workspace,
/// then answers stably. Refusing an edit inside that window would gate on
/// the server's start-up schedule rather than on the code, so an
/// `Unindexed` answer is re-asked until it settles or the budget runs out.
const WARMUP_BUDGET: Duration = Duration::from_secs(3);
const WARMUP_INTERVAL: Duration = Duration::from_millis(250);

/// One declaration the server placed in a document, with the extent it
/// occupies.
///
/// The extent is what makes a symbol name load-bearing rather than
/// decorative: "the order named `parse`" says nothing about a patch until
/// the lines the patch touches can be compared against the lines `parse`
/// actually occupies. Kept as a 1-based inclusive line span to match
/// every other range in this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSymbol {
    pub name: String,
    /// The declaration this one is nested in (`impl Parser`'s type for a
    /// method, `containerName` for a flat answer). `None` at file level.
    pub container: Option<String>,
    /// First and last line of the whole declaration, 1-based inclusive.
    pub range: (u32, u32),
}

/// What the semantic provider could say about one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolAnswer {
    /// The server answered for this document; these are its declarations
    /// (flattened, nested items included) and the lines they occupy.
    Resolved(Vec<DeclaredSymbol>),
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
        let deadline = Instant::now() + WARMUP_BUDGET;
        loop {
            let answer = match self
                .request(
                    "textDocument/documentSymbol",
                    json!({"textDocument": {"uri": uri}}),
                )
                .await
            {
                Ok(result) => parse_symbols(&result, &uri),
                Err(e) => SymbolAnswer::Unindexed(e),
            };
            let warming = matches!(answer, SymbolAnswer::Unindexed(_));
            if !warming || Instant::now() >= deadline {
                return answer;
            }
            sleep(WARMUP_INTERVAL).await;
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
    let mut declared = Vec::new();
    for entry in entries {
        if let Some(found) = entry.get("location").and_then(|l| l.get("uri")) {
            let Some(other) = found.as_str() else {
                return SymbolAnswer::Mismatched("a symbol carried an unreadable uri".into());
            };
            if !uris_match(other, uri) {
                return SymbolAnswer::Mismatched(format!("symbols were reported for {other}"));
            }
        }
        if let Err(why) = collect(entry, None, &mut declared) {
            return SymbolAnswer::Mismatched(why);
        }
    }
    SymbolAnswer::Resolved(declared)
}

/// Append `entry` and every nested declaration, or say why the shape
/// cannot be trusted.
///
/// A symbol with no name or no range is not a weaker answer, it is an
/// unusable one: the gate binds a patch to a symbol's *extent*, so a
/// declaration that cannot say where it lives would silently authorize
/// nothing — or, worse, everything.
fn collect(
    entry: &Value,
    container: Option<&str>,
    out: &mut Vec<DeclaredSymbol>,
) -> Result<(), String> {
    let Some(name) = entry.get("name").and_then(Value::as_str) else {
        return Err("a symbol carried no name".into());
    };
    let Some(range) = line_range(entry) else {
        return Err(format!("the symbol {name} carried no usable range"));
    };
    out.push(DeclaredSymbol {
        name: name.to_string(),
        container: container
            .map(str::to_string)
            .or_else(|| {
                entry
                    .get("containerName")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|c| !c.is_empty()),
        range,
    });
    match entry.get("children").and_then(Value::as_array) {
        None => Ok(()),
        Some(children) => children
            .iter()
            .try_for_each(|child| collect(child, Some(name), out)),
    }
}

/// The 1-based inclusive line span of a `DocumentSymbol` (`range`) or a
/// `SymbolInformation` (`location.range`). `selectionRange` is
/// deliberately not used: it covers the name, not the declaration.
fn line_range(entry: &Value) -> Option<(u32, u32)> {
    let range = entry
        .get("range")
        .or_else(|| entry.get("location").and_then(|l| l.get("range")))?;
    let line =
        |end: &str| -> Option<u32> { u32::try_from(range.get(end)?.get("line")?.as_u64()?).ok() };
    let (start, end) = (line("start")?, line("end")?);
    Some((start + 1, end.max(start) + 1))
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

    /// A `DocumentSymbol` range, 0-based half-open as the protocol has it.
    fn range(first: u64, last: u64) -> Value {
        json!({"start": {"line": first, "character": 0}, "end": {"line": last, "character": 1}})
    }

    fn declared(answer: &SymbolAnswer) -> &[DeclaredSymbol] {
        match answer {
            SymbolAnswer::Resolved(symbols) => symbols,
            other => panic!("expected a resolved answer, got {other:?}"),
        }
    }

    fn names(answer: &SymbolAnswer) -> Vec<&str> {
        declared(answer).iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn hierarchical_symbols_flatten_to_every_declaration_with_its_extent() {
        let result = json!([
            {"name": "parse", "kind": 12, "range": range(0, 4)},
            {"name": "Parser", "kind": 23, "range": range(6, 20), "children": [
                {"name": "new", "kind": 6, "range": range(7, 10)},
                {"name": "run", "kind": 6, "range": range(12, 19)}
            ]}
        ]);
        let answer = parse_symbols(&result, URI);
        assert_eq!(names(&answer), ["parse", "Parser", "new", "run"]);
        // 0-based half-open protocol ranges become 1-based inclusive lines.
        assert_eq!(declared(&answer)[0].range, (1, 5));
        assert_eq!(declared(&answer)[1].range, (7, 21));
        // Nesting is kept, so `Parser::new` can be told from a free `new`.
        assert_eq!(declared(&answer)[2].container.as_deref(), Some("Parser"));
        assert_eq!(declared(&answer)[2].range, (8, 11));
        assert_eq!(declared(&answer)[0].container, None);
    }

    #[test]
    fn flat_symbols_are_accepted_only_for_the_document_asked_about() {
        let ours = json!([{"name": "parse", "kind": 12, "containerName": "Parser",
            "location": {"uri": URI, "range": range(3, 9)}}]);
        let answer = parse_symbols(&ours, URI);
        assert_eq!(names(&answer), ["parse"]);
        assert_eq!(declared(&answer)[0].range, (4, 10));
        assert_eq!(declared(&answer)[0].container.as_deref(), Some("Parser"));

        let theirs = json!([{"name": "parse", "kind": 12,
            "location": {"uri": "file:///repo/src/other.rs", "range": range(0, 1)}}]);
        assert!(
            matches!(parse_symbols(&theirs, URI), SymbolAnswer::Mismatched(why)
                if why.contains("other.rs")),
            "a foreign document must never authorize"
        );
    }

    #[test]
    fn differently_escaped_uris_denote_the_same_document() {
        let escaped = json!([{"name": "parse",
            "location": {"uri": "file:///repo/src/lib%2Ers", "range": range(0, 2)}}]);
        assert_eq!(names(&parse_symbols(&escaped, URI)), ["parse"]);
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
            parse_symbols(&json!([{"kind": 12, "range": range(0, 1)}]), URI),
            SymbolAnswer::Mismatched(_)
        ));
        assert!(matches!(
            parse_symbols(&json!([{"name": "x", "location": {"uri": 7}}]), URI),
            SymbolAnswer::Mismatched(_)
        ));
    }

    /// A declaration with no range cannot be bound to a patch, so it is a
    /// mismatch rather than a symbol the gate would happily authorize on
    /// name alone.
    #[test]
    fn a_symbol_without_a_range_is_refused_rather_than_authorized_by_name() {
        assert!(
            matches!(parse_symbols(&json!([{"name": "parse", "kind": 12}]), URI),
                SymbolAnswer::Mismatched(why) if why.contains("parse")),
            "a symbol that cannot say where it lives proves nothing"
        );
        // …including one buried in an otherwise well-formed tree.
        let nested = json!([{"name": "Parser", "range": range(0, 9), "children": [
            {"name": "run", "kind": 6}
        ]}]);
        assert!(matches!(
            parse_symbols(&nested, URI),
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
