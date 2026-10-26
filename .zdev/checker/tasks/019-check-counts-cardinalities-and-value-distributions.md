+++
schema_version = 1
id = "checker-019"
key = "counting-globals"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-013", "checker-014"]
+++
# Check counts, cardinalities and value distributions

## Outcome

Gecode counting and distribution globals have direct structured checking and deterministic count outputs.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Use the coverage table for exact count/among signatures, relation codes, closed/open universes and offset conventions.

## Done when

- [ ] Assigned count/count_eq/reif/imp, at_least/at_most, among, nvalue, global_cardinality/closed/low_up and positional count_matches forms and aliases check all supplied values.
- [ ] Exact count outputs derive when unique; open versus closed cardinality, repeated cover values, empty inputs, lower/upper bounds and full/half reification follow their source contracts.
- [ ] Typed findings identify offending counts/values/positions and invalid shapes/codes diagnose at both load paths; incomplete inputs and explicit count conflicts preserve the report contract.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
