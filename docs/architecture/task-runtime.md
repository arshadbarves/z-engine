# Persistent task and execution contracts

Status: proposed contracts for the
[GUI-first architecture](agent-harness.md). None of the states or ports below
should be assumed implemented; see the [baseline](current-state.md).
The implemented subset and its stricter initial Rust check policy are documented
in the [S1 contract](verification-completion.md).

## 1. Task is not session, turn, or attempt

- **Session:** a conversation and its workspace association.
- **Task:** an original goal, clarified requirements, constraints, and completion
  record. A session may contain multiple tasks.
- **Attempt:** one execution/resumption interval with effective settings,
  prompt/strategy versions, budget, and termination reason.
- **Operation:** an identified tool/check/action with declared effects.
- **Evidence:** structured, attributable observations supporting requirements.

Clarifying a task updates versioned requirements; it does not erase the original
request. A new unrelated goal creates a new task. UI session switching or model
switching does not implicitly complete the active task.

## 2. Lifecycle and completion

Use separate task state, current activity, and attempt termination. Do not
encode "verifying and pause requested" as unrelated booleans.

| Task state | Meaning | Legal next outcomes |
|---|---|---|
| `PENDING` | Goal recorded; execution not started | Running, Stopped |
| `RUNNING` | Authorized work may be scheduled | WaitingUser, PauseRequested, Stopping, Interrupted, Blocked, Complete |
| `WAITING_USER` | Identified question or approval blocks progress | Running, PauseRequested, Stopping |
| `PAUSE_REQUESTED` | No new work; active unit is draining | Paused, Stopping, Blocked, Interrupted |
| `PAUSED` | Safe checkpoint durably acknowledged | Running after resume validation, Stopped |
| `STOPPING` | Cancellation requested; outcomes being reconciled | Stopped, Interrupted |
| `STOPPED` | User cancelled this attempt; task is not complete | Running only on explicit continuation |
| `INTERRUPTED` | Crash, transport failure, or uncertain operation | Running after assessment, WaitingUser, Blocked, Stopped |
| `BLOCKED` | Progress unsafe/impossible under current conditions | Running after resolution, WaitingUser, Stopped |
| `COMPLETE` | Requirements assessed and evidence gate committed | New task or explicit reopen invalidating completion |

Activity (investigating, implementing, verifying, reviewing, repairing) is
orthogonal and extensible. Attempt termination records provider failure,
context exhaustion, timeout, budget exhaustion, user cancellation, or completion.

Completion assessment returns one of `COMPLETE`, `CONTINUE`, `REPAIR`, `VERIFY`,
or `ASK_USER`, referencing requirements and evidence. It is a model proposal
validated by the runtime, not an authority to execute arbitrary follow-up work.
After interruption, assessment may record recommended work but must await
explicit continuation before effects; it is not replay.

The Complete gate requires:

1. Every accepted requirement has supporting evidence or a recorded,
   user-approved scope change; unknown coverage is not satisfied coverage.
2. Applicable required checks passed for the current workspace/change manifest.
3. No unresolved operation, failed persistence, pending decision, or stale
   evidence affects the result.
4. The assessment and evidence references are durably stored before the GUI
   receives the Complete event.

Documentation/informational tasks may have checks marked not applicable with a
reason. A user accepting an unverified result is recorded as an explicit
acceptance/scope decision, not fabricated passing checks. Never label that
outcome fully verified.

## 3. Durable records

Define versioned records as they become necessary, not one all-purpose JSON
blob.

| Record | Minimum information |
|---|---|
| Task | ID, original goal, workspace identity, requirement revisions, state, timestamps |
| Attempt | ID, task ID, effective settings/strategy versions, budget, termination reason |
| Operation | ID, parent attempt/step, tool identity, validated input reference, effects, authorization scope, replay classification |
| Operation outcome | Exit/transport status, changed paths/effects, diagnostics, artifact references, known/unknown outcome |
| Check plan | Project profile, selected checks, scope, rationale, required/optional/not-applicable classification |
| Evidence | Check/operation ID, command+cwd, toolchain/config identity, input fingerprint, outcome, timestamps, output references |
| Checkpoint | Last committed sequence, explicit continuation state, pending decisions, workspace versions, managed edit pre/postimages |
| Assessment | Requirement coverage, evidence IDs, unresolved issues, proposed next action, gate decision |

Use stable IDs and a monotonic sequence per task journal. A durable event can
carry a schema version and be projected into a snapshot. The GUI uses task,
attempt, operation, and sequence IDs to reject duplicates and misrouted events.

## 4. Execution protocol

```text
typed proposal
  -> validate task state and capability
  -> resolve effects, paths and current versions
  -> policy decision / identified approval
  -> persist intent + approved scope + replay classification
  -> execute one safe unit
  -> observe postconditions / repository effects
  -> persist outcome + evidence
  -> update state and publish event
```

Tool metadata includes effect kind (read, managed write, process, external),
scope, concurrency, timeout, safe-boundary behavior, and replay safety.
Unknown MCP effects default to gated and serialized, not read-only.
Namespaced external tool identities include server identity to avoid confusing
built-ins and same-named tools.

A managed multi-file edit validates the entire plan before writing, journals
preimages, applies with recoverable progress, and checks expected postimages.
There is no filesystem-wide atomicity claim. On failure, restore only
agent-owned versions or stop with a conflict report.

Arbitrary shell/MCP operations may write outside managed paths or mutate remote
systems. Report rollback coverage explicitly. Workspace leases serialize
agent writes but do not lock out the user's editor; compare versions at apply
and recovery boundaries.

Hooks and check scripts use this protocol too. Tool output is bounded and
schema-checked; `isError`, failing exits, malformed responses, and missing
artifacts cannot become a successful empty result.

## 5. Stop, Pause, and restart

### Stop: cancellation, not rollback

Immediately signal provider requests and cancellable tools; stop scheduling new
operations. Terminate/reap owned processes where supported. Record completed,
cancelled, partial, or unknown outcomes, then acknowledge Stopped once local
execution is quiescent.

Stop does not undo edits or imply a remote request was cancelled. An unresolved
remote effect remains recorded for reconciliation before continuation.
Rollback requires a separate explicit operation.

### Pause: suspend at a safe boundary

1. Record the request and stop scheduling new operations.
2. Finish the active atomic/recoverable tool unit and record its outcome.
3. End/cancel an active model stream at a recorded boundary; discard partial
   unvalidated tool proposals rather than executing them.
4. Persist continuation state, pending decisions, repository versions, evidence,
   and checkpoint commit sequence.
5. Publish Paused only after the durable commit succeeds.

A pending question/approval can be checkpointed without granting it. Resume
must revalidate its scope and versions. Long-running tools have a declared
deadline; if no safe boundary is reached, keep Pause requested, explain the
delay, and offer Stop. Do not falsely acknowledge suspension.

### Restart and recovery

Load the journal, validate versions, reconstruct explicit state, and identify
operations with intent but no durable outcome. Preserve valid legacy transcript
history; label it legacy/unassessed, not verified. Opening a session only loads
state; continuation needs user action.

| Recovery condition | Required behavior |
|---|---|
| Torn final record | Recover only the known-valid prefix; retain/report damaged tail |
| Corruption before final record | Stop recovery and report; do not silently skip facts |
| Managed write matches recorded postimage | Reconcile as applied; do not reapply |
| Managed write matches preimage | Replan/retry only after current authorization validation |
| File matches neither version | Conflict; do not overwrite the user's work |
| Shell/remote effect has no outcome | Inspect external/repository state or ask; never blindly retry |
| Evidence input fingerprint changed | Mark stale and reverify |
| Workspace missing, moved, or changed identity | Ask to locate/confirm; do not bind by display name alone |
| Unsupported record version | Preserve data and report migration requirement |
| Checkpoint/storage failure | Do not acknowledge Paused or Complete; block unsafe progress |

Persisting intent before effects narrows ambiguity but cannot remove the
crash window between an external effect and its outcome record. Idempotency
keys help only when the called system actually supports them.

Storage selection starts with the existing local journal approach. Add schema,
sync/flush boundaries, atomic snapshots, and migrations before considering a
database replacement. Document whether acknowledgement means process-crash or
power-loss durability on supported filesystems; implement the declared
file/directory synchronization protocol and test it.

## 6. Verification evidence and freshness

A check outcome is `PASSED`, `FAILED`, `BLOCKED`, `CANCELLED`, `STALE`, or
`NOT_APPLICABLE`, with reasons. The verifier owns result interpretation;
the model may choose among applicable checks but cannot turn failure into pass.

Record actual commands, cwd, exits, parsed diagnostics and raw-output references.
A compiler/parser that emits no diagnostics after a spawn or build failure
still failed. Output truncation must preserve a usable evidence artifact or
record the artifact loss.
Artifacts needed by committed results belong in durable task storage, not
temporary spill paths. Retention/deletion must surface loss of supporting
evidence rather than continuing to imply that it remains inspectable.

Bind evidence to the task's change manifest and relevant source, config,
dependency, toolchain and workspace identities. Track pre-existing user changes
separately from observed agent effects. Checks may generate output files, but
source/config changes during a check invalidate its input fingerprint.

Prefer affected-package/test selection. Expand scope for shared APIs,
dependencies or uncertain impact. A check cannot claim coverage of paths it
never inspected. Recheck fingerprints at the completion gate so an edit after
a green test invalidates the evidence.

The completion view shows requirement coverage, changes, checks actually run,
failures/blocks, freshness, and residual limitations. A model's final prose
cannot directly set the success badge.

## 7. Structured questions

A question record includes ID, task/attempt, one decision, reason it blocks,
choices with stable IDs, optional recommendation, free-text allowance, and any
related plan/versions. The GUI renders it through existing primitives.

- Persist before displaying; answers reference the question ID.
- Handle duplicate, stale, cancelled, or wrong-task answers explicitly.
- Support custom answers and dismissal without silently choosing a default.
- Revalidate the affected plan after an answer or resume.
- Ask when ambiguity changes scope, authority, irreversible effects, or
  correctness. Otherwise choose a reasonable implementation and record it.

Question prompts and tool approvals are distinct: answering a design question
does not grant permission to execute every implied operation.

## 8. Experience, privacy, and observability

Experience references the completed task's evidence and scope. Redact secrets;
store concise approaches rather than full transcripts. Retrieval cannot
override current repository facts, user instructions, or policy. Outdated or
contradicted experience is downranked or retired with provenance.

Record observable decisions, not private chain-of-thought. Logs correlate task,
attempt, operation, policy, evidence, and checkpoint IDs, with bounded/redacted
payloads. Proposed reliability measures include false-completion rate on seeded
failures, recovery without duplicate effects, cancellation acknowledgement
latency, time-to-safe-pause, and verification cost.

Each [slice](../roadmap/agent-harness.md) must include deterministic negative
tests and persistence/replay assertions before claiming these guarantees.
