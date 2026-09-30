+++
schema_version = 1
id = "base-036"
key = "definitions-constant"
area = "base"
status = "done"
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

- [x] Detect parameter initializers and unconditional scalar equalities in constraints/conjunctions and resolved transparent wrappers.
- [x] Recognise full unfiltered forall definitions over matching array index sets, including multiple indices; reject partial, filtered, mismatched and cyclic definitions as proof of a whole constant array.
- [x] Deliver file-aware constant-variable advice and reusable definition/dependency facts without evaluating arbitrary functions or instance values.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check initializer/equality/full-array positives and disjunction, implication, partial array, N/K mismatch and cycle negatives with a compact fixture.

## Result

Added independent definition/dependency facts and constant-variable advice for supported parameter initializers, unconditional scalar equalities and complete matching forall definitions. Explicit enforcement/safety states, original-scope forwarding defaults and the bounded mixed value/callable binding correction preserve the stated proof boundaries. Independent verification confirmed the whole task against W9e9ebdc3700eaff3.

Validation:

- cargo fmt --all -- --check passed.
- cargo clippy --workspace --all-targets -- -D warnings passed.
- cargo test --workspace passed: 88 tests.
- Independent whole-task review PASS against W9e9ebdc3700eaff3; official comparisons equal, no repository validation writes.
- Four bounded MiniZinc 2.10.1 model-check-only/core CLI replays matched five positive warnings, two sparse/forwarding warnings, one default-forwarding warning and quiet controls with two explicit safety limitations. No solver ran.
- Reverse-name mixed binding regression failed before the bounded fix and passed afterward; initial catalogue-count failure and original annotation-name collision remain separate rejected evidence.
- zdev check base --format json and git diff --check passed. Six foreign baseline paths stayed byte-identical. Existing globals loader-cycle failures remain outside this task's acceptance claim.
