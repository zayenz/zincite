+++
schema_version = 1
id = "base-037"
key = "domain-advice"
area = "base"
status = "open"
complexity = "standard"
afk = true
priority = "normal"
blocked_by = ["base-036"]
+++
# Report unbounded decisions without false defining-equality warnings

## Outcome

Thesis rule unbounded-variable respects explicit bounds and complete definitions.

## Context

Read brief Corpus and thesis expansion and the applicable sections of ../background/thesis-rule-coverage.md. Implement section 4.8 using the domain and enforced-definition facts already consumed by constant-variable; keep the advice independent of solver inference.

## Done when

- [ ] Report unbounded integer/float decisions and array element declarations through aliases, excluding explicit domains and initializer/complete defining equations.
- [ ] Conditional equalities, circular self-definitions and partial element constraints do not incorrectly suppress warnings; nonnumeric decisions are not flagged.

## Validation

- Run the area Cargo checks for Rust changes and zdev check for record changes; no tests solely for documentation.
- Use scalar/array int/float examples, alias bounds, a conditional equation and a partial array definition; check selected-rule/suppression behavior.
