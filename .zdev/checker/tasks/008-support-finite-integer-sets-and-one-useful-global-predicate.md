+++
schema_version = 1
id = "checker-008"
key = "finite-sets"
area = "checker"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["checker-004", "checker-005", "checker-006"]
+++
# Support finite integer sets and one useful global predicate

## Outcome

The checker supports a bounded finite-set dialect and directly checks one example-driven global predicate with useful structured feedback.

## Context

Read [the area brief](../brief.md), especially the Checking contract and Testing sections. Expand the working compiler/runtime/serialized-model path. Finite sets are the planned next value family; choose the global from an actual constraint in the application example, documenting why keeping it intact improves feedback or avoids measured decomposition cost. Do not expand to floats.

## Boundaries

- Keep set domains, values and operations finite and bounded; avoid expanding large ranges merely to represent them.
- Support membership, equality, subset, cardinality and union/intersection as needed for a small complete example. Add only one justified global predicate, not a global-predicate catalogue.

## Done when

- [ ] A small model compiles and checks finite-set values, relevant domains, arrays and supported deterministic set definitions; unsupported set operations still fail preparation explicitly.
- [ ] The selected global predicate has a documented checker-library declaration and concrete evaluator/reporting semantics, with one valid and one violating application example. The choice and coverage table are recorded.
- [ ] Serialized-model round trips preserve set/global reports and metadata. Domain failures, missing inputs and repeated checking preserve the brief's violation/completeness behavior, and parser-free Wasm builds still work.

## Validation

- Add a few public checks combining set-domain/definition behavior, an incomplete set value, and the global predicate's valid/invalid cases; reuse serialization checks.
- Compile and directly inspect the small example, run the Rust workspace checks and the established parser-free Wasm build.
