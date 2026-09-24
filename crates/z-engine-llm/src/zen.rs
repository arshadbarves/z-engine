//! OpenCode Zen request identity.
//!
//! Zen's gateway expects the session headers the official OpenCode client
//! sends (`x-opencode-session`, `x-opencode-request`, `x-opencode-client`);
//! without them free models answer 400 `MissingSessionID`. We send them and
//! identify as z-engine.

const CLIENT: &str = "cli";
const ID_CHARS: usize = 24;

pub(crate) fn is_zen_url(base_url: &str) -> bool {
    base_url.to_ascii_lowercase().contains("opencode.ai")
}

pub(crate) fn new_session_id() -> String {
    prefixed_id("ses_")
}

/// A Zen-shaped session id derived from a stable conversation key, so one
/// conversation keeps one Zen session. `None` when the key has no usable
/// characters.
pub(crate) fn session_id_for(key: &str) -> Option<String> {
    if key.starts_with("ses_") {
        return Some(key.to_string());
    }
    let tail: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .take(ID_CHARS)
        .collect();
    (!tail.is_empty()).then(|| format!("ses_{tail}"))
}

pub(crate) fn user_agent() -> String {
    format!("zengine/{}", env!("CARGO_PKG_VERSION"))
}

pub(crate) fn apply_headers(
    request: reqwest::RequestBuilder,
    session_id: &str,
) -> reqwest::RequestBuilder {
    request
        .header("x-opencode-session", session_id)
        .header("x-opencode-request", prefixed_id("req_"))
        .header("x-opencode-client", CLIENT)
        .header(reqwest::header::USER_AGENT, user_agent())
}

fn prefixed_id(prefix: &str) -> String {
    let ulid = ulid::Ulid::new().to_string().to_ascii_lowercase();
    format!("{prefix}{}", &ulid[..ID_CHARS])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_zen_gateway() {
        assert!(is_zen_url("https://opencode.ai/zen/v1"));
        assert!(is_zen_url("https://OPENCODE.AI/zen/v1/"));
        assert!(!is_zen_url("https://openrouter.ai/api/v1"));
    }

    #[test]
    fn ids_match_opencode_shape() {
        let session = new_session_id();
        let request = prefixed_id("req_");
        assert!(session.starts_with("ses_"));
        assert!(request.starts_with("req_"));
        assert_eq!(session.len(), 28);
        assert_eq!(request.len(), 28);
        assert_ne!(session[4..], request[4..]);
    }

    #[test]
    fn conversation_keys_map_to_stable_session_ids() {
        let key = "01K5ZQ8X1B2C3D4E5F6G7H8J9K";
        assert_eq!(session_id_for(key), session_id_for(key));
        assert_eq!(session_id_for(key).unwrap(), "ses_01k5zq8x1b2c3d4e5f6g7h8j");
        assert_eq!(
            session_id_for("ses_existing").as_deref(),
            Some("ses_existing")
        );
        assert_eq!(session_id_for("--"), None);
    }

    #[test]
    fn user_agent_is_zengine() {
        assert!(user_agent().starts_with("zengine/"));
        assert!(!user_agent().contains("opencode/"));
    }
}
