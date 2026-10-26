+++
schema_version = 1
id = "checker-018"
key = "float-functions"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-015"]
+++
# Complete float powers and transcendental operations

## Outcome

The full standard float function family and Gecode float extensions check under the selected numeric policy.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read the standard mathematical/domain contracts and Gecode MPFR-conditional registrations. A solver build lacking MPFR does not justify omitting the targeted predicate from the Rust/Wasm checker.

## Boundaries

- Use a small portable math dependency only if the concrete native/Wasm operation needs it; no Gecode/MPFR process or dynamic-library dependency in checking.

## Done when

- [x] All assigned sqrt, pow/fixed-base/integer-exponent, exp/log, trig/inverse-trig and hyperbolic/inverse-hyperbolic operations, plus float array extrema, directly check and support documented forward outputs.
- [x] Negative/fractional power domains, zero denominators, log/sqrt boundaries, conversion overflow and nonfinite results produce the documented errors/findings without crashes or silent acceptance.
- [x] Native and Wasm use a documented portable mathematical implementation with the common tolerance policy; malformed loaded operands are rejected and typed numeric findings identify the relation.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added all 54 assigned float-function spellings, bringing admission to 581, with portable Rust math, exact domains, integer-power and array-extremum completion, typed findings and validated loading.

Validation:

- Independent full review passed all task conditions, naming and decomposition with no required corrections.
- Workspace fmt, all-targets Clippy and 596 tests; parser-free all-targets Clippy and tests passed.
- Locked parser-free Wasm release build, matching Node/web bindings and dependency/feature checks passed; all 68 native/Wasm report/error comparisons agreed.
- Both pinned MiniZinc compile-only controls, source formatting/default lint and both CLI source/load comparisons passed. Documented compiler metadata limitations were reproduced; full profile closure is future work.
- Coverage regeneration/no-write comparison, uv Ruff formatting/lint, ty, both zdev checks and diff whitespace checks passed.
