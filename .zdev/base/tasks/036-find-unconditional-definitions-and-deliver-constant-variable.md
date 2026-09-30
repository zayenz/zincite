+++
schema_version = 1
id = "base-036"
key = "definitions-constant"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-035", "base-034"]
+++
# Find unconditional definitions and deliver constant-variable advice

## Outcome

Thesis rule constant-variable recognises parameter-valued scalar and complete array definitions safely.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.3 and the shared enforced-context walk described in the rule contract. Use declaration identity and par/var facts; keep equations in disjunctions or opaque contexts conditional.

## Done when

- [ ] Detect parameter initializers and unconditional scalar equalities in constraints/conjunctions and resolved transparent wrappers.
- [ ] Recognise full unfiltered forall definitions over matching array index sets, including multiple indices; reject partial, filtered, mismatched and cyclic definitions as proof of a whole constant array.
- [ ] Deliver file-aware constant-variable advice and reusable definition/dependency facts without evaluating arbitrary functions or instance values.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check initializer/equality/full-array positives and disjunction, implication, partial array, N/K mismatch and cycle negatives with a compact fixture.
