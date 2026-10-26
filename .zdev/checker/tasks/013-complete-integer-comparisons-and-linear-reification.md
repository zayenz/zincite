+++
schema_version = 1
id = "checker-013"
key = "integer-relations"
area = "checker"
status = "done"
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

The assigned scalar and linear signatures select relations by predicate name;
none takes a numeric relation-code argument. Validate operators, signatures and
serialized relation variants at these entry points. Numeric integer relation
codes occur in Gecode's gecode_count_int_fzn and gecode_count_matches_fzn
signatures and their registered aliases, owned by checker-019; validate those
codes there rather than adding counting in this task.

## Done when

- [x] Every assigned int_eq/ne/lt/le/gt/ge and int_lin_eq/ne/lt/le/gt/ge form, including _reif and _imp aliases, checks the documented relation; relation-code values are validated.
- [x] Reification equates its Boolean with the relation; implication constrains it only when the guard is true. Supported Boolean outputs and unique exact linear outputs derive without guessing or treating implication as equivalence.
- [x] Coefficient/array shapes, repeated output occurrences, checked accumulation and malformed source/serialized operands are validated; all concrete findings coexist with unresolved checks.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added all 108 exact scalar and linear integer relation names, full reification, implication guard handling, unique equality completion and typed findings while preserving version-1 executable and report shapes.

Validation:

- Independent verifier passed snapshot Wadb5a405ae6f88d1 with no required corrections; names, responsibilities, source semantics and compatibility checked.
- Workspace formatting, Clippy with denied warnings, workspace tests and parser-free serialized tests and library Clippy passed.
- Locked parser-free Wasm release and matching Node/web bindings passed; syntax was excluded and 20 native/Wasm comparisons agreed.
- Fresh MiniZinc 2.10.1 compilation preserved predicates and metadata; representative candidate checks, pinned-source coverage and Python checks passed.
