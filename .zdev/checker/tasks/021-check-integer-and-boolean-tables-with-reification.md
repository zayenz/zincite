+++
schema_version = 1
id = "checker-021"
key = "table-globals"
area = "checker"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-013", "checker-014"]
+++
# Check integer and Boolean tables with reification

## Outcome

Gecode table predicates validate tuples and report membership failures directly.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Inspect gecode_table_int/bool and their reified/implication registry handlers and standard table decomposition output.

## Done when

- [x] All assigned table spellings, tuple layout conventions and _reif/_imp variants validate widths, flattened dimensions and value types before checking.
- [x] Concrete tuple membership, duplicates, empty tables, zero-width cases and false guards follow upstream semantics; reifier outputs derive only from a known tuple.
- [x] Typed table findings and metadata survive loading and Wasm; missing tuple entries produce incomplete reports without choosing an allowed tuple.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.

## Result

Added all 18 integer and Boolean table names with validated row-major shapes, deterministic reifier completion, typed findings and CLI rendering; total admission is 761 names.

Validation:

- Independent full review passed against W298887314e572826, including naming and responsibility boundaries; snapshot comparison equal.
- Workspace formatting, Clippy and tests; parser-free check/test; locked release Wasm with matching bindings and parser-excluding dependency trees; 86 native/Wasm comparisons passed.
- Pinned coverage comparison, Ruff/ty, MiniZinc format/default lint, both profiles direct controls and both CLI source/export/load reports passed. Standard-wrapper abort and Gecode metadata limits documented for checker-030/031.
