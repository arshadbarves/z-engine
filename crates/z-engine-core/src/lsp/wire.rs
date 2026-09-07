//! JSON-RPC wire helpers: framing, uris, message shapes.
//!
//! Everything here is pure so the protocol details the gate depends on —
//! notably that `didOpen` carries the judged bytes and that a server *request*
//! is never mistaken for a response to ours — are testable without a server.

use std::path::Path;

use serde_json::{Value, json};

/// Wrap a body in the `Content-Length` framing every LSP message uses.
pub fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// Find the end offset of a complete frame (headers + body), if present.
pub fn find_frame(buf: &[u8]) -> Option<usize> {
    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)?;
    let headers = String::from_utf8_lossy(&buf[..header_end - 4]);
    let len: usize = headers
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .and_then(|v| v.trim().parse().ok())?;
    if buf.len() >= header_end + len {
        Some(header_end + len)
    } else {
        None
    }
}

/// A `file://` uri for an absolute path, percent-encoding what must be.
pub fn percent_encode_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::from("file://");
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Is this message a response to a request *we* sent?
///
/// Servers send requests too (rust-analyzer asks for diagnostic refreshes),
/// and those carry an id from the server's own counter. Routing them by id
/// would resolve one of our pending requests with a bogus answer and leave
/// the real response unclaimed, so a `method` field disqualifies a message.
pub fn is_response(v: &Value) -> bool {
    v.get("id").and_then(Value::as_i64).is_some() && v.get("method").is_none()
}

/// `textDocument/didOpen` params: the whole document goes in `text`.
pub fn did_open_params(uri: &str, text: &str) -> Value {
    json!({
        "textDocument": {
            "uri": uri,
            "languageId": "rust",
            "version": 1,
            "text": text,
        }
    })
}

/// `textDocument/didChange` params: a single full-document change.
pub fn did_change_params(uri: &str, version: i64, text: &str) -> Value {
    json!({
        "textDocument": {"uri": uri, "version": version},
        "contentChanges": [{"text": text}]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_states_the_byte_length_of_its_body() {
        assert_eq!(frame("{}"), b"Content-Length: 2\r\n\r\n{}".to_vec());
    }

    #[test]
    fn a_partial_frame_is_not_a_frame_yet() {
        assert_eq!(find_frame(b"Content-Length: 5\r\n\r\n{}"), None);
        assert_eq!(find_frame(b"Content-Length: 2\r\n\r\n{}xx"), Some(23));
    }

    #[test]
    fn paths_with_spaces_are_encoded() {
        assert_eq!(
            percent_encode_path(Path::new("/a b/c.rs")),
            "file:///a%20b/c.rs"
        );
    }

    #[test]
    fn a_server_request_is_not_a_response() {
        let refresh = json!({"jsonrpc":"2.0","id":1,"method":"workspace/diagnostic/refresh"});
        assert!(!is_response(&refresh));
        assert!(is_response(&json!({"jsonrpc":"2.0","id":1,"result":null})));
        assert!(is_response(
            &json!({"jsonrpc":"2.0","id":1,"error":{"message":"no"}})
        ));
        assert!(!is_response(
            &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics"})
        ));
    }

    #[test]
    fn did_open_carries_the_text_and_did_change_carries_the_edit() {
        let open = did_open_params("file:///x.rs", "fn main() {}");
        assert_eq!(open["textDocument"]["text"], "fn main() {}");
        assert!(open.get("contentChanges").is_none());

        let change = did_change_params("file:///x.rs", 7, "fn main() {}");
        assert_eq!(change["contentChanges"][0]["text"], "fn main() {}");
        assert_eq!(change["textDocument"]["version"], 7);
    }
}
