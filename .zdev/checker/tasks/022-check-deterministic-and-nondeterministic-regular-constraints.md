+++
schema_version = 1
id = "checker-022"
key = "automata-globals"
area = "checker"
status = "open"
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

- [ ] All assigned regular, regular_set, regular_nfa_int and set-labelled NFA encodings validate transition dimensions, offsets, accepting/start states and symbols before execution.
- [ ] Concrete words use the correct acceptance semantics for nondeterminism, empty words and missing/dead transitions. A missing symbol blocks the relation without guessing a symbol.
- [ ] Typed acceptance findings and source metadata survive validated loading and Wasm; malformed automaton encodings diagnose and repeated calls have fresh state.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
