+++
schema_version = 1
id = "checker-016"
key = "elements-conditionals"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-014", "checker-015"]
+++
# Support elements, offsets and conditionals for every value type

## Outcome

Array selection and conditional predicates work for Boolean, integer, float and finite-set values.

## Context

Read the area brief, especially Full FlatZinc and library support and Expansion validation, and the coverage table produced by full-coverage. Start with crates/zincite-check/src/{prepare,lib,serialized}.rs and tests/{checking,serialized,cli}.rs. Read standard element/versioned redefinitions and Gecode 1D/2D element and *_ite signatures. Current arrays are 1D prepared bindings, so preserve actual flattening/offset semantics rather than inventing multidimensional runtime arrays.

## Done when

- [ ] All assigned array_[var_]TYPE_element spellings, emitted nonshifted variants, Gecode offset/2D forms and int/bool/set/float conditional operations validate dimensions/index sets and check the selected value.
- [ ] Known indices/conditions derive the unique selected output without waiting for irrelevant branches; absent selected inputs block completion. Out-of-range coordinates, offsets, empty arrays and row/column layout follow the actual signature.
- [ ] The source and loader paths validate the same shapes/types; indexed and conditional findings include typed indices/arguments and preserve explicit output conflicts and repeated-check freshness.

## Validation

- Add focused public cases for the named semantic distinctions, loading rejection and deterministic completion; compare a representative new report before/after serialization and through parser-free Wasm.
- Run the established Rust workspace checks, parser-free Wasm build and focused native/Wasm smoke; update the coverage table and checker guide.
