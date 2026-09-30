+++
schema_version = 1
id = "base-032"
key = "binding-capture"
area = "base"
status = "open"
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

- [ ] Resolve top-level and included declarations, callable parameters, let/generator/index bindings, aliases and enum names with correct visibility and shadowing; field labels are not lexical references.
- [ ] Report captured top-level decision variables in user callable bodies with exact reference locations and suppressions; do not warn on parameter globals or shadowing.
- [ ] Represent unresolved/ambiguous binding facts explicitly and expose them as analysis limitations to consumers.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Check nested shadowing, forward/global references, included decisions and parameter-valued globals through library/CLI examples.
