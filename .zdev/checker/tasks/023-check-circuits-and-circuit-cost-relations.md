+++
schema_version = 1
id = "checker-023"
key = "circuit-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-012"]
+++
# Check circuits and circuit-cost relations

## Outcome

Gecode circuit and cost predicates report concrete topology and cost failures.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read circuit and cost wrappers for successor offsets, cost-matrix layout and the public/native distinction.

## Done when

- [x] Every assigned circuit, circuit_cost and circuit_cost_array alias checks successor domains, permutation/single-cycle requirements and documented cost calculations.
- [x] Empty/singleton inputs, disconnected subtours, offsets and cost overflow follow upstream semantics; unique costs derive from known successors, without searching for a circuit.
- [x] Typed failures identify supplied successors/costs and relevant positions. Shape/reference validation and known violations with incomplete other inputs agree natively and after loading/Wasm.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added nine circuit aliases with typed topology and independent edge/total cost faults, deterministic marked cost outputs and validated loading; total admission is 779 names.

Validation:

- Independent full review passed against W43135082c2601147, including naming and responsibility boundaries; snapshot comparison equal.
- Workspace formatting, Clippy and tests; parser-free checks/tests; locked release Wasm with matching bindings and parser-excluding dependencies; 100 native/Wasm comparisons passed.
- Pinned coverage comparison, Ruff/ty, both MiniZinc controls under both profiles, format/default lint and 36 both-CLI source/export/load comparisons passed. Compiler-marker limits documented for checker-030/031.
