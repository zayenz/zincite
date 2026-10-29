+++
schema_version = 1
id = "base-158"
key = "float-equality"
area = "base"
status = "done"
complexity = "standard"
afk = true
priority = "low"
blocked_by = ["base-152"]
+++
# Add float-equality for parameter float comparisons

## Outcome

A new opt-in suspicious rule reports `=` and `!=` between parameter-valued float expressions.

## Context

Computed floats rarely compare equal exactly, so `if total / n = 0.5 then ... endif` over parameters is a common mistake. A decision float equality is an ordinary constraint and must stay quiet. Type and instantiation facts are in `types.rs` and `instantiations.rs`.

## Boundaries

- Report only when both operands are proved par float. Integer, decision and unknown-typed comparisons stay quiet.
- Opt-in only. No fix.

## Done when

- [x] A par float `=` and `!=` each report a finding.
- [x] `var float: x; constraint x = 1.5;` and an integer equality stay quiet.
- [x] The rule is suspicious family, warning severity, not in the default, and a member of the domains preset.
- [x] `docs/lint-domains.md` documents the rule with an example.
- [x] The domains row of the preset table in `docs/linting.md` lists the rule.

## Validation

- Add a focused test with the cases above.
- Run the brief's Rust validation commands and `zdev check base --format json`.

## Result

Added float-equality, an opt-in suspicious rule in the domains preset that reports =, == or != between two proved parameter floats. Decision, integer, optional, unknown-typed and user-overloaded comparisons stay quiet.

Validation:

- cargo fmt --check, cargo clippy -D warnings, cargo test --workspace, zdev check: all pass
- Release CLI with 2.10.1 stdlib: par float comparisons in initializers, filters, lets, function bodies and array accesses warn; decision and integer comparisons quiet; docs example renders
- Independent Codex verifier: pass on snapshot W1ca63a0d5c526b13
