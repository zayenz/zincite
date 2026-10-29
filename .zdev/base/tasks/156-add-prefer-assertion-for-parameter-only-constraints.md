+++
schema_version = 1
id = "base-156"
key = "prefer-assertion"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-152"]
+++
# Add prefer-assertion for parameter-only constraints

## Outcome

A new opt-in style rule suggests `assert` for constraint items whose expression depends only on parameters.

## Context

`constraint n > 0;` with a parameter `n` fails as an unexplained model inconsistency; `constraint assert(n > 0, "n must be positive");` explains the failed input. The user's style guidance favours input assertions. Instantiation facts already classify parameter and decision dependence (see `instantiations.rs` and how decision-variable-condition uses them).

## Boundaries

- Stay quiet when the expression already is an `assert` call, when it depends on any decision, or when instantiation is unknown.
- Opt-in only. No fix.

## Done when

- [ ] `constraint n > 0;` reports a finding; `constraint assert(n > 0, "...");` and a constraint over a decision stay quiet.
- [ ] The rule is style family, warning severity, not in the default, and a member of the conventions preset.
- [ ] `docs/lint-conventions.md` documents the rule with an example.
- [ ] The conventions row of the preset table in `docs/linting.md` lists the rule.

## Validation

- Add a focused test with the cases above.
- Run the brief's Rust validation commands and `zdev check base --format json`.
