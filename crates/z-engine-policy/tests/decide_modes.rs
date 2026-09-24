//! Defaults for every permission mode and action, without any rules.

mod support;

use support::Outcome::{Allow, Ask, Deny};
use support::*;
use z_engine_policy::{Action, Decision, Policy, PolicyConfig};

#[test]
fn every_mode_and_action_without_rules() {
    let policy = policy(&[], &[], &[]);
    // Expected outcomes in MODES order: default, acceptEdits, plan, bypass.
    let cases: Vec<(&str, Action, [Outcome; 4])> = vec![
        (
            "Read",
            read(&["/work/proj/src/lib.rs"]),
            [Allow, Allow, Allow, Allow],
        ),
        ("Glob", read(&[]), [Allow, Allow, Allow, Allow]),
        (
            "Grep",
            read(&["/data/shared/table.csv"]),
            [Allow, Allow, Allow, Allow],
        ),
        ("Read", read(&["/etc/hosts"]), [Ask, Ask, Ask, Allow]),
        (
            "LSP",
            read(&["/work/proj/src", "/etc"]),
            [Ask, Ask, Ask, Allow],
        ),
        (
            "Edit",
            write(&["/work/proj/src/lib.rs"]),
            [Ask, Allow, Deny, Allow],
        ),
        (
            "MultiEdit",
            write(&["/data/shared/a", "/work/proj/b"]),
            [Ask, Allow, Deny, Allow],
        ),
        ("Write", write(&["/tmp/out.txt"]), [Ask, Ask, Deny, Allow]),
        (
            "Edit",
            write(&["/work/proj/.git/config"]),
            [Ask, Ask, Deny, Allow],
        ),
        (
            "Write",
            write(&["/work/proj/.z-engine/settings.json"]),
            [Ask, Ask, Deny, Allow],
        ),
        ("ApplyAgentChanges", write(&[]), [Ask, Ask, Deny, Allow]),
        (
            "Bash",
            exec("git status && ls -la"),
            [Allow, Allow, Allow, Allow],
        ),
        ("Bash", exec("cat /etc/hosts"), [Ask, Ask, Ask, Allow]),
        ("Bash", exec("cargo test"), [Ask, Ask, Deny, Allow]),
        (
            "Bash",
            exec("mkdir -p src/new && touch src/new/mod.rs"),
            [Ask, Allow, Deny, Allow],
        ),
        ("Bash", exec("rm -rf /"), [Ask, Ask, Deny, Allow]),
        ("Bash", exec("echo $(date)"), [Ask, Ask, Deny, Allow]),
        (
            "WebFetch",
            fetch("https://example.com/docs"),
            [Ask, Ask, Ask, Allow],
        ),
        ("WebSearch", Action::Search, [Ask, Ask, Ask, Allow]),
        (
            "Agent",
            Action::Agent {
                agent_type: "explore".into(),
            },
            [Allow, Allow, Allow, Allow],
        ),
        (
            "mcp__github__get_issue",
            mcp("github", "get_issue", true),
            [Ask, Ask, Ask, Allow],
        ),
        (
            "mcp__github__create_issue",
            mcp("github", "create_issue", false),
            [Ask, Ask, Deny, Allow],
        ),
        (
            "Skill",
            Action::Skill { name: "pdf".into() },
            [Allow, Allow, Allow, Allow],
        ),
        (
            "TodoWrite",
            Action::Other { read_only: true },
            [Allow, Allow, Allow, Allow],
        ),
        (
            "ExitPlanMode",
            Action::Other { read_only: true },
            [Allow, Allow, Allow, Allow],
        ),
        (
            "JobKill",
            Action::Other { read_only: false },
            [Ask, Ask, Deny, Allow],
        ),
    ];
    for (tool, action, expected) in cases {
        for (mode, want) in MODES.into_iter().zip(expected) {
            let decision = policy.decide(tool, &action, mode);
            assert_eq!(
                outcome(&decision),
                want,
                "{tool} {action:?} in {mode:?}: {decision:?}"
            );
        }
    }
}

#[test]
fn plan_mode_explains_how_to_leave() {
    let policy = policy(&["Bash", "Edit"], &[], &[]);
    let decision = policy.decide("Edit", &write(&["/work/proj/a.rs"]), PermissionMode::Plan);
    assert_eq!(
        decision,
        Decision::Deny {
            reason: "plan mode is read-only; use ExitPlanMode to propose the plan".into()
        }
    );
    let command = bash(&policy, "cargo build", PermissionMode::Plan);
    assert_eq!(outcome(&command), Deny);
    assert_eq!(
        outcome(&bash(&policy, "git log", PermissionMode::Plan)),
        Allow
    );
}

#[test]
fn bypass_still_honors_deny_rules() {
    let policy = policy(
        &[],
        &[],
        &[
            "Bash(rm:*)",
            "Read(~/.ssh/**)",
            "WebFetch",
            "mcp__prod",
            "JobKill",
        ],
    );
    let bypass = PermissionMode::Bypass;
    let denied: Vec<(&str, Action)> = vec![
        ("Bash", exec("rm -rf build")),
        ("Read", read(&["/home/me/.ssh/id_ed25519"])),
        ("WebFetch", fetch("https://example.com")),
        ("mcp__prod__drop", mcp("prod", "drop", false)),
        ("JobKill", Action::Other { read_only: false }),
    ];
    for (tool, action) in denied {
        assert_eq!(
            outcome(&policy.decide(tool, &action, bypass)),
            Deny,
            "{tool}"
        );
    }
    assert_eq!(outcome(&bash(&policy, "cargo build", bypass)), Allow);
    assert_eq!(
        policy.decide("Bash", &exec("cargo build"), bypass).reason(),
        "bypass mode"
    );
}

#[test]
fn approval_cards_carry_suggestions_and_persistence() {
    let policy = policy(&[], &[], &[]);
    let default = PermissionMode::Default;
    let outside_read = policy.decide("Read", &read(&["/etc/hosts"]), default);
    assert_eq!(suggested(&outside_read), Some("Read(//etc/hosts)"));
    assert!(!persistable(&outside_read));
    let home_read = policy.decide("Read", &read(&["/home/me/.cargo/config.toml"]), default);
    assert_eq!(suggested(&home_read), Some("Read(~/.cargo/config.toml)"));
    let edit = policy.decide("Edit", &write(&["/work/proj/src/lib.rs"]), default);
    assert_eq!(suggested(&edit), Some("Edit"));
    assert!(persistable(&edit));
    assert_eq!(edit.reason(), "edits src/lib.rs");
    let outside_write = policy.decide("Write", &write(&["/tmp/x[1].txt"]), default);
    assert_eq!(suggested(&outside_write), Some("Edit(//tmp/x[[]1[]].txt)"));
    assert!(!persistable(&outside_write));
    let protected = policy.decide(
        "Edit",
        &write(&["/work/proj/.git/hooks/pre-commit"]),
        default,
    );
    assert_eq!(suggested(&protected), None);
    assert!(!persistable(&protected));
    let command = bash(&policy, "npm run test -- --watch", default);
    assert_eq!(suggested(&command), Some("Bash(npm run test:*)"));
    assert!(persistable(&command));
    let fetch = policy.decide("WebFetch", &fetch("https://Docs.RS/serde"), default);
    assert_eq!(suggested(&fetch), Some("WebFetch(domain:docs.rs)"));
    let search = policy.decide("WebSearch", &Action::Search, default);
    assert_eq!(suggested(&search), Some("WebSearch"));
    let tool = policy.decide(
        "mcp__github__create_issue",
        &mcp("github", "create_issue", false),
        default,
    );
    assert_eq!(suggested(&tool), Some("mcp__github__create_issue"));
    let kill = policy.decide("JobKill", &Action::Other { read_only: false }, default);
    assert_eq!(suggested(&kill), Some("JobKill"));
}

#[test]
fn read_only_auto_allow_can_be_turned_off() {
    let settings = PolicyConfig {
        auto_allow_read_only_bash: false,
        ..PolicyConfig::default()
    };
    let (policy, _) = Policy::new(&settings, ctx());
    let status = bash(&policy, "git status", PermissionMode::Default);
    assert_eq!(outcome(&status), Ask);
    assert_eq!(status.reason(), "read-only commands need approval");
    assert_eq!(
        outcome(&bash(&policy, "ls", PermissionMode::AcceptEdits)),
        Ask
    );
    assert_eq!(
        outcome(&bash(&policy, "mkdir docs", PermissionMode::AcceptEdits)),
        Allow
    );
    assert_eq!(
        outcome(&bash(&policy, "git status", PermissionMode::Plan)),
        Ask
    );
}
