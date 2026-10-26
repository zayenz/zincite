+++
schema_version = 1
id = "checker-013"
key = "integer-relations"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-011"]
+++
# Complete integer comparisons and linear reification

## Outcome

All standard and Gecode integer relation and linear forms have consistent check/reification behavior.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Generalize the existing int_lin_eq/int_lin_le and int_le_reif cases without changing worklist semantics.

## Done when

- [ ] Every assigned int_eq/ne/lt/le/gt/ge and int_lin_eq/ne/lt/le/gt/ge form, including _reif and _imp aliases, checks the documented relation; relation-code values are validated.
- [ ] Reification equates its Boolean with the relation; implication constrains it only when the guard is true. Supported Boolean outputs and unique exact linear outputs derive without guessing or treating implication as equivalence.
- [ ] Coefficient/array shapes, repeated output occurrences, checked accumulation and malformed source/serialized operands are validated; all concrete findings coexist with unresolved checks.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
