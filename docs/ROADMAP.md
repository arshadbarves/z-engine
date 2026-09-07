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

## GUI — desktop application v0.1 (post-1.0)

- [x] M0 scaffold — Tauri 2 shell owning AgentHandle; EventRx → webview `appEvent`; Svelte 5 streaming transcript; Send/Stop composer (`834b0da`)
- [x] M1 chat parity — approve/approve_with_rule/deny/set_mode/set_model commands; tool start+finish merge into one card; approval cards with buttons + rule suggestions; thinking stream/collapse; runtime fix (app-lifetime tokio runtime entered for Tauri) (`ec39fc6`)
- [x] M2 sessions & permissions — list/open/delete/new sessions via core API with replay-resume; settings panel CRUD for persisted bash rules (`2754878`, `5652d9d`)
- [x] M3 packaging — scripts/package-gui.sh assembles Harness.app (Info.plist, RGBA icon, ad-hoc codesign); bundled binary verified launching (`14:42` build)

Design doc: docs/design/gui-v0.1.md · TUI frozen at v1.1.x as keyboard-only client.

## v1.2 — Evidence-gated runs (Rust vertical slice)

Opt-in per run with `--guarded`. A guarded turn mints evidence when it
reads, refuses mutations that no admitted work order covers, and ends only
when verification it did not author says it may.

- [x] Evidence store: every read mints an id; edits cite the lines they cover
- [x] Work orders: goal, writable paths, target symbols, cited evidence, acceptance commands
- [x] Mutation gate: work order → in-root → declared scope → named symbol (Rust) → evidence covering every changed line → rust-analyzer placing that symbol
- [x] Completion gate: `cargo check --workspace --all-targets` plus the order's acceptance commands, over a diff whose every change is attributable to a governed tool; the verdict lands in `<project>/.z-engine/runs/<id>/verification.json`
- [x] Record & replay: a cassette carries the run's traffic, gate rulings, evidence ids, verdict and cost; a replay re-asks its own gates and re-runs its own verification with no network and no key
- [x] Headless exposure: `--guarded`, `--record-run PATH`, `--replay-run PATH`, `--metrics-out PATH`

```bash
# record an evidence-gated run and keep its metrics
zengine --guarded --headless "make word_count count words" \
        --record-run ~/tapes/run.jsonl --metrics-out ~/tapes/metrics.json

# re-run it from the tape: no provider, no credential
zengine --headless "make word_count count words" \
        --replay-run ~/tapes/run.jsonl
```

Cassettes must live outside the project directory — a guarded run accounts
for every change under its root, its own tape included.

Baseline (needs `rust-analyzer` on PATH; without it the test fails rather
than skipping — set `Z_ENGINE_ALLOW_MISSING_SEMANTICS=1` to opt out):
`crates/z-engine-core/tests/guarded_vertical_slice.rs` drives the
frozen fixture in `tests/fixtures/guarded-rust-edit` end to end (scoped
order, one function changed, second file refused, held-out `cargo test`
deciding the turn) and replays that run to the same verdict.
Limits — Rust-only symbol authorization, rust-analyzer required,
cargo-only acceptance, no mutating shell, no unread new files — are
recorded in docs/deviations.md rows 10–14.
