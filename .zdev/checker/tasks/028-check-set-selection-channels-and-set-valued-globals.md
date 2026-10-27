+++
schema_version = 1
id = "checker-028"
key = "set-channel-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-027", "checker-020"]
+++
# Check set selection, channels and set-valued globals

## Outcome

Gecode set-channel and selection predicates are usable without solver-side propagation.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Inspect link_set_to_booleans, inverse_set, range/roots, indexed-set operations, sorted channels and set precedence definitions and aliases.

## Done when

- [x] Every assigned Boolean/set/integer channel, inverse_set, range/roots, member/precede_set, set sequence and selected union/intersection/partition operation checks its exact index and universe convention.
- [x] Empty selected indices, intersect-in universes, repeated references, absent members, sorted-channel cardinality and offset boundaries match upstream semantics; deterministic outputs derive only where unique.
- [x] Typed failures identify involved sets/indices/positions, with identical source/loaded validation and no arbitrary member selection or stale per-call state.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added 30 set-selection and channel spellings with compact padding/index calculations, typed findings and marked scalar/full-array completion. Forward bucket zero includes its virtual prefix so real channel faults do not cause false downstream findings.

Validation:

- Initial full review W3725e8b2f171ba5d requested one bucket-prefix completion correction. Root independently reproduced it; the regression and fresh focused recheck W147a507faa613551 passed with snapshot equality.
- Workspace formatting, Clippy and tests passed; parser-free check, 19 tests and locked build passed. Native all-target and Wasm-feature Clippy passed with warnings denied.
- Locked release Wasm, Node/Web bindings and syntax-free dependency trees passed; all 148 native/Wasm comparisons passed, with 16 original artifacts and 69 prior candidate byte slices unchanged.
- Both CLI/source-load paths agreed across 44 existing control checks and four focused regression checks. Eight temporary source/load/Wasm boundary comparisons passed. Pinned profiles compile; four expected annotation-related preparation rejections are documented for checker-030/031.
- Coverage reports 899 names with output rules checked for all 30 aliases. Ruff, ty, MiniZinc formatting, diff and zdev checks passed. Default MiniZinc lint reports only the documented upstream naming warning and 44 analysis limitations.
