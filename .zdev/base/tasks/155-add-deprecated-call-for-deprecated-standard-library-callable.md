+++
schema_version = 1
id = "base-155"
key = "deprecated-call"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-153", "base-152", "base-154"]
+++
# Add deprecated-call for deprecated standard-library callables

## Outcome

A new default suspicious rule reports calls that resolve to a standard-library callable annotated `mzn_deprecated`, quoting the version and explanation from the annotation.

## Context

The 2.10.1 stdlib declares `annotation mzn_deprecated(string: version, string: explanation);` in `std/stdlib/stdlib_ann.mzn`. Callables in the stdlib's `*.deprecated.mzn` files carry it, for example `all_different_set` and `cumulatives_opt`. Zincite already resolves calls to standard callables (see `callables.rs` and `global_uses.rs`). New rules register in `Rule`, `Rule::all()`, the catalogue match, presets and `Rule::DEFAULT`, and need a docs entry in their group page.

## Boundaries

- Use the resolved callable identity; a user-defined callable with the same name is not deprecated.
- Unresolved or ambiguous calls follow the existing limitation behavior.
- No fix.

## Done when

- [ ] A model calling a deprecated global reports one finding at the call with the annotation's version and explanation.
- [ ] A non-deprecated overload, and a user predicate with a deprecated callable's name, stay quiet.
- [ ] The rule is suspicious family, warning severity, and a member of the conventions preset. The default preset ends with exactly `pairwise-self-comparison, deprecated-call`.
- [ ] `docs/lint-conventions.md` documents the rule with an example. The preset table and default description in `docs/linting.md`, and the CLI help listing of the default, include it.

## Validation

- Add a focused test with the positive and quiet cases, using a small stub stdlib like other rule tests.
- Check one positive case against the installed 2.10.1 stdlib.
- Run the brief's Rust validation commands and `zdev check base --format json`.
