# Large-Codebase Hallucination Resistance

**Status:** Approved design | **Date:** 2026-08-29 | **Target:** Z Engine on
1-5 million LOC polyglot monorepos

## 1. Purpose

Large repositories expose three distinct coding-agent failures:
1. **Wrong localization:** editing the wrong file, symbol, package, or dependency.
2. **Unsupported assumptions:** inventing APIs, symbols, behavior, or architecture.
3. **False completion:** reporting success with failed verification or unmet criteria.

All three are equal hard gates. A model may propose an unsafe action, but it must
not cross the governed edit or completion boundary undetected.
Reliability takes priority over speed and cost. Every code-changing task,
including a trivial fix, requires a typed work order. In fully automatic mode,
the harness never asks questions: an unmet gate produces a structured safe stop.

## 2. Evidence and Approach

Research consistently indicates that:

- long context and relevant distractors degrade retrieval and reasoning;
- repository localization is separate from patch-writing ability;
- graph-guided localization improves downstream repair;
- small errors compound over long tasks;
- visible tests and model review do not reliably establish correctness; and
- truncated or stale tool output can trigger unsupported assumptions.

The design combines:

1. a high-fidelity **Semantic Evidence Plane** that discovers repository truth;
2. a **Deterministic Governance Plane** that controls editing and completion.

An LLM critic may add findings but cannot waive a deterministic failure.

## 3. Goals and Non-Goals

### Goals

- Ground edits in fresh evidence and enforce scope below the model.
- Reject unresolved references before patch acceptance.
- Keep prompts bounded and make gate decisions replayable.
- Require healthy semantic providers for edited languages.
- Reserve `Completed` for verified tasks and fail closed otherwise.

### Non-goals

- Preventing incorrect model text or treating confidence as calibrated.
- Replacing deterministic checks with an LLM quorum.
- Sending whole repositories or raw transcripts to a model.
- Authorizing edits from text or tree-sitter alone.
- Claiming universal zero hallucinations from a finite evaluation.

## 4. Invariants

1. **Semantic evidence authorizes edits.** Every edited path and target symbol
   has fresh evidence from the pinned repository revision.
2. **Discovery is not authorization.** Tree-sitter and text search locate
   candidates; only a healthy language adapter authorizes edits unless the user
   has recorded an explicit override.
3. **Scope is enforced below the model.** Patch application rejects writes
   outside the work order.
4. **Evidence is immutable and revision-scoped.** Changes append invalidation or
   supersession records; stale evidence cannot authorize mutation.
5. **Completion belongs to the harness.** Model output cannot transition a task
   to `Completed`.
6. **Missing truth fails closed.** Missing semantic, storage, or verification
   data is never interpreted as success.
7. **Prompt size is repository-size independent.** Full evidence stays in
   durable storage; prompts contain bounded excerpts and handles.
8. **One task has one mutation authority.** Read-only work may be concurrent;
   task mutations are serialized through one work order and amendment chain.

## 5. Architecture

### 5.1 Semantic Evidence Plane

The evidence plane indexes workspaces, build targets, symbols, exact source
ranges, imports, references, call and implementation edges, dependencies,
associated tests, diagnostics, content hashes, revisions, and adapter health.

It returns bounded graph results and evidence handles, not repository dumps.

### 5.2 Deterministic Governance Plane

The governance plane controls work orders, evidence freshness, scope, patch
authorization, repository truth, verification, task transitions, and safe stops.

It contains no model calls. Models propose typed inputs; deterministic code
validates them.

## 6. Components

### 6.1 Repository Coordinator

Resolves workspace roots, pins revisions, detects languages and build systems,
selects adapters, schedules indexing, and reports repository drift. It does not
parse code or call models.

### 6.2 Language Adapter Registry

Every edit-capable adapter provides:

```text
health, index, definition, references, callers, callees,
imports, implementations, diagnostics, format, tests_for
```

Adapters use LSP plus language-specific build metadata. Tree-sitter provides
fast structural discovery and range validation. Text search is discovery-only.

Health is explicit:

- `Healthy`: may issue edit-authorizing evidence.
- `Degraded`: discovery only; editing requires a recorded override.
- `Unavailable`: editing requires a recorded override.

The default blocks `Degraded` and `Unavailable`. Fully automatic runs cannot
request an override and therefore stop safely.

### 6.3 Semantic Graph

The graph stores stable symbol identities and revision-scoped edges. Bounded
queries include:

```text
impact_radius, tests_for, path_to_symbol,
dependencies_of, dependents_of, why_relevant
```

Updates are content-hash driven. A changed file invalidates affected incoming
and outgoing facts before replacement facts become visible. Queries never mix
known-stale and current edges.

### 6.4 Evidence Store

Append-only typed records cover evidence, invalidations, tool and model
exchanges, work orders, prompts, patches, gate decisions, verification, and
state transitions.

Large content uses a SHA-256 content-addressed blob store. Records reference
blobs rather than duplicate content.

```text
Evidence {
  id, task_id, claim_kind, source_file, source_range,
  symbol_id, content_hash, repository_revision,
  acquisition_method, adapter_id, acquired_at
}
```

Retention must preserve active-task evidence, committed work orders,
verification manifests, and artifacts needed by retained replays.

### 6.5 Prompt Builder

The Prompt Builder is pure:

```text
build(TaskSnapshot, PromptBudget) -> PromptManifest
```

Identical snapshots and budgets produce byte-identical manifests containing:

1. stable instructions;
2. work-order digest;
3. relevant semantic skeleton;
4. selected fresh evidence;
5. recent actions and gate failures;
6. tools and required output schema.

Each fragment records origin, hash, evidence handle, token estimate, priority,
freshness, and eviction eligibility. Instructions, work order, active failures,
and currently required evidence cannot be evicted. Full content is retrievable
by handle.

### 6.6 Work Order

Every code-changing task has:

```text
WorkOrder {
  id, task_id, goal, non_goals, acceptance_criteria,
  target_symbols, readable_scope, writable_scope,
  expected_tests, invariants, supporting_evidence_ids,
  risk_level, repository_revision
}
```

Trivial tasks use the same contract with smaller collections. Validation
requires existing or justified targets, fresh evidence, minimal writable scope,
graph-associated tests, checkable criteria, and defined invariant evaluators.
Scope expansion appends an evidence-backed amendment.

### 6.7 Gate Engine

Predicates return `Pass`, `Fail`, or `NeedsEvidence`. Missing data never becomes
`Pass`.

**Localization Gate**

- Paths and symbols are listed in the work order.
- Symbols resolve through healthy adapters.
- Targets have fresh evidence.
- The patch touches no unlisted semantic target.

**Repository-Truth Gate**

- Symbols, APIs, imports, signatures, and dependencies resolve in the current
  graph.
- Evidence hashes match current content.
- Text candidates have semantic confirmation.
- Concurrent changes invalidate authorization before application.

**Completion Gate**

- Changed files re-index successfully.
- Formatting and required static checks pass.
- Direct and policy-required broader tests pass.
- Every acceptance criterion and invariant passes independently.
- The diff matches the work order and amendments.
- No required verifier is skipped, unavailable, timed out, or unresolved.

### 6.8 Verification Runner

Verification commands have hard timeouts, process-tree cleanup, separate exit
code/stdout/stderr, bounded live output, durable logs, and explicit retry/flake
policy. Failed, timed-out, flaky, or unavailable verification does not pass.

An optional LLM critic runs after deterministic verification. It may reject but
cannot override a deterministic rejection.

## 7. Task State Machine

```text
Received -> Indexed -> Exploring -> WorkOrderReady
         -> Editing -> Verifying -> Completed
```

Any non-terminal state may become `Blocked`; only `Verifying` may become
`Completed`.

### Received and Indexed

Pin the revision, detect workspaces and languages, check adapter health, and
establish the graph snapshot. An unhealthy required adapter blocks fully
automatic mode with `SemanticProviderUnavailable`.

### Exploring

Only read-only tools are available. The agent must collect evidence for the
implementation point, impact radius, affected tests, invariants, and relevant
configuration. Searches return ranked symbols and handles; reads use bounded
ranges. Time, query, and token budget exhaustion yields `InsufficientEvidence`.

### WorkOrderReady

The model proposes a work order from collected evidence. Interactive modes ask
only for ambiguity, override, scope expansion, or policy adjudication. Fully
automatic mode advances only when all predicates pass.

### Editing

Each mutation supplies its work-order ID, evidence IDs, expected hashes, and
semantic targets. The harness checks freshness immediately before application.
Drift returns the task to `Exploring`; new scope requires an amendment.

### Verifying

1. Re-index changed files.
2. Evaluate repository truth.
3. Format and run static checks.
4. Run graph-selected direct tests.
5. Run policy-required broader tests.
6. Evaluate each criterion and invariant.
7. Compare the diff with the work order and amendments.
8. Optionally run an LLM critic.

### Completed and Blocked

Completion emits a durable manifest:

```text
VerificationManifest {
  task_id, work_order_id, revisions, final_diff_hash,
  semantic_gate_results, commands, exit_codes, test_results,
  acceptance_results, invariant_results, evidence_ids, completed_at
}
```

A blocked report states the exact gate and predicate, attempted acquisition,
remaining uncertainty, artifact hashes, and safe next actions. In fully
automatic mode, `Blocked` is terminal: no question, exception, speculative edit,
or success-shaped response.

## 8. Failure Handling

| Failure | Behavior |
|---|---|
| Repository drift | Invalidate evidence and return to `Exploring` |
| LSP crash | Bounded restart; otherwise `Blocked` |
| Degraded adapter | Block unless a prior override applies |
| Truncated output | Preserve full blob and mark excerpt incomplete |
| Needed omitted content | Fetch and verify the full blob |
| Verification timeout | Record output and block |
| Flaky test | Bounded policy reruns; unresolved flakiness blocks |
| Provider failure | Preserve state and retry; never invent missing output |
| Hash mismatch/corruption | Quarantine artifact and fail closed |
| Exploration exhaustion | Emit `InsufficientEvidence` |

## 9. Concurrency

Indexing and evidence acquisition may run concurrently across independent
partitions. Each task has one snapshot, work order, amendment chain, and
serialized mutation stream. Concurrent tasks require workspace isolation;
overlapping writable scope or conflicting revisions block mutation.

## 10. Evaluation

### Corpus

Create a private, post-cutoff set from 1-5 million LOC polyglot monorepos. Every
task has known files, symbols, impact radius, criteria, invariants, affected
tests, and held-out tests. Include bugs, multi-package features, refactors,
configuration changes, and ambiguous prompts for every edit-capable adapter.

### Adversarial cases

Test misleading names and docs, generated/vendor copies, dead code,
cross-language calls, hidden consumers, invented or renamed APIs, concurrent
edits, stale/truncated evidence, false text matches, LSP degradation, weak or
wrong-package tests, held-out failures, flaky/time-limited verification, partial
implementations, and premature success claims.

### Metrics and release gate

Primary metrics:

- undetected wrong-scope edit rate;
- undetected unresolved-symbol/API rate;
- undetected false-completion rate.

Also measure false blocks, localization precision/recall, freshness violations,
visible-to-held-out gap, task success, time, tool calls, tokens, index/query
latency, and prompt size.

Release requires:

1. zero undetected violations in all three primary classes on the frozen set;
2. every attempted violation blocked by a recorded deterministic gate;
3. no completion without a Verification Manifest;
4. crash, stale-index, and cross-language scenarios passing for every supported
   adapter; and
5. every prompt remaining within its hard budget.

The precise product claim is:

> Z Engine produced zero undetected wrong-scope edits, unresolved repository
> references, and false completions on the frozen evaluation corpus, while
> enforcing fail-closed runtime invariants for those failure classes.

## 11. Testing

- **Unit:** graph invalidation, evidence freshness, prompt determinism, work
  orders, gates, transitions, and manifests.
- **Adapter contracts:** definitions, references, call edges, imports,
  diagnostics, formatting, tests, health, and stale-index behavior.
- **Properties:** no scope escape, no stale authorization, no incomplete
  completion, append-only history, and bounded prompts.
- **Replay:** recorded providers and tools produce identical prompts and gates.
- **Fault injection:** LSP crashes, concurrent edits, corrupt blobs, truncation,
  timeouts, flaky tests, and interrupted provider streams.
- **Performance:** cold/incremental index, graph queries, evidence fetch, prompt
  construction, and memory use on representative monorepos.
- **Dogfood:** sealed worktrees graded by independent held-out checks.

## 12. Current Foundations and Delivery

Existing foundations include read-before-edit checks, Rust tree-sitter outlines,
rust-analyzer tools, context compaction, spill files, session JSONL, prompt
inspection, read-only subagents, checkpoints, usage reporting, and thin TUI/GUI/
headless clients.

Implementation requires these end-to-end slices:

1. deterministic recording, replay, and baseline metrics;
2. durable evidence records and content-addressed blobs;
3. pure bounded Prompt Builder;
4. adapter contract and revisioned Semantic Graph;
5. Work Orders and patch-layer localization gate;
6. repository-truth gate;
7. automatic verification and completion gate;
8. polyglot scale-out and adversarial release corpus.

No slice may weaken an earlier fail-closed invariant. Research sources informing
the design include Lost in the Middle (arXiv:2307.03172), Context Rot, LocAgent
(arXiv:2503.09089), SWE-Explore (arXiv:2606.07297), SpecBench
(arXiv:2605.21384), AgentHallu (arXiv:2601.06818), SWE-agent
(arXiv:2405.15793), and deterministic agent replay (arXiv:2607.16200).
