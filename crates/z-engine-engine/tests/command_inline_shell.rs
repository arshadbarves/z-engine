//! `` !`cmd` `` in a command body runs before the prompt is sent when the
//! command's `allowed-tools` permit it; otherwise it is left unrun with a
//! note that it needs approval.

mod support;

use support::{Harness, user_blocks};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn inline_commands_need_allowed_tools() {
    let repo = FixtureRepo::empty();
    repo.write(
        ".z-engine/commands/allowed.md",
        "---\nallowed-tools: Bash(touch:*)\n---\nMade: !`touch made.txt && echo created`",
    );
    repo.write(
        ".z-engine/commands/blocked.md",
        "Made: !`touch blocked.txt`",
    );
    let mut h = Harness::start(repo).await;

    h.model.push(Script::text("ok"));
    h.command("allowed", "");
    h.turn_finished().await;
    let body = user_blocks(&h.main_requests().pop().unwrap())[1].clone();
    assert!(h.repo.exists("made.txt"));
    assert!(
        body.contains("$ touch made.txt && echo created\ncreated"),
        "{body}"
    );

    h.model.push(Script::text("ok"));
    h.command("blocked", "");
    h.turn_finished().await;
    let body = user_blocks(&h.main_requests().pop().unwrap())[1].clone();
    assert!(!h.repo.exists("blocked.txt"));
    assert!(body.contains("`touch blocked.txt` was not run"), "{body}");
}
