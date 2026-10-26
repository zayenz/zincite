+++
schema_version = 1
id = "checker-015"
key = "float-core"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-011", "checker-013", "checker-014"]
+++
# Add finite floats, configurable tolerance and core float predicates

## Outcome

Floats have one documented checking policy across Rust, CLI, prepared artifacts and Wasm.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Float support was deferred in the initial implementation. The user selected configurable tolerance. Read standard float builtins and Gecode float relation/conditional declarations, then extend Value, Domain, assignment parsing and transport directly.

## Done when

- [ ] Finite binary64 scalars, domains, aliases, arrays, objective values and decimal/exponent literals are supported in source/assignment/prepared JSON and Wasm. Integer decimal strings stay exact; NaN/infinite input and malformed float transport diagnose.
- [ ] One documented absolute/relative tolerance formula, defaults and zero-tolerance strict setting govern equality, complementary disequality, strict/non-strict inequalities, reification, implication and domain checks consistently through all entry points. Settings are explicit/reproducible; they do not round or complete missing values.
- [ ] Assigned float comparisons/linear forms, float_abs/plus/times/div/min/max, int2float, float_in/dom/set_in, float_ceil/floor/round and actual Gecode aliases check and derive supported outputs. Conversion ties, range failures, partial arithmetic and invalid domains follow the recorded policy.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
- Exercise near-tolerance comparisons, exponent literals, float objectives, >2^53 integer separation and a native/Wasm report agreement case. Document any expected numeric result tolerance.
