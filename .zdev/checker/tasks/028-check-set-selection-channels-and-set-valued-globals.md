+++
schema_version = 1
id = "checker-028"
key = "set-channel-globals"
area = "checker"
status = "open"
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

- [ ] Every assigned Boolean/set/integer channel, inverse_set, range/roots, member/precede_set, set sequence and selected union/intersection/partition operation checks its exact index and universe convention.
- [ ] Empty selected indices, intersect-in universes, repeated references, absent members, sorted-channel cardinality and offset boundaries match upstream semantics; deterministic outputs derive only where unique.
- [ ] Typed failures identify involved sets/indices/positions, with identical source/loaded validation and no arbitrary member selection or stale per-call state.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
