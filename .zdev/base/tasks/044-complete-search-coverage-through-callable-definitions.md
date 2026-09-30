+++
schema_version = 1
id = "base-044"
key = "search-callables"
area = "base"
status = "open"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-043"]
+++
# Complete search coverage through callable definitions

## Outcome

Thesis search-coverage follows guaranteed callable output definitions and completes its required rule families.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Section 4.9 follows output-argument definitions such as count(xs, value, result) recursively through enforced callable bodies. Extend existing context/dependency analysis; do not add general theorem proving.

## Done when

- [ ] Map unconditional output-parameter definitions through resolved user/standard calls, recursive call graphs and transparent array views, using conservative terminating analysis.
- [ ] Do not infer whole-array or unconditional definitions from partial access, filtered/reified calls, opaque functions or cyclic summaries; expose unavailable facts.
- [ ] Exercise all section 4.9 families and remove the direct-only limitation, with documented soundness boundaries and source diagnostics.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check count-like output arguments, nested user predicates, array1d, recursion, a reified call and a partly defined array; verify current standard-library signatures.
