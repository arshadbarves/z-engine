# Roadmap progress

Mirrors spec §9. Each version: tests green · clippy clean · demo done · tagged.

- [x] **v0.1 — Walking skeleton** — workspace; SSE provider w/ streamed tool-call deltas;
      agent loop (approvals, abort, parallel tools); TUI chat/input/status/approval modal;
      tools `bash` + `read_file`; mocked-provider integration suite; headless acceptance mode
- [ ] v0.2 — Editing (`write_file`, `edit_file` ladder + read-before-edit, `glob`, `grep`, diff modal)
- [ ] v0.3 — Context engine (AGENTS.md ✓ already, token meter, `/compact`, notes, demotion, auto-compaction)
- [ ] v0.4 — Sessions (JSONL persistence, resume, picker)
- [ ] v0.5 — Permissions hardening (persistent allowlists, config layering, outside-root guard)
- [ ] v0.6 — Repo map (tree-sitter)
- [ ] v0.7 — Subagents (`task`)
- [ ] v0.8 — LSP (rust-analyzer, diagnostics hook)
- [ ] v0.9 — Review pass + MCP
- [ ] v1.0 — Polish & distribution

## Live acceptance runs

| Version | Date | Task | Result |
|---|---|---|---|
| v0.1 | pending | failing-test fix in scratch repo | — |
