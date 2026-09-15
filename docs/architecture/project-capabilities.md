# Project capability discovery

`z-engine-project` is a bounded, read-only discovery boundary, independent of
the provider, agent runtime, and UI. It reports project roots, language
evidence, and statically justified **command suggestions**, not execution
results, installed-tool availability, semantic refactoring, or verified task
completion.

## API and adapter

The public entry points in
[`z-engine-project`](../../crates/z-engine-project/src/lib.rs) are:

```rust
discover(workspace: &Path, options: &DiscoveryOptions)
    -> Result<ProjectReport, DiscoveryError>

discover_with_cancel(workspace: &Path, options: &DiscoveryOptions, cancel: &AtomicBool)
    -> Result<ProjectReport, DiscoveryError>
```

`DiscoveryOptions` validates depth, entry, profile, per-file byte, and total
byte limits. Defaults are depth 8, 20,000 entries, 128 profiles, 256 KiB per
text file, and 4 MiB total. Limits include local ignore-file reads and ignored
directory entries. Each profile inspects at most 32 manifests and returns at
most 32 commands and 32 detailed
diagnostics plus an overflow diagnostic. Exhausted scan limits are explicit
partial reports; invalid arguments and workspace/cancellation failures are
typed errors. Malformed manifests retain their path and actionable diagnostics.

The [`inspect_project` adapter](../../crates/z-engine-core/src/tools/inspect_project.rs)
implements the existing `Tool` interface. It accepts only optional discovery
limits, not an arbitrary filesystem path. It scans `ToolCtx.project_root` on
a blocking worker with cooperative cancellation and returns a structured JSON
`ProjectReport`. Any error diagnostic makes the tool output unsuccessful
while preserving other discovered profiles. Discovery does not mark files
read for the edit gate and does not create verification evidence.

## Static support

| Input | Reported support |
| --- | --- |
| Cargo package/workspace | Rust; conventional `cargo check`, `cargo build`, `cargo test` |
| Node `package.json` | Only declared nonempty verification scripts; npm/pnpm/Yarn/Bun manager selection from `packageManager` or nearest lockfiles |
| Python `pyproject.toml`, `pytest.ini`, `tox.ini`, `setup.cfg` | Python; explicitly configured pytest and Ruff checks; no test runner inferred merely from source files/dependencies |
| Go `go.mod` | Go module declaration; conventional `go build`, `go vet`, `go test` for `./...` |
| Maven `pom.xml` | Conventional compile/verify/test lifecycle commands; local Maven wrapper when present |
| Gradle Groovy/Kotlin manifests | Marker-only: executable DSL/plugins/tasks are not evaluated or assumed |
| .NET SDK projects, solutions | Explicit build target; test suggestion only with static test-project evidence; solution references are not followed |
| CMake files/presets | Marker-only without declared build/test presets; direct, nonconditional presets are suggestions with unknown configure prerequisites |
| Makefiles | Literal unconditional build/check/test/lint targets; no included files, variable evaluation, or custom dialect interpretation |
| Other source languages | Filename-based language evidence; unsupported/unconfigured profile when no recognized project root covers the files |

`packageManager` takes precedence over conflicting locks, with a warning.
Conflicting locks without a declaration produce no guessed command.
Nested packages can inherit their nearest ancestor's manager while keeping
their own exact working directory. Without any declaration or lockfile,
npm is explicitly labeled a convention rather than a verified installation.
Lockfiles are filename evidence, not parsed dependency or integrity data.

Commands contain a program, argument vector, absolute `cwd`, manifest/lock
evidence, a declaration/convention basis, and `execution: "not_run"`.
They are untrusted suggestions: a later execution requires normal policy
and approval, and may execute arbitrary repository code, mutate files, or
access the network. A suggested test does not imply tests exist, were run,
or passed. Absent supported checks are `unconfigured`; toolchain and
semantic capabilities remain `unknown`; completion evidence is `not_provided`.

## Filesystem and parsing boundary

Filesystem reads use `cap-std` directory capabilities rooted at the authorized
workspace. Symlink entries are skipped; capability-relative opens prevent
escaping the workspace even if a nested path is replaced concurrently.
Scans are not atomic snapshots. VCS directories, common dependency/build
outputs, and local `.gitignore`/`.ignore` exclusions are pruned.
The established `ignore` crate compiles only the bounded local rules loaded
through the workspace capability: no parent/global Git configuration is read.
Queued directories share immutable compiled ignore rules instead of duplicating
their pattern storage.
Unreadable/invalid ignore rules stop that directory's scan instead of silently
disabling its exclusions. Manifest references outside the workspace are
never followed. XML predefined escapes and numeric references are resolved
locally; DTDs and custom entities are rejected and never fetched.

Parsers validate syntax and the fields they use, not complete build-tool
semantics. Inheritance, imports, conditions, remote/generated configuration,
toolchain versions, environment variables, and installed dependencies remain
unresolved. Language evidence does not imply that every language under a root
is verified by that root's build commands.

## Runtime integration

Core registers `inspect_project` in both the main built-ins and the read-only
research subset. Its name is reserved against MCP shadowing by the existing
registry collision guard. Policy allows this bounded read-only operation;
effect tracking leaves current verification evidence intact.

Discovery uses the ordinary execution path: the GUI receives tool events,
the model receives structured output, and the session journal retains the
result. The integration fixture exercises discovery before editing and after
verification, proving that suggestions do not manufacture evidence or invalidate
an otherwise valid check.

Crate dependencies: workspace `serde`, `serde_json`, `thiserror`, `toml`,
`ignore`; direct `cap-std = "3"` and `quick-xml = "0.38"`; dev-only `tempfile`.
No new async or provider dependency is introduced in the discovery crate.
