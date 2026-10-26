+++
schema_version = 1
id = "checker-011"
key = "text-grammar"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-010"]
+++
# Accept the full FlatZinc text structure and annotations

## Outcome

Standard FlatZinc declarations and compiler metadata prepare through the existing CST with precise validation.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. The current lowerer rejects almost all predicate declarations and many harmless annotations. Read the 2.10.1 FlatZinc grammar and Gecode metadata declarations before extending Source and Lowerer.

## Boundaries

- Preserve source spelling/ranges in shared syntax. Do not execute search annotations, external callbacks or high-level MiniZinc code.

## Done when

- [ ] All non-float text forms in the target grammar are admitted with correct literal, parameter-reference, scalar/array alias, index and declaration-order rules; the float seam is covered by float-core. Arbitrary MiniZinc expressions, predicate bodies and invalid reference/type/shape forms still diagnose.
- [ ] Known external declarations validate argument types, roles and shapes without requiring upstream formal parameter spelling. Unknown/mismatched declarations and undeclared external calls are diagnosed; annotations on declarations follow the target grammar.
- [ ] Standard/Gecode search, propagation, output and introduced-variable annotations are parsed and classified without changing feasibility. Application metadata and source ranges survive; unknown semantic annotations and experimental external/search-state hooks receive located diagnostics.

## Validation

- Exercise compact grammar/admission cases and actual emitted declarations/annotations from both pinned libraries, including renamed formal parameters, nested annotation arrays and malformed signatures.
- Run Rust workspace checks and both zdev checks; confirm formatter/linter/query source-preservation behavior if shared syntax changes.
