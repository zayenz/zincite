+++
schema_version = 1
id = "checker-022"
key = "automata-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-017"]
+++
# Check deterministic and nondeterministic regular constraints

## Outcome

Gecode regular, regular-set and NFA predicates check finite input words without variable search.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read regular/regular_nfa wrappers, flattened transition encodings, state/alphabet offsets and registry DFA construction. Reachable-state computation over a concrete word is evaluation, not choosing missing assignment values.

## Done when

- [x] All assigned regular, regular_set, regular_nfa_int and set-labelled NFA encodings validate transition dimensions, offsets, accepting/start states and symbols before execution.
- [x] Concrete words use the correct acceptance semantics for nondeterminism, empty words and missing/dead transitions. A missing symbol blocks the relation without guessing a symbol.
- [x] Typed acceptance findings and source metadata survive validated loading and Wasm; malformed automaton encodings diagnose and repeated calls have fresh state.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added nine exact DFA/NFA names with shared fixed-encoding validation, concrete acceptance and typed rejection findings; corrected inclusive regular_set endpoint semantics. Total admission is 770 names.

Validation:

- Independent full review passed against Wf852493528e971f2, including naming and responsibility boundaries; snapshot comparison equal.
- Workspace formatting, Clippy and tests; locked parser-free checks/tests; release Wasm with matching bindings and parser-excluding dependency trees; 92 native/Wasm comparisons passed.
- Pinned coverage comparison, Ruff/ty, both MiniZinc controls under both profiles, format/default lint and both CLI source/export/load reports passed. Standard-wrapper auxiliary-witness limitation documented for checker-030/031.
