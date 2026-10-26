+++
schema_version = 1
id = "checker-025"
key = "explicit-end-scheduling"
area = "checker"
status = "open"
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

## Done when

- [ ] gecode_unary_flex, gecode_cumulative_flex, gecode_cumulatives_flex and optional unary/cumulative variants validate and check explicit start/duration/end/presence arguments.
- [ ] Caller obligations such as end equalities and machine/nonnegative-duration restrictions are distinguished from relations imposed by the native predicate. Contradictory supplied ends expose the actual upstream checks, not an invented stronger or weaker global.
- [ ] Zero-duration interior/boundary cases, optional absent values, signed multi-machine usage and active-time upper/lower capacity semantics match the pinned source; no horizon-sized expansion is introduced.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
- Compile a tiny model through the pinned Gecode library and inspect the new native calls and separate end equations, then directly calculate one valid and one multi-finding invalid candidate.
