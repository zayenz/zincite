+++
schema_version = 1
id = "base-149"
key = "unused-declaration-skip-generators"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = []
+++
# Leave unused generator bindings to their own rule

## Outcome

unused-declaration no longer reports generator bindings; unused-generator-binding remains their only reporter.

## Context

`constraint forall (j in 1..n) (x[1] > 0);` currently reports `j` twice: as `declaration 'j' is unreachable` from unused-declaration and from unused-generator-binding. About 390 of the roughly 1,000 unused-declaration findings in the corpus review were on generator lines. Start in `crates/zincite-lint/src/unused_declarations.rs`.

## Boundaries

- Keep reporting unused top-level declarations, callables, and let-locals.
- Do not change unused-generator-binding.

## Done when

- [ ] The example in Context produces one unused-generator-binding finding and no unused-declaration finding.
- [ ] Unused top-level parameters, decisions and functions are still reported.
- [ ] `docs/lint-scope.md` states which rule reports generator bindings.

## Validation

- Update the existing unused-declaration tests with the generator case.
- Run the brief's Rust validation commands and `zdev check base --format json`.
