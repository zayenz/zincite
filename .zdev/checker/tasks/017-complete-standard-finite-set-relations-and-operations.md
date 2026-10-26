+++
schema_version = 1
id = "checker-017"
key = "set-primitives"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-011", "checker-014"]
+++
# Complete standard finite-set relations and operations

## Outcome

All standard set primitives, reification and implication forms use compact exact set values.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Extend sets.rs range algorithms and current six set predicates; read lexicographic set-order definitions and Gecode relation-code aliases.

## Done when

- [x] Assigned set_eq/ne/le/lt/subset/superset/in/card/union/intersect/diff/symdiff and aliases, with all actual _reif/_imp forms, check upstream semantics using canonical compact ranges.
- [x] Unique set/reifier/cardinality outputs derive where defined; comparisons never choose members. Empty-set ordering, endpoint boundaries, variable membership and disabled guards are correct.
- [x] Large intervals are not expanded just for representation or algebra. Source/transport validates canonical ranges; cardinality and downstream arithmetic use checked exact integers and preserve incomplete/invalid independence.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Completed 84 exact finite-set primitive names, adding 78 admissions, with compact range algebra, both order semantics, deterministic completion and typed native/Wasm findings.

Validation:

- Independent full review passed runtime, source/loading contracts, legacy encodings, naming and decomposition; the sole stale coverage total was corrected and passed a fresh focused recheck.
- Workspace fmt, Clippy and 589 tests; parser-free Clippy and 8 serialized tests passed.
- Locked parser-free Wasm release build, matching Node/web bindings and dependency tree passed; all 56 native/Wasm report comparisons agreed.
- Both pinned MiniZinc compile-only profiles, metadata inspection and 32 source/load checks through both CLI forms passed; source formatting/default lint passed.
- Coverage regeneration and no-write verification, Ruff formatting/lint, ty and diff whitespace checks passed.
