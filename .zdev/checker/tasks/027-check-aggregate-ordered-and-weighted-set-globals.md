+++
schema_version = 1
id = "checker-027"
key = "set-aggregate-globals"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-017", "checker-012"]
+++
# Check aggregate, ordered and weighted set globals

## Outcome

Gecode aggregate-set and extremum relations support compact exact values and useful outputs.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read aggregate-set and set-extremum declarations/registrations. In particular, the empty Gecode array intersection uses its finite solver set universe, not an unbounded i64 universe.

## Done when

- [ ] All assigned array union/intersection/partition, disjoint, convex/hull, seq/seq_union, set_min/max reif/imp, fixed-cardinality atmost_one and weighted-sum forms check their exact contracts.
- [ ] Empty-array/intersection identities, empty-set extrema, weights, pairwise cardinalities and set order are correct. Unique aggregate/extremum outputs derive without enumerating huge ranges for storage.
- [ ] Typed aggregate findings and explicit Gecode universe constants survive native/loading/Wasm paths; large cardinalities/weighted arithmetic are checked and missing values stay incomplete.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
