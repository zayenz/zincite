+++
schema_version = 1
id = "checker-030"
key = "stdlib-profile"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-012", "checker-013", "checker-014", "checker-015", "checker-018", "checker-016", "checker-017"]
+++
# Compile the complete standard library to supported checker operations

## Outcome

A documented offline MiniZinc standard-library profile compiles mathematical models to supported FlatZinc.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read the pinned standard library include/redefinitions hierarchy. The initial recipe used a Gecode compiler selection plus one custom override; this task establishes the standard-decomposition path independently.

## Boundaries

- Do not vendor the entire standard library or build a MiniZinc interpreter. Preserve required upstream notices for any small copied declarations.

## Done when

- [ ] A minimal packaged checker library/configuration exposes the supported standard builtins and uses MiniZinc 2.10.1 standard decompositions for other globals, with an explicit reproducible --compile command and entry point.
- [ ] The standard include/decomposition closure is checked against the coverage table. Representative integer, Boolean, set, float, global, optional and reified models compile without unsupported emitted mathematical predicates; missing primitive gaps are fixed in their owning family rather than hidden.
- [ ] Application metadata, aliases and definitions are inspected after compilation. Documentation states actual domain tightening/elimination and symmetry choices; no arbitrary source-equivalence or solver capability is advertised.

## Validation

- Run compile-only cases through the pinned standard sources, inspect emitted names/signatures/metadata and directly calculate representative valid/invalid/partial reports. No solver matrix.
- Run appropriate existing Rust/Wasm checks for changed code, MiniZinc formatting/linting for authored models and both zdev checks.
