# Rust Evidence-Gated Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship one guarded Rust edit path that records and replays, requires fresh evidence and a work order, enforces scope, and verifies completion.
**Architecture:** Add provider and recording seams around the loop, then thread evidence and a work order through `ToolCtx`; guarded mode bounds prompts, blocks ungrounded mutation, and verifies before completion.

**Tech Stack:** Rust 1.85, Tokio, serde/JSONL, SHA-256 content addressing, tree-sitter-rust, rust-analyzer, Cargo diagnostics, existing TUI headless runner.

## Global Constraints

- Keep `z-engine-provider` transport-only and `z-engine-core` UI-independent.
- Keep `mod.rs` and `lib.rs` as declarations/re-exports only.
- Keep every file at or below 400 lines; target 300.
- Put no prompt prose in Rust; edit `crates/z-engine-core/prompts/*.md`.
- Libraries use typed `thiserror` errors; shells may use `anyhow`.
- Guarded mode is opt-in for this Rust-only slice; existing behavior remains unchanged.
- In guarded automatic mode, unmet evidence or verification stops without asking.
- Do not port the Sentinel workspace. Reuse only its CAS, append-only fold, and
  prompt-fragment patterns after adapting them to Z Engine.

---

### Task 1: Introduce the provider seam
**Files:**
- Create: `crates/z-engine-provider/src/transport.rs`
- Modify: `crates/z-engine-provider/src/client.rs`
- Modify: `crates/z-engine-provider/src/lib.rs`
- Modify: `crates/z-engine-core/src/agent/{handle,task,turn,side_requests,subagent}.rs`
- Test: `crates/z-engine-provider/src/transport.rs`

**Interfaces:**
- Produces: `EventStream` and object-safe `ChatProvider`.
- Consumers: all core model calls and Task 8 replay providers.

- [ ] **Step 1: Write a failing provider-seam test**

```rust
#[test]
fn client_implements_chat_provider() {
    fn accepts(_: &dyn ChatProvider) {}
    let client = Client::new("http://localhost:1", None).unwrap();
    accepts(&client);
}
```

- [ ] **Step 2: Run the focused test**

Run: `cargo test -p z-engine-provider transport::tests::client_implements_chat_provider`
Expected: FAIL because `ChatProvider` does not exist.

- [ ] **Step 3: Add and implement the seam**

```rust
pub type EventStream = tokio::sync::mpsc::Receiver<Result<StreamEvent, ProviderError>>;

pub trait ChatProvider: Send + Sync {
    fn stream_chat(&self, request: &ChatRequest, abort: Arc<AtomicBool>) -> EventStream;
    fn set_api_key(&self, key: Option<String>);
}
```

Implement it for `Client`. Replace concrete `&Client` parameters with
`&dyn ChatProvider`; use `Arc<dyn ChatProvider>` where cloning is required.
Add `spawn_with_provider` for tests while preserving existing `spawn` APIs.

- [ ] **Step 4: Verify provider and mocked-loop tests**

Run: `cargo test -p z-engine-provider -p z-engine-core --test agent_loop_mocked`
Expected: PASS.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-provider crates/z-engine-core/src/agent && git
commit -m "refactor(provider): add injectable chat transport"`

### Task 2: Add content-addressed evidence storage
**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/z-engine-core/Cargo.toml`
- Create: `crates/z-engine-core/src/evidence/{mod,blob,record,ledger}.rs`
- Modify: `crates/z-engine-core/src/lib.rs`

**Interfaces:**
- Produces: `BlobHandle`, `BlobStore`, `FsBlobStore`, `EvidenceRecord`,
  `EvidenceLedger::append`, and `EvidenceLedger::read_all`.
- Consumes: repository-relative paths and raw artifact bytes.

- [ ] **Step 1: Write failing CAS and ledger tests**

```rust
#[test]
fn duplicate_bytes_share_one_blob() {
    let store = FsBlobStore::new(tempfile::tempdir().unwrap().path());
    assert_eq!(store.put(b"same").unwrap(), store.put(b"same").unwrap());
}

#[test]
fn ledger_is_append_only_and_ordered() {
    let ledger = EvidenceLedger::open(tempfile::tempdir().unwrap().path()).unwrap();
    ledger.append(&fixture("a")).unwrap();
    ledger.append(&fixture("b")).unwrap();
    assert_eq!(ledger.read_all().unwrap().len(), 2);
}
```

- [ ] **Step 2: Confirm the tests fail**

Run: `cargo test -p z-engine-core evidence::`
Expected: FAIL because the evidence module is absent.

- [ ] **Step 3: Implement typed storage**

Add workspace `sha2 = "0.10"`. Use atomic blob creation at
`.z-engine/runs/<run-id>/blobs/<sha256>` and append one flushed JSON object per
line to `evidence.jsonl`. Reject malformed handles and hash mismatches with a
typed `EvidenceError`; do not silently skip corrupt records.

- [ ] **Step 4: Run evidence tests**

Run: `cargo test -p z-engine-core evidence::`
Expected: PASS, including reopen and corruption tests.

- [ ] **Step 5: Commit**

Run: `git add Cargo.toml crates/z-engine-core && git commit -m "feat(evidence):
add append-only content-addressed store"`

### Task 3: Capture fresh file evidence

**Files:**
- Modify: `crates/z-engine-core/src/tools/context.rs`
- Modify: `crates/z-engine-core/src/tools/read_file.rs`
- Modify: `crates/z-engine-core/src/tools/mod.rs`
- Test: `crates/z-engine-core/src/tools/read_file.rs`

**Interfaces:**
- Produces: `ToolCtx::record_read_evidence` and
  `ToolCtx::fresh_read_evidence(path)`.
- Consumes: `EvidenceLedger`, `FsBlobStore`, file range, content hash, and revision.

- [ ] **Step 1: Write a failing read-evidence test**

```rust
let out = ReadFileTool.run(json!({"path":"src/lib.rs","offset":1,"limit":20}), &ctx).await?;
assert!(out.result.contains("[evidence:"));
assert!(ctx.fresh_read_evidence(Path::new("src/lib.rs")).is_some());
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core tools::read_file::tests::read_records_fresh_evidence`
Expected: FAIL because reads do not create evidence.

- [ ] **Step 3: Record evidence after successful reads**

Extend `ToolOutput` with `evidence_ids: Vec<String>`. Record canonical relative
path, displayed line range, full-file SHA-256, blob handle for the returned
range, acquisition method `read_file`, and current git HEAD or `working-tree`.
Binary and failed reads must not become edit-authorizing evidence.

- [ ] **Step 4: Test stale and truncated evidence**

Add tests proving an external file change invalidates freshness and a limited
read authorizes only its recorded range.
Run: `cargo test -p z-engine-core tools::read_file evidence::`
Expected: PASS.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-core/src/tools crates/z-engine-core/src/evidence
&& git commit -m "feat(evidence): ground file reads in revisioned records"`

### Task 4: Add typed work orders and a pure prompt manifest

**Files:**
- Create: `crates/z-engine-core/src/governance/{mod,work_order,prompt}.rs`
- Create: `crates/z-engine-core/src/tools/set_work_order.rs`
- Modify: `crates/z-engine-core/src/tools/{mod,context}.rs`
- Modify: `crates/z-engine-core/src/agent/{state,turn,prompt_inspect}.rs`
- Modify: `crates/z-engine-core/prompts/system-main.md`

**Interfaces:**
- Produces: `WorkOrder`, `AcceptanceCommand`, `ActiveWorkOrder`,
  `PromptSnapshot`, `PromptManifest`, and `set_work_order`.
- Requires: every writable path references fresh evidence from Task 3.

- [ ] **Step 1: Write failing validation and determinism tests**

```rust
assert!(WorkOrder::validate(order_without_evidence(), &ledger).is_err());
assert_eq!(build_prompt(&snapshot, 25_000), build_prompt(&snapshot, 25_000));
assert!(build_prompt(&snapshot, 25_000).estimated_tokens <= 25_000);
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core governance::`
Expected: FAIL because governance types are absent.

- [ ] **Step 3: Implement the minimal Rust work order**

```rust
pub struct WorkOrder {
    pub id: String,
    pub goal: String,
    pub writable_paths: Vec<PathBuf>,
    pub target_symbols: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub acceptance_commands: Vec<AcceptanceCommand>,
}
```

`set_work_order` validates and stores one active order. The prompt builder emits
stable instructions, order digest, current evidence excerpts, recent failures,
working messages, and tools in canonical order. Return a typed overflow error if
pinned content alone exceeds 25K estimated tokens.

- [ ] **Step 4: Integrate and test**

Run: `cargo test -p z-engine-core governance:: --lib`
Expected: PASS with byte-identical serialization tests.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-core/src/governance crates/z-engine-core/src/tools
crates/z-engine-core/src/agent crates/z-engine-core/prompts/system-main.md &&
git commit -m "feat(governance): add work orders and bounded prompts"`

### Task 5: Enforce localization and repository truth

**Files:**
- Create: `crates/z-engine-core/src/governance/gate.rs`
- Modify: `crates/z-engine-core/src/tools/{context,edit_file,write_file,bash}.rs`
- Modify: `crates/z-engine-core/src/lsp/mod.rs`

**Interfaces:**
- Produces: `GateDecision::{Pass,Fail,NeedsEvidence}` and
  `ToolCtx::authorize_mutation(path, changed_range)`.
- Consumes: active work order, fresh evidence, current hashes, Rust symbols, LSP health.

- [ ] **Step 1: Write failing gate tests**

Test that guarded mode rejects: no work order, out-of-scope path, stale evidence,
unhealthy rust-analyzer, and an unresolved target symbol.

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core governance::gate`
Expected: FAIL because mutation authorization is absent.

- [ ] **Step 3: Implement fail-closed authorization**

Expose `LspClient::health()`. Before writes, require a healthy Rust semantic
provider, scoped path, current hash, covered read range, and resolvable target.
For this slice, guarded mode rejects mutating `bash` commands because their write
set cannot be proven before execution. Return model-visible typed gate failures.

- [ ] **Step 4: Verify gate behavior**

Run: `cargo test -p z-engine-core governance::gate tools::edit_file tools::write_file`
Expected: PASS; unguarded compatibility tests remain unchanged.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-core/src/governance crates/z-engine-core/src/tools
crates/z-engine-core/src/lsp && git commit -m "feat(governance): block
ungrounded Rust mutations"`

### Task 6: Make verification own completion

**Files:**
- Create: `crates/z-engine-core/src/governance/{verify,manifest}.rs`
- Modify: `crates/z-engine-core/src/agent/{events,state,turn,task}.rs`
- Modify: `crates/z-engine-tui/src/headless.rs`

**Interfaces:**
- Produces: `VerificationRunner`, `VerificationManifest`,
  `Event::TurnBlocked`, and guarded `TurnOutcome`.
- Consumes: final diff, `cargo check --workspace --all-targets`, and work-order commands.

- [ ] **Step 1: Write a failing false-completion integration test**

Script a model final answer after a broken edit. Assert no `TurnCompleted` event,
then assert `TurnBlocked { gate: "completion", .. }`.

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core --test agent_loop_mocked guarded_completion`
Expected: FAIL because model final text currently completes the turn.

- [ ] **Step 3: Implement verification-owned completion**

When guarded mode receives a final model answer after mutation, re-check hashes,
run Cargo check and each allowlisted acceptance command with timeout/process-tree
cleanup, then write `verification.json`. Any failure emits `TurnBlocked` and
returns a non-success headless exit. Only a complete manifest permits
`TurnCompleted`.

- [ ] **Step 4: Test pass and fail paths**

Run: `cargo test -p z-engine-core --test agent_loop_mocked guarded_`
Expected: PASS for broken-edit, passing-edit, timeout, and missing-command cases.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-core/src/governance crates/z-engine-core/src/agent
crates/z-engine-tui/src/headless.rs && git commit -m "feat(governance): gate
completion on Rust verification"`

### Task 7: Record and replay complete guarded runs

**Files:**
- Create: `crates/z-engine-core/src/replay/{mod,cassette,recording,replaying}.rs`
- Modify: `crates/z-engine-core/src/lib.rs`
- Modify: `crates/z-engine-core/src/agent/{handle,execute,turn}.rs`
- Test: `crates/z-engine-core/tests/guarded_replay.rs`

**Interfaces:**
- Produces: `RunCassette`, `RecordingProvider`, `ReplayProvider`, `RunMetrics`.
- Consumes: exact serialized requests, normalized stream events, tool outcomes,
  prompt hashes, evidence IDs, gate decisions, and verification manifest hash.

- [ ] **Step 1: Write the failing round-trip test**

Record a guarded edit against a temp Rust fixture, reset the fixture, replay
without HTTP, and assert identical request hashes, tool outcomes, gate decisions,
final diff hash, and metrics.

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core --test guarded_replay`
Expected: FAIL because cassette providers do not exist.

- [ ] **Step 3: Implement strict JSONL cassettes**

Derive serde round trips for request and stream types. Recording wraps any
`ChatProvider`; replay rejects the first request mismatch with its sequence
number and never falls back to network. Append tool and gate records from the
same run recorder. Emit `{model_id,input_tokens,output_tokens,turns,tool_calls,
wall_time_ms,outcome}`.

- [ ] **Step 4: Verify deterministic replay**

Run: `cargo test -p z-engine-core --test guarded_replay -- --nocapture`
Expected: PASS and zero network access during replay.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-provider crates/z-engine-core && git commit -m
"feat(replay): record and replay guarded agent runs"`

### Task 8: Expose the vertical slice and lock its baseline

**Files:**
- Modify: `crates/z-engine-tui/src/{main,headless}.rs`
- Create: `crates/z-engine-core/tests/fixtures/guarded-rust-edit/`
- Create: `crates/z-engine-core/tests/guarded_vertical_slice.rs`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/deviations.md`

**Interfaces:**
- Produces CLI flags: `--guarded`, `--record-run <path>`,
  `--replay-run <path>`, and `--metrics-out <path>`.

- [ ] **Step 1: Write a failing CLI/e2e test**

The fixture must force the agent to read `src/lib.rs`, set a scoped work order,
edit one function, and pass a held-out test. Assert a second-file edit is blocked.

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p z-engine-core --test guarded_vertical_slice`
Expected: FAIL because the guarded runner is not exposed.

- [ ] **Step 3: Wire CLI modes and documentation**

Reject incompatible record/replay flags, require headless mode for cassette
paths in this slice, and never resolve an API key in replay mode. Document the
Rust-only and mutating-bash limitations explicitly.

- [ ] **Step 4: Run complete validation**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
wc -l $(git diff --name-only HEAD~8 -- '*.rs')
```

Expected: formatting clean, zero warnings, all tests green, every Rust file at
or below 400 lines, and live-record/replay manifests identical.

- [ ] **Step 5: Commit**

Run: `git add crates/z-engine-core crates/z-engine-tui docs && git commit -m
"feat: ship Rust evidence-gated vertical slice"`
