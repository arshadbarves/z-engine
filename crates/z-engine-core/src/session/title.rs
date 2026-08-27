//! Session display titles: persisted `Title` events plus a first-line
//! fallback used until (or if) the side-request completes.

use super::SessionEvent;

/// 3–8 word titles from the model are stored as `SessionEvent::Title`.
/// Until then, show a short first line of the first user message.
pub fn display_title(events: &[SessionEvent]) -> Option<String> {
    events
        .iter()
        .find_map(|ev| match ev {
            SessionEvent::Title { text } => {
                let t = text.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            }
            _ => None,
        })
        .or_else(|| {
            events.iter().find_map(|ev| match ev {
                SessionEvent::UserMsg { text, .. } => Some(fallback_title(text)),
                _ => None,
            })
        })
}

/// First non-empty line, clipped to 48 characters.
pub fn fallback_title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(prompt.trim());
    let mut out: String = line.chars().take(48).collect();
    if line.chars().count() > 48 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionEvent;

    fn user(text: &str) -> SessionEvent {
        SessionEvent::UserMsg {
            text: text.into(),
            images: vec![],
        }
    }

    #[test]
    fn prefers_persisted_title_over_prompt() {
        let events = vec![
            user("Fix the flaky auth test in login.rs please"),
            SessionEvent::Title {
                text: "Fix flaky auth test".into(),
            },
        ];
        assert_eq!(
            display_title(&events).as_deref(),
            Some("Fix flaky auth test")
        );
    }

    #[test]
    fn falls_back_to_clipped_first_line() {
        let long = "a".repeat(80);
        let events = vec![user(&format!("{long}\nmore"))];
        let t = display_title(&events).unwrap();
        assert!(t.starts_with('a'));
        assert!(t.ends_with('…'));
        assert!(t.chars().count() <= 49);
    }

    #[test]
    fn empty_events_have_no_title() {
        assert_eq!(display_title(&[]), None);
    }
}
