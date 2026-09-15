//! OpenCode Zen request identity.
//!
//! Zen's `/chat/completions` gateway expects the same session headers the
//! official OpenCode client sends (`x-opencode-session`, `x-opencode-request`,
//! `x-opencode-client`). Without them, free models return 400
//! `MissingSessionID`. We send those headers and identify as z-engine.

const CLIENT: &str = "cli";

pub fn is_zen_url(base_url: &str) -> bool {
    base_url.to_ascii_lowercase().contains("opencode.ai")
}

pub fn new_session_id() -> String {
    prefixed_id("ses_")
}

pub fn new_request_id() -> String {
    prefixed_id("req_")
}

pub fn user_agent() -> String {
    format!("zengine/{}", env!("CARGO_PKG_VERSION"))
}

pub fn apply_headers(req: reqwest::RequestBuilder, session_id: &str) -> reqwest::RequestBuilder {
    req.header("x-opencode-session", session_id)
        .header("x-opencode-request", new_request_id())
        .header("x-opencode-client", CLIENT)
        .header(reqwest::header::USER_AGENT, user_agent())
}

fn prefixed_id(prefix: &str) -> String {
    let ulid = ulid::Ulid::new().to_string().to_ascii_lowercase();
    format!("{prefix}{}", &ulid[..24])
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
        let request = new_request_id();
        assert!(session.starts_with("ses_"));
        assert!(request.starts_with("req_"));
        assert_eq!(session.len(), 28);
        assert_eq!(request.len(), 28);
        assert_ne!(session, request);
    }

    #[test]
    fn user_agent_is_zengine() {
        assert!(user_agent().starts_with("zengine/"));
        assert!(!user_agent().contains("opencode/"));
    }
}
