# Roadmap progress

Mirrors spec §9. Each version: tests green · clippy clean · demo done · tagged.

- [x] **v0.1 — Walking skeleton** — workspace; SSE provider w/ streamed tool-call deltas;
      agent loop (approvals, abort, parallel tools); TUI chat/input/status/approval modal;
      tools `bash` + `read_file`; mocked-provider integration suite; headless acceptance mode
- [x] v0.2 — Editing (`write_file`, `edit_file` ladder + read-before-edit, `glob`, `grep`, diff modal)
- [x] v0.3 — Context engine (AGENTS.md ✓ already, token meter, `/compact`, notes, demotion, auto-compaction)
- [x] v0.4 — Sessions (JSONL persistence, resume, picker)
- [x] v0.5 — Permissions hardening (persistent allowlists, config layering, outside-root guard)
- [x] v0.6 — Repo map (tree-sitter)
- [x] v0.7 — Subagents (`task`)
- [ ] v0.8 — LSP (rust-analyzer, diagnostics hook)
- [x] v0.9 — Review pass + MCP
- [x] v1.0 — Polish & distribution

## Live acceptance runs

| Version | Date | Task | Result |
|---|---|---|---|
| v0.1 | 2026-08-24 | failing-test fix in tmp/acceptance-v01 via OpenRouter | PASS — model ran tests, read lib.rs, hit macOS `sed -i` failure, self-corrected via python3, tests green (2/2) |
| v0.2 | 2026-08-24 | multi-file priority feature in tmp/acceptance-v02 via OpenRouter | PASS — 2×read_file + 3×edit_file (diff previews auto-approved) + 1×bash across tasks.rs/main.rs; 4/4 tests; CLI `[P3] write docs` verified |
| v0.3 | 2026-08-24 | synthetic >100k-token session (auto-compaction at 95% budget) + live smoke re-run of v0.1 scenario | PASS — summarizer side-request fired, secret fact survived via L1 notes, elision markers + spill files verified; live headless smoke green |
| v0.4 | 2026-08-24 | kill -9 mid-task → --session resume → continue (mocked) + live plant/recall codeword across restart | PASS — transcript survived SIGKILL incl. round-1 tool result; resumed request carried full prior context; live model answered "MANGO-77" from a previous process's session |
| v0.4b | 2026-08-24 | live layering proof in tmp/acceptance-v01 | PASS — `echo hello-layering` denied pre-config, executes after `.harness/config.toml` allow=["echo*"]; mocked e2e proves AlwaysPersist writes rule + zero re-prompts; outside-root write disables persist |
| v0.6 | 2026-08-24 | "where is process-group kill defined?" on harness itself | PASS — answered kill_tree @ crates/harness-core/src/tools/bash.rs from injected symbol map; single read_file, zero greps |
| v0.7 | 2026-08-24 | task-tool delegation on harness repo (mocked token-delta e2e + live OpenRouter) | PASS — sub explored read-only, parent context delta <1KB vs ~4.4KB sub burn; live run listed all 10 tools/*.rs via delegation |
| v0.8 | 2026-08-24 | deliberately broken edit reported + fixed without manual cargo check | PASS — edit_file result carried `[lsp EE0277] cannot add &str to i32` via cargo-check hook; model quoted it verbatim and fixed; go_to_definition/find_references/lsp_diagnostics tools registered |
| v0.9 | 2026-08-24 | seeded bug caught by review pass + third-party MCP server usable | PASS — reviewer findings injected as user-role message after edit batch (mocked + live); python echo MCP server registered from project config and called by the model (`PONG:live-mcp-check`) |
| v1.0 | 2026-08-26 | error audit (8 runtime unwraps → 1 real fix), cost table live in status bar, README, `cargo install --path crates/harness-tui`, v1.0.0 | PASS — install verified, help shows v1.0.0, live demo completed with usage accounting; **week-long daily-driver validation handed to the owner** |

## v1.1 — Interaction parity (post-1.0)

- [x] Inline renderer (no alt-screen/mouse capture) — native scrollback + selection restored
- [x] Wheel direction fixed; streaming append-only
- [x] Approval keys: 1/y once · 2/a/s session · 3/p persist · 4/n/Esc/Ctrl-C deny (legacy 'a' regression fixed)
- [x] Shift+Tab permission modes: normal / auto-accept edits / plan (+ `--permission-mode` flag); PLAN blocks mutations without prompting
- [x] Slash commands: /help /clear /cost /status /quit added (/compact /notes existed)
- Live: accept-edits auto-approved an edit; plan mode blocked edit_file with notice

| Version | Date | Task | Result |
|---|---|---|---|
| v1.1 | 2026-08-26 | pty parity drive + live mode demos | PASS — see checklist above |
