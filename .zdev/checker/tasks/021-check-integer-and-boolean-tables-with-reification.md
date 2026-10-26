+++
schema_version = 1
id = "checker-021"
key = "table-globals"
area = "checker"
status = "open"
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

- [ ] All assigned table spellings, tuple layout conventions and _reif/_imp variants validate widths, flattened dimensions and value types before checking.
- [ ] Concrete tuple membership, duplicates, empty tables, zero-width cases and false guards follow upstream semantics; reifier outputs derive only from a known tuple.
- [ ] Typed table findings and metadata survive loading and Wasm; missing tuple entries produce incomplete reports without choosing an allowed tuple.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
