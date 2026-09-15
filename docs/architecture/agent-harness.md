# GUI-first agent harness architecture

Status: proposed staged architecture, not an implementation claim.
Baseline: commit `31ad509` (2026-09-12).

Read with the [current-state assessment](current-state.md),
[task/runtime contracts](task-runtime.md),
[engineering guide](../engineering/style-guide.md), and
[delivery roadmap](../roadmap/agent-harness.md).

The [S1 implementation](verification-completion.md) establishes the first
verification/completion boundary. The [supervised harness slice](supervised-harness.md)
adds bounded continuation, grounded context packets and project discovery.
The broader design below remains staged work.

## 1. Product direction

**The LLM provides cognition. Z-Engine provides engineering discipline.**

The product is one excellent GUI for supervising persistent engineering tasks,
not a collection of user-selected agent modes. Core capabilities remain
UI-independent so another interface could be added later without being
maintained today.

"Autonomous" means the system can choose and execute authorized next steps,
show evidence, and recover or escalate. It does not mean AGI, proof of arbitrary
program correctness, unlimited execution, or permission to change user intent.

### Non-negotiable invariants

1. A response ending is not task completion.
2. Code-changing tasks need current verification evidence and explicit
   requirement coverage before a Complete transition.
3. Tool success, process exit, model assessment, and verification are separate.
4. Stop cancels; Pause acknowledges only a durable safe boundary.
5. Unknown side effects remain unknown until reconciled.
6. All executable capabilities, including MCP, hooks, and verification scripts,
   pass through the same policy and operation recording boundary.
7. A resumed task never silently repeats a non-idempotent operation.
8. User edits, credentials, and consent survive agent mistakes and migrations.

### Explicit non-goals

- Maintaining TUI, a public headless CLI, or speculative frontend adapters.
- Rewriting every subsystem before delivering a useful feature.
- Shipping every IDE refactoring across every language at once.
- Creating an in-process plugin ABI, distributed workflow service, or general
  graph DSL before a concrete extension needs it.
- Treating model debate, reviewer agreement, or more agents as proof.

## 2. Current architecture and migration authority

The GUI-only workspace retains provider and core, and introduces focused
runtime, context and project-discovery crates alongside the GUI.
Core still owns reusable tools, permissions, session persistence, context integration,
LSP/MCP, and an agent loop. The GUI already has commands, event projections,
settings, approval cards, and session replay. Reuse these seams.

The [structure contract](../../AGENTS.md) remains binding until an implementation
slice updates it. In particular, prompts stay in
[core prompt Markdown](../../crates/z-engine-core/prompts) and
[its registry](../../crates/z-engine-core/src/prompts.rs) today.
This document does not authorize moving them early or leaving multiple owners.

The target is a shrinking core, not a renamed monolith. New capabilities start
as focused modules when extraction would add only forwarding. Extract them as
soon as a real dependency or testing boundary justifies it.

## 3. Responsibility map

The following are logical boundaries and candidate crate names, not a request
to scaffold every crate now.

| Boundary / candidate crate | Owns | Must not own |
|---|---|---|
| GUI application (`z-engine-gui`) | Composition, IPC, views, user decisions, local UI state | Agent policy or authoritative completion |
| Contracts (`z-engine-contracts`) | Cross-boundary IDs, commands/events, task/evidence records, narrow capability ports | I/O, provider messages, implementation helpers |
| Runtime (`z-engine-runtime`) | Task transitions, scheduling, cancellation, execution gate, completion gate | Provider HTTP, UI rendering, language-specific parsing |
| Cognition (`z-engine-agent`) | Task interpretation, next-step/role selection, model-based assessment | Direct writes, permission grants, durable-state bypass |
| Provider (`z-engine-provider`, existing) | HTTP/SSE, wire types, stream accumulation | Task lifecycle, tools, GUI types |
| Tools (`z-engine-tools`) | Built-in tool input validation and execution adapters | Deciding task completion or granting authority |
| Repository (`z-engine-repository`) | Workspace identity, versions, managed mutations, checkpoints, change manifests | Model reasoning or UI diff state |
| Verification (`z-engine-verification`) | Project/check discovery, check plans, result interpretation, coverage | Ungated process execution or self-declared task success |
| Persistence (`z-engine-persistence`) | Versioned journal/snapshot/artifact storage and migrations | Choosing next actions or treating replay as execution |
| Policy (`z-engine-policy`) | Capability/effect decisions and scoped authorization | Executing tools or learning permission from habits |
| Language tooling (`z-engine-language-tools`) | LSP/AST adapters, diagnostics, semantic edit plans | Applying edits outside repository transactions |
| MCP (`z-engine-mcp`) | Discovery, protocol, remote capability transport | Policy bypass or unchecked success translation |
| Prompts (`z-engine-prompts`, later) | Versioned instruction data and registry | Runtime control flow |
| Experience (`z-engine-experience`, later) | Scoped extraction/retrieval, provenance, preference proposals | Hidden global behavior changes or authority |

Research, planning, debugging, reviewing, and question selection begin as agent
strategies, not separate crates. Repository indexing begins inside repository
or language tooling. Extract either only when its lifecycle or dependency cost
becomes independently meaningful.

### Dependency direction

Arrows mean "imports." Application wiring is the only place that knows all
concrete implementations.

```text
GUI application -> runtime, agent, tools, repository, verification,
                   persistence, policy, provider, optional adapters

runtime      -> contracts, policy
agent        -> contracts, provider, prompts
tools        -> contracts, repository, language-tools, mcp
verification -> contracts, repository
repository   -> contracts
persistence  -> contracts
policy       -> contracts
language-tools / mcp / experience -> contracts
provider / prompts -> no application or agent dependencies
```

Contracts contain only values/interfaces actually exchanged between boundaries.
An implementation-specific type stays with its owner. Add no generic `shared`
dependency that imports all the subsystems.

Verification selects checks and interprets execution records. Runtime submits
those checks through the same authorized executor used by other tools; the
verifier does not spawn a second unchecked command path. Semantic tools produce
edit plans and repository applies them under that gate.

During extraction, core may temporarily re-export moved APIs. Remove each
facade once GUI/tests use the new owner; never maintain duplicate implementations
or introduce dependencies back into core.

## 4. End-to-end execution

```text
GUI goal
  -> persist original request + task identity
  -> optional enhancement / requirement interpretation
  -> cognition proposes next step
  -> runtime validates state, authority, budget and capabilities
  -> journal operation intent
  -> authorized tool / managed repository change
  -> journal outcome and evidence
  -> verification / review / repair as needed
  -> completion assessment + deterministic evidence gate
  -> durable task state -> GUI projection
```

The runtime owns this boundary even when the next step is selected dynamically.
It must be possible to answer "what is this task waiting for?" without asking
the model or parsing the last assistant message.

### Minimum ports, introduced when used

| Port | Contract |
|---|---|
| `Cognition` | Given explicit task context and capabilities, propose a typed next action/assessment |
| `ToolExecutor` | Execute an authorized, identified operation; return typed outcome/effects/artifacts |
| `Repository` | Observe versions, prepare/apply managed edits, reconcile and restore owned changes |
| `Verifier` | Select applicable checks and interpret recorded outcomes against a change manifest |
| `TaskStore` | Append ordered records, commit checkpoints, load/replay, report durability errors |
| `ExperienceStore` | Retrieve or propose scoped experience with provenance and consent metadata |

The types above are proposed contracts, not existing Rust symbols.
Do not build a configurable step engine to deliver the first completion slice.

## 5. Dynamic cognition without user modes

The model chooses how much investigation, planning, research, review, or repair
is useful. The runtime decides whether a requested transition is legal.

Each internal step declares input/output schemas, capabilities, budgets,
cancellation boundaries, and a strategy/prompt version. Add a new step through
one registration and its own tests, not new branches in ten feature screens.

Examples:

- A small fix can investigate, edit, verify, and assess.
- An ambiguous redesign can ask, research, plan, implement, verify, review, and
  repair before assessing.
- An informational answer needs source/requirement checks, not a compiler run.
- Security, performance, or dependency review can be added as specialized
  strategies without replacing the runtime. Their tools remain permissioned.

Role outputs are advice or typed proposals. A reviewer cannot grant itself
writes. A planner cannot exempt a task from verification. Research remains
read-only until an explicit capability contract says otherwise.

Remove user-facing workflow modes incrementally, but retain explicit access
controls. "Read-only" is an authority constraint, not an instruction to be less
intelligent; model autonomy never implies automatically accepting edits.

## 6. Prompt enhancement and methodology

Preserve the original request verbatim. An enhancer creates a separate task
interpretation containing the goal, known constraints, acceptance criteria,
uncertainties, and suggested investigation. It cannot manufacture requirements,
change approval policy, or replace the source of user intent.

Proposed default: **off**. When off, bypass the enhancer and pass the original
request unchanged to normal task intake. Normal system instructions and context
still apply. When on, expose the enhancement and its provenance in the GUI.
Enhancer failure is visible and offers retry or use-original, never a hidden
extra model call or altered goal.

Prompts teach an engineering method: understand intent, inspect existing
architecture, select capabilities, ask about material ambiguity, implement
precisely, verify, recover, and explain evidence. Introduce specialist prompts
only when their callers land, with scenario tests and versioned records.

## 7. Language-aware tools and verification

Expose intent-level capabilities rather than pretending text editing is
semantic refactoring. Start with a Rust semantic rename: resolve symbol,
request a workspace edit, validate all affected versions/paths, preview,
approve, apply under the repository boundary, and verify.

Then add references/implementations/callers, targeted tests, diagnostics,
formatting, and dependency inspection according to measured project needs.
Change-signature, move-symbol, extract-method, and import repair require
language-specific support; advertise unsupported capabilities honestly.

Verification discovers existing project scripts/configuration. Select
applicable tests, build/type checks, diagnostics, formatting, and dependency
consistency checks based on changed scope. Account for generated files,
configuration, dependency manifests, and public API consumers.
Diagnostics may include unresolved references, unused imports/symbols,
unreachable code and dead code; report each analyzer's availability and limits
rather than assuming every language supports every check.

Every task receives a completion assessment, but not every task runs every
check. An unavailable or unrun required check is a visible block, not a pass.
Compiler success alone cannot prove all behavior; requirement coverage and
reported residual uncertainty remain part of the completion record.

## 8. Experience and generated-code lifecycle

Only after evidence-backed completion, extract problem, investigation,
decisions, alternatives, failed attempts, solution, verification, and reusable
pattern. Separate repository-scoped experience from explicitly promoted global
knowledge. Retrieve a small relevant subset with source, age, applicability,
and contradiction handling; never dump all history into a prompt.

Observed preferences create suggestions, not silent settings changes. Global
promotion requires user confirmation, and permissions never become inferred
preferences. Provide inspect, edit, delete, and disable controls; keep secrets
and repository-private content out of global records by default.

Treat unused-code analysis as evidence, not a deletion command. Track candidate
symbols as `ACTIVE`, `UNUSED`, `SPECULATIVE`, `DEFERRED`, `REQUIRED_LATER`, or
`REJECTED`, with owner, reason, evidence and revisit condition. Check reflection,
registration, public API use and generated callers before proposing removal.
Deletion is a normal reviewed mutation followed by verification.

## 9. Adoption and unresolved decisions

The [roadmap](../roadmap/agent-harness.md) sequences working GUI slices and
names extraction gates. The first reliability milestone is a failed-test fix
whose completion is backed by persisted, current evidence, not a rewrite.

Before relevant implementation slices, confirm:

- Which desktop OS builds remain release commitments. GUI-first removes extra
  frontends, not automatically Windows/Linux support or necessary OS adapters.
- Cancellation latency and storage durability objectives on supported hosts.
- Initial supported language/check profiles beyond Rust.
- Retention/size limits and explicit consent for cross-project experience.

These decisions do not block documenting the architecture or the first Rust
completion slice. No runtime behavior, platform support, or stored data is
changed by this proposal.
