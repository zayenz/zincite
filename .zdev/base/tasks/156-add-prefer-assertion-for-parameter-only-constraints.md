+++
schema_version = 1
id = "base-156"
key = "prefer-assertion"
area = "base"
status = "done"
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

- [x] `constraint n > 0;` reports a finding; `constraint assert(n > 0, "...");` and a constraint over a decision stay quiet.
- [x] The rule is style family, warning severity, not in the default, and a member of the conventions preset.
- [x] `docs/lint-conventions.md` documents the rule with an example.
- [x] The conventions row of the preset table in `docs/linting.md` lists the rule.

## Validation

- Add a focused test with the cases above.
- Run the brief's Rust validation commands and `zdev check base --format json`.

## Result

Added prefer-assertion, an opt-in style rule in the conventions preset that suggests assert for top-level constraints depending only on parameters. Existing assert calls in any written form, decision-dependent constraints, lets with local constraints and unknown instantiation stay quiet.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace, zdev check: all pass
- Release CLI: constraint n > 0 and the docs example warn; parenthesized, annotated, quoted and infix asserts and let-local constraints stay quiet
- Independent Codex verifier: rework, one correction, recheck rework (infix assert), user-authorized second correction, final recheck pass on snapshot W4442f77d6366805e
