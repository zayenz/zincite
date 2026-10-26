+++
schema_version = 1
id = "checker-014"
key = "boolean-relations"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-011", "checker-013"]
+++
# Complete Boolean logic, clauses and Boolean linear constraints

## Outcome

Boolean standard builtins and Gecode aliases check and complete deterministic outputs.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Reuse the integer relation and reification helpers delivered by integer-relations. This family includes scalar truth tables, array reductions and bool2int channeling.

## Done when

- [x] All assigned Boolean comparisons, bool_not/and/or/xor, left/right implication, array_bool_and/or/xor, bool_clause, bool2int and bool_lin relations work, including actual _reif/_imp variants and argument conventions.
- [x] Empty reductions/clauses, Boolean linear bounds, false reifiers and disabled implication guards follow upstream semantics. Only unique supported outputs derive; XOR predicate/result arities are distinguished.
- [x] Typed violations and completion survive source preparation, validated loading and repeated checks; missing inputs do not turn an implication into an arbitrary assignment.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added 165 exact Boolean predicate names with truth calculations, unique completion, typed findings, declaration checks and validated loading; existing artifacts and reports stay compatible.

Validation:

- Independent verifier passed snapshot W89536e45c2c7b906 with no required corrections; source semantics, naming and responsibilities checked.
- Workspace formatting, Clippy with denied warnings, workspace tests and parser-free serialized tests and library Clippy passed.
- Locked parser-free Wasm build and matching bindings passed; parser exclusion verified and 24 native/Wasm reports agreed.
- Two MiniZinc 2.10.1 explicit controls passed eight candidates; pinned coverage confirmed 312 names and Python checks passed.
