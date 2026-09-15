# GUI-first agent harness delivery roadmap

Status: S1 and the bounded-supervision/context foundation are implemented;
GUI-only retirement is included in the current migration. Other slices remain
planned. No release dates promised.
Baseline and known gaps: [current state](../architecture/current-state.md).
Design: [architecture](../architecture/agent-harness.md) and
[runtime contracts](../architecture/task-runtime.md).
Conventions: [engineering guide](../engineering/style-guide.md).

The [older roadmap](../ROADMAP.md) records historical delivery; its checkmarks
do not certify the guarantees below.

## Delivery rules

- Deliver user-visible vertical slices, not a long infrastructure phase.
  Each slice wires GUI -> agent -> tools -> state -> verification -> persistence.
- S0 is product-surface cleanup and can run alongside S1; do not delay evidence
  correctness while reorganizing packages.
- Retain existing behavior until a tested replacement lands. No deleting tests
  because the frontend that originally drove them is being retired.
- Use deterministic mocked-provider and local fixture tests as release gates.
  Live-model demos supplement them; they cannot substitute for failure tests.
- Mark a slice done only with recorded acceptance evidence and a usable GUI
  flow. Do not treat creating a trait/crate or updating a prompt as delivery.
- Extract boundaries when used. Every extraction updates the structure contract
  and import graph; do not pre-create the complete candidate crate list.

## S0. One supported product surface

**Implemented:** terminal targets/dependencies/scripts are removed; private
integration fixtures preserve useful acceptance coverage. Releases package only
the desktop app, retaining all existing desktop OS targets. Native interactive
acceptance and cross-platform installer builds remain separate validation steps;
browser component checks are not a substitute for them.

**Outcome:** contributors launch and release the GUI without maintaining a
second product frontend. Can proceed alongside S1.

- Promote GUI install/development/release documentation.
- Move useful headless acceptance scenarios into Rust integration fixtures or
  a private test driver, not a new supported CLI.
- Remove TUI/headless product targets, dependencies, release artifacts and
  frontend-specific scripts after their still-useful tests have replacements.
- Keep provider/core libraries UI-independent and preserve stored sessions.
- Update workspace/default build targets and the structure contract in the
  same implementation slice; audit CI, packaging and updater inputs.

**Accept:** GUI builds, launches, submits a fixture task, streams tools, approves
an edit, stops, opens a saved session, and displays the diff. Release jobs no
longer produce CLI artifacts. Supported desktop OS scope is confirmed before
removing any platform job; GUI-first alone does not select macOS-only.

## S1. Evidence-backed bug fix: first reliability milestone

[Implementation and acceptance evidence](../architecture/verification-completion.md).

**Outcome:** "fix this failing Rust test" produces a diff and a persisted
completion report backed by checks actually run. No prerequisite full rewrite.

- GUI separates response-finished from task outcome and displays required
  checks, failures/blocks, requirement coverage and evidence freshness.
- Agent owns a task goal/requirements record and requests applicable
  verification before proposing completion.
- Add typed targeted-test/build operations using existing Cargo commands
  through the normal policy gate; retain exit status and artifact references.
- Add check-plan/evidence/assessment records and explicit non-success outcomes.
  Use focused core modules first, extracting verification/contracts only if
  their real shared boundary is already clear.
- Persist task identity, checks and assessment with backward-readable session
  handling. Reopening historical sessions shows unassessed, never verified.

**Accept:** mocked-model bug fix runs a failing test, edits, reruns it green,
shows current evidence, and survives reload with the same outcome. A model that
says "done" without checks, a nonzero exit with no diagnostics, a denied check,
a persistence failure, or an edit after the passing check cannot produce
Complete. Unknown impact from shell changes blocks or expands verification.

## S2. Ask about meaningful uncertainty

The intervening [supervision/context foundation](../architecture/supervised-harness.md)
adds a real bounded continuation loop, protected task packets, project
capability discovery, and GUI supervision settings/reporting. It is not the
full S7 strategy engine or S4 durable Pause. The next capability remains
structured questions and explicit task-intent contracts.

**Depends on:** S1 task identity and replay.
**Outcome:** the agent can ask one structured question and continue from the
answer without guessing or losing it on a session switch.

- Introduce an identified question tool, request/answer records and a waiting
  state. Use the GUI's existing form/dialog primitives and command boundary.
- Include choices, custom answer, reason, optional recommendation and dismiss.
  Keep design answers separate from execution approvals.
- Update main/question guidance; register only the tool that actually exists.

**Accept:** a fixture with two incompatible requirements cannot edit until
answered; custom answer and dismissal behave explicitly. Reload/session switch
preserves the pending question; duplicate, stale and wrong-task answers do not
resume the wrong operation. Answered decisions are visible on replay.

## S3. Optional, inspectable prompt enhancement

**Depends on:** S2 for unresolved ambiguity.
**Outcome:** a user can enable enhancement, inspect its interpretation, or keep
the original prompt path exactly as supplied.

- Add a default-off setting through types/partial/file shape, loader, store,
  IPC wrapper and GUI settings; show effective scope and source.
- Preserve original and enhanced versions separately with strategy version.
  Enhancement proposes acceptance criteria, not new authority.
- Use an injected enhancer strategy; do not bake enhancement into every turn.
  Failure offers retry/use-original and does not silently mutate the request.

**Accept:** off causes zero enhancer requests and identical original user text
at agent intake. On makes one attributable enhancement request; scope changes
ask the user. Global/project precedence, restart persistence, failure, and
disable behavior are tested. No settings edit retroactively rewrites a task.

## S4. Safe Pause and resume after restart

**Depends on:** S1; integrate S2 pending questions when available.
**Outcome:** Pause drains a managed operation, checkpoints, and later resumes;
Stop remains cancellation rather than suspension or rollback.

- Add distinct GUI controls/states and runtime commands/events.
- Introduce explicit continuation/checkpoint records, durable operation
  intent/outcome, effect classification, and workspace identity.
- Stop scheduling on pause; finish the current safe managed-edit unit, persist
  its result, then acknowledge Paused. Questions/approvals can be persisted.
- Resume validates versions and permissions before new work. Use existing
  journal storage with schema/migration support, not an unneeded database swap.
- Extract runtime contracts/persistence where needed to keep ownership clear.

**Accept:** pause during streaming, an edit, and a pending question reaches the
correct boundary; restart restores the goal, tools, decisions and evidence.
Storage failure never produces Paused. A long tool remains Pause requested
until safe, with Stop available. Stop does not claim completion or undo files.
Measure cancellation and pause latency on supported platforms and set budgets.

## S5. Crash reconciliation and conflict-safe rollback

**Depends on:** S4.
**Outcome:** an interrupted task explains what happened, assesses completion,
and offers safe continuation or restoration without overwriting user edits.

- Journal managed pre/postimages and operation progress. Cover shell, hooks,
  check scripts and MCP effects with explicit known/unknown outcomes.
- Restore only owned versions; show unsupported/untracked rollback scope.
  Do not automatically replay non-idempotent process/remote actions.
- Compare goal, repository, changes, prior state and verification to propose
  Complete/Continue/Repair/Verify/AskUser; the deterministic gate still decides.
- Detect missing workspaces, torn tails, mid-journal corruption and unsupported
  schema versions. Preserve damaged data for recovery.

**Accept:** crash before effect, after effect/before outcome, and after durable
outcome yields correct reconciliation without duplicate effects. User edits
cause a conflict, not overwrite. Missing artifacts invalidate evidence. An
interrupted model/tool/time budget never becomes Complete solely on reload.

## S6. One safe semantic refactor

**Depends on:** S1 and S5 mutation/recovery contracts.
**Outcome:** rename one Rust symbol across declarations/references from the GUI,
with a preview, approval, managed application and current verification.

- Reuse Rust LSP integration; add capability discovery and workspace-edit
  conversion instead of building a second language engine.
- Validate all affected paths and document versions before applying the plan.
  Handle LSP position encoding and file operations explicitly.
- Extract repository/language-tooling boundaries when this real operation needs
  them. Tools return structured edit plans and evidence.

**Accept:** a multi-file fixture updates the intended symbol and compiles/tests;
same-spelled unrelated symbols remain unchanged. Stale files, unsupported
rename, denied paths, and interrupted application refuse or recover explicitly.
Preview/application agree; regex fallback is never advertised as semantic.

## S7. Dynamic internal strategies, not workflow modes

**Depends on:** S1, S2 and S4; benefits from S6 capabilities.
**Outcome:** small tasks stay short; complex tasks choose research, planning,
review, repair and verification without a user selecting a workflow mode.

- Extract cognition from runtime enforcement, and policy from execution.
- Add narrow strategy registration and typed next-action results. Introduce
  planner/debugger/verifier/reviewer prompts only with working callers.
- Preserve read-only and approval controls as explicit authority settings while
  retiring workflow-mode coupling; migrate saved preferences visibly.
- Record strategy/version, selected step, budget and decision summary.

**Accept:** deterministic simple and complex fixtures take different legal
paths, both satisfy completion gates, and survive pause/resume. Invalid model
steps, cyclic non-progress and budget exhaustion block/escalate. A read-only
task never gains writes by choosing an implementation strategy. One additional
test strategy registers without changes to unrelated GUI screens.

## S8. MCP parity and broader verification profiles

**Depends on:** S5 and S7.
**Outcome:** configured external capabilities and additional project checks are
discoverable, permissioned, observable, resumable where safe, and truthful.

- Preserve the shared Tool adapter boundary; namespace server/tool identities.
- Carry typed tool effects, protocol failure status, timeout/cancellation and
  artifact metadata through policy, journal and GUI.
- Add one real non-Rust profile using that project's existing scripts, including
  targeted tests, type/build checks and dependency/configuration coverage.
- Add implementations/callers, formatting and dependency inspection as separate
  registered capabilities when supported. Do not promise universal refactoring.

**Accept:** local fake MCP servers cover success, explicit tool error, malformed
response, disconnect and unknown post-timeout effects. No failure becomes
empty-success. A non-Rust fixture displays check evidence and rejects stale
results. Discovery cannot self-grant permissions or bypass operation records.

## S9. Useful project and global experience

**Depends on:** S1 evidence and S7 strategies; S5 recovery is recommended first.
**Outcome:** a second similar task retrieves a proven approach, with provenance
the user can inspect or delete.

- Extract a concise problem/approach/alternatives/failures/solution/verification
  record only after evidence-backed completion.
- Start with project-scoped retrieval; global promotion requires explicit
  consent and private-content filtering.
- Bound retrieval by relevance and budget. Version records, detect stale
  applicability, and provide inspect/edit/delete/disable GUI controls.
- Use an experience module until independent storage/retrieval needs a crate.

**Accept:** related tasks retrieve useful evidence-backed records; unrelated
tasks do not. Failed/unverified tasks cannot create "successful" experience.
Deleted/disabled records are absent after reload; global records exclude
repository-private content by default and never override current instructions.

## S10. Preference suggestions without hidden behavior changes

**Depends on:** S9.
**Outcome:** repeated choices can produce an explainable, dismissible preference
suggestion instead of silently changing the agent globally.

- Persist observation provenance and project scope; keep observations separate
  from active settings and permissions.
- Let users accept locally, promote globally, edit, or dismiss. Record consent
  and effective preference scope; avoid repetitive suggestions.

**Accept:** repeated fixture choices produce a suggestion but no behavior
change until acceptance. Dismissal and project/global promotion survive restart.
Revocation affects subsequent attempts. Approval history never becomes an
automatic authorization rule.

## S11. Generated-code lifecycle and advanced capabilities

**Depends on:** S6, S8 and S9.
**Outcome:** unused/speculative generated code is reviewed with evidence and
revisited when its recorded condition is reached.

- Track symbol/change provenance and Active/Unused/Speculative/Deferred/
  RequiredLater/Rejected states with reasons and revisit triggers.
- Combine diagnostics and references; account for public APIs, reflection,
  registration and generated use before proposing deletion.
- Show retain/defer/remove decisions in the GUI; use normal mutation,
  verification and persistence paths. Reevaluate deferred items later.
- Add change-signature, move-symbol, extract-method and import repair one
  supported language operation at a time, each as its own tested vertical slice.

**Accept:** truly unused fixture code can be removed and verified; externally
used/dynamically registered code is not auto-deleted. Deferred items survive
restart and reactivate or prompt for removal at their trigger. No speculative
tool is advertised merely because its name appears in a roadmap.

## Extraction and retirement gates

| Stage | Boundary work, only with its working slice |
|---|---|
| S0 | Remove frontend/product duplication, not reusable libraries/tests |
| S1-S3 | Focused task/evidence/question/enhancer modules; first verification seam |
| S4-S5 | Contracts, runtime, persistence and repository ownership; durable execution |
| S6-S8 | Language tools, policy, cognition, tools and MCP adapters as justified |
| S9-S11 | Experience boundary; indexing only if independently needed |

Prompts move into a dedicated crate only when cognition extraction updates the
current prompt-location contract. Core eventually becomes a composition facade
and is removed once GUI/tests directly use the extracted owners. Each facade
has that concrete removal gate; no permanent legacy compatibility layer.

## Coverage of the original twenty requirements

| # | Requirement | Delivery |
|---|---|---|
| 1 | GUI-first | S0 |
| 2 | Vertical slices | All slices and delivery gates |
| 3 | Modular crates and deliberate dependencies | Extraction gates, S4-S8 |
| 4 | Engineering style guide | Companion guide now; enforced on changes |
| 5 | Explanatory code and documentation | Guide and every slice |
| 6 | Guided/specialist prompts | S1-S3, S7, S9 |
| 7 | Optional prompt enhancer | S3 |
| 8 | Powerful coding tools | S1, S6, S8, S11 |
| 9 | Structured questions | S2 |
| 10 | First-class MCP | Existing adapter retained; S5, S8 |
| 11 | Dynamic loops without workflow modes | S7 |
| 12 | Evidence-backed final verification | S1, S8; invariant thereafter |
| 13 | Unused/speculative code lifecycle | S11 |
| 14 | Stop and durable Pause | S4 |
| 15 | Completion assessment after interruption | S1, S5 |
| 16 | Project/global engineering experience | S9 |
| 17 | Consent-based preference learning | S10 |
| 18 | Fail-safes, recovery, permissions | S1, S4-S6, S8 |
| 19 | Easy experimentation | S3, S7; versioned narrow registrations |
| 20 | Cognition separate from discipline | All architecture and gates |

**Next implementation:** S2 structured questions and explicit task-intent
contracts, followed by S4/S5 durable execution and recovery. Preserve the S1
evidence gate and bounded-supervision acceptance scenarios throughout.
