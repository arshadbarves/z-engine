# Grounded context packets

## Implemented slice

[`z-engine-context`](../../crates/z-engine-context/) is a deterministic, pure
projection of harness-owned task observations plus explicitly unverified model
notes. It depends only on workspace `serde`, `serde_json`, and `thiserror`.
There is no provider, UI, filesystem access, clock, model call, retrieval
service, repository scan, or completion decision in this crate.

The core [adapter](../../crates/z-engine-core/src/context/task_packet.rs)
clones the current `ToolCtx.task` report under its lock, releases the lock,
projects it, and reads the supplied `NotesStore`. Missing tasks, poisoned task
locks, unsupported report schemas, invalid protected identities, and JSON
serialization errors return typed errors. It does not refresh verification.

### Protected data and optional context

The packet preserves the original report goal, every active requirement,
observed task status, all blockers, runtime supervision, and every evidence
reference verbatim.
References contain real evidence IDs, observed outcomes, recorded input
fingerprints, check start/duration, exit codes, and test counts. Unknown
fingerprints remain null. Task ID, report schema version, workspace root,
packet schema version, and source provenance remain present.
Every emitted packet has the discriminator `kind: "task_context"` for request
inspection; this is structured task data, not an additional user instruction.

When present, `harness.supervision` preserves the runtime report's
`continuations`, `maxContinuations`, `lastAction` (a snake_case action value),
and full `reason`. The adapter serializes this runtime-owned structure with
`serde_json::to_value`, propagating errors; the context crate has no runtime
dependency and does not reinterpret the action. Supervision is protected even
when its reason alone exceeds the target. Absence remains absence, including
when reading older packets that have no supervision field.

The original goal and active requirements can intentionally duplicate recent
user-message content: they are protected anchors for transcript compaction,
not new requests or a change in scope.

Compaction summaries are also retained in `modelNotes` before optional entries:
they replaced prose in working history, so evicting them would remove both
representations. This retention guarantee does **not** elevate their trust or
move them into harness observations. Summaries can cause explicit budget
overflow; cumulative-summary recompression is not implemented.

Optional entries are selected whole, deterministically:

1. Evidence details: non-passing checks first, then passing checks; within each
   group, reverse report order. Details retain the evidence ID, recorded
   command, working directory, toolchain, summary, and artifact paths/digests.
2. Changed paths, in report order.
3. Other model notes, in caller order. The core adapter prioritizes needs-later,
   decisions, then progress; newest entries first within a category.

An entry that does not fit is skipped; a later smaller entry may still fit.
Evidence references are never discarded when their optional details are
omitted. Omission counts cover evidence-detail records, changed paths, and
model notes independently.

The target measures the **entire compact JSON packet in UTF-8 bytes**, including
escaping, metadata, counters, and the byte count itself; it is not a token
estimate. `budget.serializedBytes` is exact for `to_json()` output.
Protected content exceeding the target is returned intact with
`budget.overBudgetBytes > 0`; all optional entries are omitted. The caller
must handle that condition explicitly if it needs a hard transport limit.
Zero and maximum-size targets are supported. Treat a built packet as immutable;
modifying its public data after construction invalidates its budget accounting.
There is no fixed protected-content size cap or size-based rejection in the
builder or adapter. The required `target_bytes` parameter lets the parent
allocate context based on `cfg.max_context_tokens` and the rest of the request.
For example, a request larger than 15 KiB duplicated in its goal requirement
remains intact even when the selected optional-context target is only 8 KiB;
that target yields an explicit overflow report, not an error or truncation.
Do not turn a small fixed packet target into a hard rejection of an otherwise
supported request. Any true provider-limit error must be explicit and account
for the whole request, including intentional duplicates; bytes are not tokens.

### Authority and freshness

The harness section is authoritative about what the current report records at
its observation time, **not** about the current filesystem or continuous
refresh. `freshness = "report_snapshot_not_revalidated"` applies to the whole
projection. `reportObservedAtMs` is null because `TaskReport` has no report-level
observation timestamp. Individual check timestamps are copied, not relabeled as
report refresh times. Input fingerprints describe checked source versions, not
a newly calculated workspace fingerprint. There are no confidence scores.

Notes, hypotheses expressed in notes, and compaction/legacy summaries have
`trust = "unverified"` and separate model provenance. They cannot create an
evidence reference, change status, clear blockers, or replace goals or
requirements. The completion-assessment proposal is intentionally not projected:
model-written assessment explanations are not harness observations. Runtime
supervision remains owned by its report/runtime surface; the packet only
projects that recorded decision and never increments counters or decides to
continue.

The existing [notes store](../../crates/z-engine-core/src/context/notes.rs)
now labels its display block unverified. Its persisted shape, legacy tool
payload parsing, droppable IDs, and summary handling are unchanged. Even legacy
summary text claiming to be authoritative stays data inside an unverified note.

## Runtime integration

The workspace and core dependency register the crate. Per-round
[request assembly](../../crates/z-engine-core/src/agent/request.rs) replaces
system-role notes with a fresh structured packet as a **user** message after
working history. Packets are not appended to the durable conversation.
Repository maps are also lower-trust user-role data.

[Context guidance](../../crates/z-engine-core/prompts/context-packet.md) and
[supervision guidance](../../crates/z-engine-core/prompts/task-supervision.md)
are loaded through the prompt registry into the stable system prefix.
After a durable supervision decision, the next packet carries it without
fabricating a user request.

API:

```rust
build_task_packet(ctx: &ToolCtx, notes: &NotesStore, target_bytes: usize)
    -> Result<z_engine_context::ContextPacket, TaskPacketError>

task_packet_json(ctx: &ToolCtx, notes: &NotesStore, target_bytes: usize)
    -> Result<String, TaskPacketError>
```

The byte target uses the configured context-token count as a byte allocation,
clamped to 4-64 KiB; it is not a tokenizer guarantee. Protected overflow produces
a status notification and remains explicit in the packet. No notes/task mutex
is held across `.await`. Verification refresh remains at lifecycle boundaries,
not in this per-round projection.

Targeted validation is:

```sh
cargo test -p z-engine-context
cargo test -p z-engine-core --lib context::
```

Focused tests cover protected-content overflow, exact JSON byte accounting,
deterministic selection, skipped oversized entries, omissions, source
fingerprints/artifact references, unknown freshness, legacy-note trust,
fabricated success claims, and poisoned/missing task state.

## Fail-closed compaction

The [planner](../../crates/z-engine-core/src/context/compact.rs) now returns
`Result<CompactionOutcome, CompactionError>`. Tool-result replacements are
published only after writing and syncing the full spill. Spill failures expose
their path and I/O cause rather than emitting an `<unavailable>` pointer.
Eager elision stages all replacements before mutating the transcript, so a
later failure leaves every original message intact. An already-elided result
is reused only in its complete marker form with an accessible regular backing
file; embedded markers cannot discard surrounding output.

The [agent commit boundary](../../crates/z-engine-core/src/agent/compaction.rs)
requires a nonempty summary, a healthy notes lock, and a successful
`record_durable` before replacing prose in the working transcript. The next
packet retains that replacement even over its byte target. Empty or
failed summaries, absent recorders, lock poisoning, and storage failures
retain the original working context and notes and return typed errors.
Successful summaries remain unverified model notes, not authoritative facts.
The summarizer shares the active Stop flag and has a 60-second deadline,
64 KiB collected-output bound and 2,048-token requested output ceiling.
Cancellation or a failed bound preserves the original context.

Prose is selected in whole-message units under a 12,000-character input
ceiling matching the current summarizer. Messages outside that ceiling stay
verbatim rather than being silently clipped. Image messages and narration on
tool-call anchors also stay verbatim because they are not represented by that
text-only summarization request. Tool-only compaction can proceed without a
model summary once its original output is safely stored.

### Compaction integration

The turn loop uses `elide_marked_outputs`, which clears droppable IDs only
after all spills succeed and surfaces notes-lock errors. Compaction errors
interrupt the task without replacing its original context. The summarizer and
planner share the same whole-message character ceiling.

Additional targeted validation:

```sh
cargo test -p z-engine-core --lib compact
```

## Future roadmap, not implemented

Cross-task cognitive memory, accepted/verified experience records, semantic
retrieval, embeddings, a vector database, graph-based dependencies, automatic
contradiction resolution, learned ranking, and cross-session memory promotion
are **not implemented**. Any future retrieval must retain original goals,
source identity/version, explicit trust boundaries, deterministic budget
reporting, and the runtime's exclusive completion authority. Do not treat
model notes as verified durable knowledge merely because they survived
compaction or were retrieved repeatedly.
