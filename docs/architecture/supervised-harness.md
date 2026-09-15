# Supervised engineering harness

Status: first implementation slice, not an AGI claim.

The product is a GUI-first coding harness. The model chooses an engineering
approach; deterministic code controls authority, execution, evidence and task
outcomes. This document distinguishes the delivered boundary from the broader
[target architecture](agent-harness.md) and [delivery roadmap](../roadmap/agent-harness.md).

## Implemented boundaries

| Owner | Responsibility |
|---|---|
| `z-engine-runtime` | Provider/UI-independent bounded continuation decisions and serializable supervision reports |
| `z-engine-context` | Bounded context packets distinguishing protected task data, observed evidence and unverified model notes |
| `z-engine-project` | Read-only discovery of project manifests and existing check capabilities across language ecosystems |
| `z-engine-core` | Integration with the existing agent/tool loop, permission gates, verification, journal and prompt assembly |
| `z-engine-provider` | Model transport |
| GUI | Settings, task projections, supervision/evidence disclosure and session replay |

These are used boundaries, not empty framework crates. Core remains larger
than the target architecture; further extraction follows working slices rather
than creating parallel implementations. There is no maintained terminal or
public headless product. The agent's shell tool and developer build commands
are still necessary capabilities, not alternative product frontends.

## A response boundary is not a completion boundary

The inner loop still lets the model select tools dynamically. When a response
ends, the supervisor can request another model round under the **same task ID**
and original requirement:

1. Observe the current workspace and assess recorded evidence.
2. Honor cancellation before any continuation.
3. Refuse continuation across policy, unknown-effect, storage or check blockers.
4. If supported code changed or a verification check was attempted, determine
   whether verification, failure investigation, or requirement coverage remains.
5. Compare the observed workspace/check state with earlier boundaries.
6. Record the supervision decision durably through `TaskUpdated`.
7. Assemble the next task-context packet with that decision.
8. Continue only when authorized by the bounded supervisor.

The GUI receives one response-lifecycle completion at the end, not one for
each internal continuation. Only the existing final evidence gate can commit
a Complete task status. No model or reviewer can manufacture passing evidence.

### Limits and intentional scope

- `max_task_continuations` defaults to 3, accepts 0 through 10, and is captured
  when a GUI session starts. Zero disables automatic continuation.
- Repeated or cyclic workspace/check states stop continuation. Evidence IDs,
  timestamps and model summaries are excluded from the progress signature.
- The existing tool-round safety limit remains in force across continuations.
- Malformed-only tool responses receive at most two correction opportunities
  with paired error results; repeated protocol failure ends Interrupted.
- Permission denials are not retried automatically. Cancellation never starts
  another attempt. A budget/non-progress stop remains explicitly incomplete.
- Read-only/informational responses do not automatically loop merely because
  they have no tests. The first slice activates on observed code changes/checks,
  not a heuristic that guesses intent from user text. A premature answer before
  any such activity remains unverified, but does not yet get auto-replanned.
- S1's current full-workspace Cargo evidence requirement is unchanged.
  Discovering another language does not silently weaken that completion gate.
- A journal preserves reports, not suspended execution. Restart reconstructs
  interrupted history; durable Pause/Resume and operation reconciliation remain
  subsequent slices. Opening a session never repeats side effects.

The report's optional `supervision` field is additive to schema version 1.
Legacy reports without it remain readable. It contains continuation count,
configured maximum, last decision and a human-readable reason. A `complete`
supervision proposal is not a Complete task status: finalization must still
revalidate and durably record the outcome.

## Context: evidence, not a simulated brain

The context boundary makes a useful distinction:

- Original goal and accepted requirements are protected task data.
- Check outcomes and evidence references are observations from the harness.
- Model notes, summaries and hypotheses remain attributed, unverified data.

Optional material is budgeted deterministically; omission is explicit. Source
fingerprints are recorded evidence identity, not a claim of continuous live
freshness. Final verification still checks current inputs. Context retrieval
cannot grant authority or override the user's task.

Committed compaction summaries cannot be evicted by the optional budget after
their source prose is removed. They remain unverified and may cause explicit
overflow rather than silent loss.

The next model round receives the current task packet even when conversation
compaction has removed old prose. This prevents a closing response from being
the only visible indication of what is left to do.

See [grounded context](grounded-context.md) for the packet contract and
limitations. Semantic dependency retrieval, embeddings, contradiction
resolution and project/global experience are future capabilities, not hidden
features of this initial packet builder.

## Tools and language support

`inspect_project` discovers manifests and available project check suggestions
without executing them. It goes through the normal tool registry and is
read-only. Existing file/search/managed-edit/shell tools remain language-neutral.

Language support has separate levels:

1. General repository inspection and managed text operations.
2. Build/test profile discovery and available project commands.
3. Language-server/compiler-backed semantic operations, only where supported.
4. Typed verification evidence and completion coverage, currently S1 Cargo.

These levels must not be conflated. A discovered command has not run. A parsed
file has not been type-checked. A textual replacement is not a semantic rename.
An unsupported project is not verified because the model believes its edit is
correct. See [project capabilities](project-capabilities.md).

## Prompt methodology and advisory review

Main guidance now addresses task interpretation, source-grounded investigation,
scope, hypothesis testing, appropriate delegation, verification and escalation.
Continuation guidance is separate Markdown data. Runtime decisions are
structured context, not fabricated user requests.

Review distinguishes exact clean verdicts, findings, unavailable/empty results,
timeouts and cancellation. A review mentioning the clean marker alongside a
finding is not clean. An unavailable enabled reviewer is surfaced and blocks
certification rather than appearing as "no findings." Review remains advisory:
it cannot supply test evidence or independently inspect unseen code.

Provider requests are cancelled locally when their receiver is dropped or the
abort flag is raised, including stalled response headers, SSE reads and retry
backoff. This bounds the lifetime of an abandoned reviewer request without
cancelling the main task. It cannot guarantee that a remote provider stops
server-side computation or billing after a disconnected request.

Reviewer, summarizer and title requests share a bounded text collector.
Summarization shares the active Stop flag; timeouts, output overflow and
transport failures retain original context rather than returning a fake summary.

## Validation contract

The supervisor's pure tests cover budget enforcement, repeated/cyclic states,
disabled behavior, explicit cancellation, blockers and wire shape.

Mocked-provider integration tests exercise real managed edits and Cargo checks:

- premature closing response followed by verification and durable completion;
- same task identity and original requirement across internal continuations;
- repeated no-progress answers and budget exhaustion;
- denied verification and Stop during continuation;
- provider failure remaining Interrupted rather than Complete;
- malformed tool correction and bounded protocol failure;
- unchanged read-only responses remaining single-response interactions.

Context/discovery crates have fixture-based unit tests. GUI tests validate both
legacy reports and supervision payloads, settings, and live/replay projections.
An isolated browser component fixture also exercises the real Svelte supervision
details and settings: default count, keyboard focus, out-of-range validation,
zero-as-disabled, and no horizontal overflow at 360px. This is not a native
Tauri launch or a live-model acceptance test.
These scenarios demonstrate bounded behavior; they do not measure general
autonomous task success or establish parity with another coding product.

## Next experiments

1. Explicit task-intent/accepted-requirement contracts and structured questions.
2. Durable operation journal, safe Pause/Resume and crash reconciliation.
3. Source-linked repository retrieval and affected-check coverage.
4. One safe semantic rename with version-checked managed application.
5. Additional typed verification profiles, beginning with TypeScript/Svelte.
6. Scoped specialists with measured benefit over the single-agent baseline.
7. Consent-based project experience, then optional global promotion.

Use held-out tasks and fault injection to compare completion correctness,
unintended changes, lost requirements, stale context, recovery, cost and required
human interventions. More prompts or agents are not evidence of improvement.
