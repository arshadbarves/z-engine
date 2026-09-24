//! `UserPromptSubmit` hooks: exit 2 blocks the prompt (nothing is sent),
//! and plain stdout of a passing hook becomes context for the model.

mod support;

use support::{BASE_SETTINGS, Harness, hook_script, hook_toml, last_user_text};
use z_engine_protocol::{Event, NoticeLevel};
use z_engine_testkit::{FixtureRepo, Script};

const GUARD: &str = r#"payload=$(cat)
case "$payload" in
  *forbidden*) echo "no forbidden words" >&2; exit 2 ;;
esac
echo "remember the style guide"
"#;

#[tokio::test]
async fn blocked_prompts_are_not_sent_and_context_is_added() {
    let scripts = tempfile::tempdir().unwrap();
    let guard = hook_script(scripts.path(), "guard.sh", GUARD);
    let settings = format!(
        "{BASE_SETTINGS}{}",
        hook_toml("UserPromptSubmit", None, &guard)
    );
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;

    h.submit("this is forbidden");
    h.wait(|e| {
        matches!(
            e,
            Event::Notice { level: NoticeLevel::Warn, text } if text.contains("no forbidden words")
        )
    })
    .await;
    assert_eq!(
        h.events.count(|e| matches!(e, Event::TurnStarted { .. })),
        0
    );
    assert!(h.transcript().is_empty());

    h.model.push(Script::text("hi"));
    h.run_turn("hello").await;
    let requests = h.main_requests();
    assert_eq!(
        requests.len(),
        1,
        "the blocked prompt never reached the model"
    );
    let prompt = last_user_text(&requests[0]);
    assert!(
        prompt.contains("hello") && prompt.contains("remember the style guide"),
        "{prompt}"
    );
}
