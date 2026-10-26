+++
schema_version = 1
id = "checker-020"
key = "ordering-channel-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-013", "checker-014"]
+++
# Check ordering, distinctness, extrema and integer channels

## Outcome

Ordering and channel globals from the Gecode library have complete concrete-assignment semantics.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read Gecode ordering/all-different/optional, arg-min/max, inverse and sort declarations and registrations; argument offsets and tie choices differ between names.

## Done when

- [x] All assigned all_different/equal/except/offset/optional, lexicographic/array inequalities, increasing/decreasing, sort/permutation, inverse_offsets, member, bool-int channel and integer/Boolean extrema/arg-extrema forms are checked.
- [x] Optional presence, offset arithmetic, duplicate values, first-tie versus any-tie arg-extrema, empty arrays and channel index domains match each signature. Check relations without choosing a permutation or inverse when not uniquely determined.
- [x] Useful unique extrema/membership/reifier/channel outputs derive and serialize; typed findings identify concrete conflicting positions and malformed shapes/flags/codes diagnose.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added 108 ordering and channel names with typed conflicts, unique scalar and array completion, validated canonical targets and exact checked offsets; total admission is 743 names.

Validation:

- Independent full review accepted runtime behavior and naming; one fzn_sort coverage correction passed a focused independent recheck against W7e57e103ef4bf4ef, with equal snapshot comparison.
- Workspace format, Clippy and tests; parser-free check/test; release Wasm and matching bindings/dependency inspection; 80 native/Wasm comparisons passed.
- Coverage/pinned-source checks, Ruff/ty, both MiniZinc controls under both profiles, format/default lint, both CLI forms and source/export/load reports passed. Compiler metadata limitations documented for checker-030/031.
