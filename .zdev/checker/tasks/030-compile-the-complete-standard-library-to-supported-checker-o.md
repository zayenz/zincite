+++
schema_version = 1
id = "checker-030"
key = "stdlib-profile"
area = "checker"
status = "done"
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

- [x] A minimal packaged checker library/configuration exposes the supported standard builtins and uses MiniZinc 2.10.1 standard decompositions for other globals, with an explicit reproducible --compile command and entry point.
- [x] The standard include/decomposition closure is checked against the coverage table. Representative integer, Boolean, set, float, global, optional and reified models compile without unsupported emitted mathematical predicates; missing primitive gaps are fixed in their owning family rather than hidden.
- [x] Application metadata, aliases and definitions are inspected after compilation. Documentation states actual domain tightening/elimination and symmetry choices; no arbitrary source-equivalence or solver capability is advertised.

## Validation

- Run compile-only cases through the pinned standard sources, inspect emitted names/signatures/metadata and directly calculate representative valid/invalid/partial reports. No solver matrix.
- Run appropriate existing Rust/Wasm checks for changed code, MiniZinc formatting/linting for authored models and both zdev checks.

## Result

Packaged an independent MiniZinc 2.10.1 standard compile-only profile, with complete selected include and primitive accounting. Definition hints are optional, known compiler metadata copies preserve distinct application roles, and operand paths are validated. Independent full review and focused candidate-format recheck passed.

Validation:

- Coverage checks 752 selected standard files, zero missing standard edges and 128 native-prepared direct signatures; Gecode selection stays separate. Upstream abort bodies and table-width restrictions are documented compiler limits without runtime-support credit.
- Fifteen controls compile and prepare; separate checks cover four 2D element types, regex lowering and eight decreasing includes. Both CLI source/load forms agree on valid, five-finding invalid, one-finding supplied-output conflict and ten-unresolved incomplete candidates.
- Workspace fmt, Clippy -D warnings and 659 tests passed; parser-free native 20 tests, supported feature Clippy, locked release Wasm, matching Node/web bindings and parser-free dependency checks passed. Native/Wasm smoke passed 168 comparisons across 19 artifacts and 84 candidates; 80 prior candidate records are unchanged.
- MiniZinc model/data formatting and linting, coverage helper, Ruff, ty, package listing and diff checks passed. Full review Wc1fe1fa5de404ec7 and focused recheck W4885d279715e042d accepted the sole four-file data-format correction.
