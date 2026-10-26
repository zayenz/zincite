+++
schema_version = 1
id = "checker-026"
key = "packing-globals"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-012", "checker-014"]
+++
# Check packing and rectangle non-overlap interfaces

## Outcome

Bin-packing and fixed/variable-size rectangle predicates have complete concrete checking.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read Gecode bin-packing wrappers, nooverlap/nooverlap_fzn and optional/flex rectangle declarations; separate explicit-coordinate obligations from actual predicate checks.

## Done when

- [ ] All assigned bin_packing/load/capa and nooverlap/diffn aliases, including optional/flex forms, validate sizes, bin offsets, dimensions and presence.
- [ ] Concrete loads and rectangle intersections follow zero-width/height, touching-boundary, absent-rectangle and explicit right/top conventions. Known assignments derive unique loads where supported; no placements are chosen.
- [ ] Typed load/rectangle conflict findings preserve all affected entity positions. Checked coordinate arithmetic, malformed arrays and loaded references follow the common report/error contract.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
