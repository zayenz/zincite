+++
schema_version = 1
id = "checker-025"
key = "explicit-end-scheduling"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-024"]
+++
# Check the latest Gecode explicit-end scheduling interfaces

## Outcome

The scheduling additions at pinned Gecode main work with typed native/Wasm feedback.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read gecode_fzn.mzn documentation and registry.cpp at commit 186551447db0f74639d65cb8de5af190f15ac2c2. These signatures are absent from the older installed Gecode library.

The user selected consistent checking of supplied start/end intervals, including
tuples for which Gecode's posting-time rewrites can change native acceptance.
Apply the brief's explicit-end convention and document this compatibility
choice. Separate model constraints check end equalities and caller obligations.

## Done when

- [x] gecode_unary_flex, gecode_cumulative_flex, gecode_cumulatives_flex and optional unary/cumulative variants validate and check explicit start/duration/end/presence arguments.
- [x] Caller obligations such as end equalities and machine/nonnegative-duration restrictions are distinguished from relations imposed by the native predicate. Contradictory supplied ends follow the brief's explicit concrete convention, without adding end equalities to the resource check.
- [x] Zero-duration interior/boundary cases, optional absent values, signed multi-machine usage and active-time upper/lower capacity semantics match the pinned source under coherent caller obligations and the brief's convention otherwise; no horizon-sized expansion is introduced.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
- Compile a tiny model through the pinned Gecode library and inspect the new native calls and separate end equations, then directly calculate one valid and one multi-finding invalid candidate.

## Result

Added fifteen explicit-end scheduling spellings and native seven-argument cumulatives aliases. Checks use supplied endpoints under the approved concrete convention, with caller equations separate and legacy artifacts and reports compatible.

Validation:

- Independent full review Wd0913955665cc152 passed with no required corrections; snapshot comparison equal.
- Workspace formatting, Clippy with warnings denied and all 635 tests passed; parser-free check, 16 tests and locked build passed.
- Locked release Wasm, Node/Web bindings and syntax-free dependency trees passed; 118 native/Wasm comparisons across 14 artifacts and 59 candidates passed with original 54 records byte-preserved.
- Pinned Gecode compilation emitted all five calls and a separate marked end equation; both CLI forms and source/load agreed across 24 checks, including calculated valid and four-finding invalid candidates.
- Coverage reports 806 names; Ruff, ty, MiniZinc formatting and warning-free lint, zdev checks and diff whitespace checks passed. Two bounded analysis limitations are documented.
