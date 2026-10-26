+++
schema_version = 1
id = "checker-012"
key = "integer-arithmetic"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-011"]
+++
# Check standard integer arithmetic and derive its outputs

## Outcome

Standard integer arithmetic works across native, serialized and Wasm checking.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Extend the checked i64 operations and existing definition worklist; follow the standard source for signed division, mod and negative powers.

## Done when

- [x] int_abs, int_plus, int_minus/int_negate aliases, int_times, int_div, int_mod, int_min, int_max, int_pow and int_pow_fixed plus their assigned Gecode aliases have exact direct checks and supported unique forward outputs.
- [x] Zero division, MIN/-1, intermediate overflow, exponent sign and zero-base boundaries follow the documented contract without wrapping. A supplied output is checked, and a derived out-of-domain value feeds downstream checks.
- [x] All assigned names have typed findings and validated operand/output loading; missing operands produce independent incomplete reports and successive calls use fresh values.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added 32 exact integer arithmetic spellings with checked i64 calculations, unique forward outputs, typed findings and validated loading across native and Wasm entry points.

Validation:

- Independent verifier passed snapshot W864b87a6585a7c59 with no required corrections; naming and responsibility boundaries checked.
- Workspace formatting, Clippy with denied warnings, workspace tests and parser-free native tests passed.
- Locked parser-free Wasm release build excluded zincite-syntax; matching wasm-bindgen bindings passed 20 native/Wasm report comparisons.
- Both MiniZinc 2.10.1 specimens compiled and checked; pinned-source coverage comparison and Python formatting, lint and type checks passed.
