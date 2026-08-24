# Deviations from spec

Recorded per §10: when a requirement proves impractical, the smallest
alternative consistent with §2 constraints is chosen **and documented here**.

| # | Spec point | Deviation | Rationale | Version |
|---|---|---|---|---|
| 1 | §4.2 "No hard turn cap" | Runaway fuse at 500 consecutive tool rounds ends the turn with an error | Guards against pathological provider loops that repeat a failing call forever; far beyond any real task | v0.1 |
| 2 | §9 v1.0 `--headless` | Shipped early as a developer/acceptance flag (`--headless [task…]`, stdin when empty, `--auto-approve` companion) | Scriptable end-to-end acceptance runs need non-interactive driving; formal UX polish still lands in v1.0 | v0.1 |
| 3 | §6 token counting | Budget pressure uses provider-reported usage (prompt = latest request, completion = running total); local estimate is chars÷4 until v1.0 calibration | Avoids heavyweight tokenizer data; provider usage is authoritative for compaction decisions anyway | v0.3 |
| 4 | §5 reads "auto-allowed" | Read tools auto-allow only by tool name; path-level read restrictions are not enforced yet | Path-scoped read gating arrives with the outside-root write guard work (v0.5) where canonicalization exists; keeping v0.1 slice small | v0.5 planned |
