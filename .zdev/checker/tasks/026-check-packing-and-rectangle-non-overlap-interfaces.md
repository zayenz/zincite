+++
schema_version = 1
id = "checker-026"
key = "packing-globals"
area = "checker"
status = "done"
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

- [x] All assigned bin_packing/load/capa and nooverlap/diffn aliases, including optional/flex forms, validate sizes, bin offsets, dimensions and presence.
- [x] Concrete loads and rectangle intersections follow zero-width/height, touching-boundary, absent-rectangle and explicit right/top conventions. Known assignments derive unique loads where supported; no placements are chosen.
- [x] Typed load/rectangle conflict findings preserve all affected entity positions. Checked coordinate arithmetic, malformed arrays and loaded references follow the common report/error contract.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added fifteen bin-packing and rectangle aliases with marked load completion, checked offsets and strict zero-size/optional/flex rectangle checks. Typed findings, validated loading and CLI feedback use the existing report and completion contracts.

Validation:

- Initial full review W66e664e2e234655a accepted semantics and requested one prepare feature gate. Independently checked correction passed fresh focused recheck W731691f36ddb7d0d; snapshot comparison equal.
- Workspace formatting, Clippy with warnings denied and all 640 tests passed. Parser-free native and Wasm Clippy passed with warnings denied; parser-free check, 17 tests and locked builds passed.
- Locked release Wasm, Node/Web bindings and syntax-free dependency trees passed; 128 native/Wasm comparisons across 15 artifacts and 64 candidates passed, preserving original 59 candidate bytes.
- Pinned standard/Gecode compile-only controls and calculated candidates passed 88 both-CLI source/load comparisons. Observed compiler metadata gaps are documented for checker-030/031.
- Coverage reports 821 names; Ruff, ty, MiniZinc formatting/default lint, zdev checks and diff whitespace checks passed.
