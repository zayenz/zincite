+++
schema_version = 1
id = "base-032"
key = "binding-capture"
area = "base"
status = "done"
complexity = "advanced"
afk = true
priority = "normal"
blocked_by = ["base-031"]
+++
# Resolve bindings and report captured global decisions

## Outcome

Thesis rule global-variable-in-function reports real captures using lexical declaration identity.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.7 end to end. Start at CST declaration/name roles and naming.rs traversal; add only the scope and reference tables needed by this rule and later consumers.

## Done when

- [x] Resolve top-level and included declarations, callable parameters, let/generator/index bindings, aliases and enum names with correct visibility and shadowing; field labels are not lexical references.
- [x] Report captured top-level decision variables in user callable bodies with exact reference locations and suppressions; do not warn on parameter globals or shadowing.
- [x] Represent unresolved/ambiguous binding facts explicitly and expose them as analysis limitations to consumers.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check nested shadowing, forward/global references, included decisions and parameter-valued globals through library/CLI examples.

## Result

Added reusable lexical binding and declared-instantiation facts, then enabled global-variable-in-function advice for captured top-level decisions with precise locations, suppression and explicit analysis limitations. Fact production and diagnostic policy remain separate; standalone and legacy APIs report missing context honestly.

Validation:

- Independent verification passed against W2db008252fa58283, with unchanged snapshot comparison before completion.
- cargo fmt --all -- --check, cargo clippy --workspace --all-targets -- -D warnings and cargo test --workspace passed; 74 tests.
- MiniZinc 2.10.1 model-check-only accepted the two complete examples; installed-library CLI checks reported 9 and 4 captures with no analysis limitations or errors and no source writes.
- Focused public library and CLI checks cover nested shadowing, forward/included references, aliases, enum names, parameter globals, unknown/ambiguous facts, suppression, exact ranges and 0/1/2 outcomes.
