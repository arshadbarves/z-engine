# S1 verification and completion contract

S1 implements the first evidence-backed GUI slice from the
[roadmap](../roadmap/agent-harness.md). The subsequent
[supervision/context slice](supervised-harness.md) adds bounded automatic
continuation. Durable Pause, a generic strategy engine and cross-language
completion coverage remain planned.

## Task versus response

Each submitted goal creates a versioned task report with a stable task ID and
an immutable requirement named `goal`, containing the original request.
The vector of requirements and explicit evidence references are the extension
seam for future task decomposition; the model cannot replace that original
requirement through the assessment tool.

`TurnCompleted` retains its existing response-lifecycle meaning. It closes
streaming and clears busy state; it does not certify the task. Only a
`TaskUpdated` report with status `complete`, emitted after durable recording,
means that the S1 completion gate passed.

The [verification module](../../crates/z-engine-core/src/verification) owns
typed check specifications, execution evidence, workspace observation and the
deterministic assessment gate. It has no GUI or agent-loop dependency.
The [task completion boundary](../../crates/z-engine-core/src/agent/task_completion.rs)
integrates that contract with runtime state and persistence.

## Verification tools

`run_verification` accepts a typed specification:

```json
{"kind":"cargo_test","package":null,"filter":null}
```

Kinds are `cargo_test` and `cargo_build`. Package/test filters support targeted
investigation, but **S1 requires a full-workspace test run for Complete**.
This strict policy was selected deliberately until affected-package coverage
analysis can safely narrow the completion check.

The full-workspace profile runs `cargo test --workspace --no-fail-fast`,
including Cargo's default unit, integration and documentation tests.
`--all-targets` is not a substitute: Cargo excludes doctests with that option.
Build-only, package-filtered and test-filtered checks cannot satisfy this gate.

Checks run through the normal tool permission gate. Plan/read-only permission
mode cannot run them: build scripts and tests execute repository code.
Neither a shell transcript saying tests passed nor an external tool's claim
can manufacture verification records.

Each result records its ID, exact argument vector, working directory, toolchain,
input fingerprint, timestamp/duration, exit code, observed test count, outcome,
summary and checksummed stdout/stderr artifacts. Artifacts live beside the
session in its `.artifacts` directory, not a disposable tool-output spill.

Check outcomes distinguish passed, failed, blocked, cancelled and stale.
A nonzero exit is a failure even if there are no parsed diagnostics. Zero
observed tests cannot satisfy the full-workspace test gate. Files changing
during verification invalidate its evidence.

## Completion assessment

After implementation and passing verification, the model supplies a proposal:

```json
{
  "summary": "The original bug is fixed and verified.",
  "coverage": [{
    "requirementId": "goal",
    "evidenceIds": ["an-actual-harness-generated-check-id"],
    "explanation": "Describe how the observed checks cover the original goal."
  }]
}
```

`assess_completion` cannot grant permissions or mark a task durably Complete.
The runtime reobserves the workspace and invokes the deterministic gate after
the model response ends.

Complete requires all of the following:

- A passing full-workspace Cargo test run with exit zero and executed tests.
- Current source/configuration and toolchain identity for referenced evidence.
- Present, intact supporting output artifacts.
- Explicit, nonempty coverage of every original requirement using real passing
  evidence IDs, with a summary and explanation.
- No unresolved later failure, denied check, unknown effect scope, or storage
  failure blocking the result.
- A successful durable append of the final task report before notifying the GUI.

Managed edits invalidate previous assessment and passing evidence. A new check
and assessment are required after editing. Known read-only tools remain usable;
opaque shell/MCP effects and outside-workspace edits conservatively block S1
certification rather than claiming whole-workspace tests cover unknown effects.
MCP cannot replace reserved built-in tool names.

This is evidence of the executed checks and model-assessed requirement coverage,
not a proof of arbitrary program correctness.

## Persistence and replay

Task snapshots are journaled at intake, around tool batches and at the terminal
response boundary. Existing conversation events remain readable. A failed
recorder prevents a later completion acknowledgement; Stop and provider failure
produce stopped/interrupted task outcomes, not Complete.
Stop is rechecked during final assessment and immediately before the completion
commit; cancellation is not treated as a successful response.

The GUI renders reports from the same schema for live events and replay, with
checks, blockers, coverage, changed paths and artifact references. Reopening a
session revalidates evidence without rerunning commands. Stale or missing
evidence loses its current Complete status. Old sessions with no task report
are unassessed, not retroactively verified.
After a restart, unfinished work is interrupted; reattaching to a still-live
session preserves its active state. Parked completion reports show freshness
unknown until backend revalidation, rather than recycling a green badge.

S1 stores reports, not suspended continuations. A subsequent user submission
creates another task record; durable resumption of the original execution
belongs to S4/S5. The original report and goal remain in history.

## Boundaries and extension rules

- Add future check kinds behind the verification contract, not special GUI
  success branches or assistant-text parsing.
- Preserve process exit status independently of parsed diagnostics.
- Do not weaken the full-workspace requirement without an explicit, tested
  coverage plan describing which changed inputs each check covers.
- Keep ephemeral model proposals separate from the authoritative persisted
  task outcome. An assessment tool returning is not a commit.
- Treat unsupported workspace layouts, missing tooling, opaque effects and
  unavailable storage as explicit limitations. Informational/non-Rust tasks
  can still receive useful responses without a verified Complete badge.
- Freshness is checked at verification, completion and session reload; this
  slice does not add continuous filesystem monitoring after completion.
- Reuse existing managed-edit checkpoints for diffs, without claiming they
  implement durable rollback or cover arbitrary shell/remote effects.

Regression coverage lives in verification unit tests, runtime/persistence
tests, the [mocked completion integration flow](../../crates/z-engine-core/tests/verification_completion.rs),
and frontend event/replay/session tests. Live provider agreement is not a gate.

## S1 acceptance evidence

- The mocked-provider integration flow runs real Cargo checks in an isolated
  workspace: failing test, managed fix, passing workspace tests, explicit
  coverage and durable Complete. It reads the journal when Complete arrives
  and checks that persisted reports and artifact digests match.
- Negative cases cover unsupported narrow/build-only evidence, failing
  doctests, edits after verification, denied checks after prior assessment,
  opaque effects, provider failure, Stop during streaming/final assessment,
  and missing or failed recording.
- [GUI replay tests](../../crates/z-engine-gui/src-tauri/src/session_store_tests.rs)
  distinguish live reattachment from restart and retain freshness validation.
  Restart interruption remains durable across subsequent live reattachment.
- Frontend tests cover report guards, event/replay parity, parked sessions,
  submission routing and sidebar outcomes. Browser-rendered fixtures confirm
  Unassessed after response-only completion, Complete from a report, Stale,
  Freshness unknown, keyboard disclosure and bounded horizontal layout.
  These are UI smoke checks, not a live-provider or native desktop acceptance run.

Validation commands are `cargo test --workspace`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the GUI package's
existing `test`, `lint` and `build` scripts.

At S1 delivery, the GUI `check` script still reports three pre-existing errors:
the `Catalog` type import in
[ComposerBar](../../crates/z-engine-gui/ui/src/components/chat/ComposerBar.svelte),
the unused `Terminal` import in
[PromptInspectSidebar](../../crates/z-engine-gui/ui/src/components/overlays/PromptInspectSidebar.svelte),
and the unsupported toast tone in
[ProviderConnectModal](../../crates/z-engine-gui/ui/src/components/settings/ProviderConnectModal.svelte).
Those components and their relevant type definitions were not changed by S1.
